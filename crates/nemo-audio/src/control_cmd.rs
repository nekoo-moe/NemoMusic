//! Command handlers and queue editing of the control thread.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use nemo_core::audio::{AudioFormat, ProcessingStage};
use nemo_core::effects::EffectsSettings;
use nemo_core::output::{DeviceId, OutputDevice, SavedDevice};
use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent, RepeatMode};
use nemo_core::track::{Track, VideoId};

use crate::control::{Core, NextSlot, NextState, Run, micros};
use crate::feeder::{FrameSpan, file_time};
use crate::pipeline::{FeedCmd, Pipeline, plan_stages};
use crate::prepare::{PrepareDeps, spawn_prepare};
use crate::render::gain_for_percent;
use crate::sink::{AudioSink, OutputSelection};

/// Past this position, Previous restarts the current track instead of going back.
const RESTART_AFTER: Duration = Duration::from_secs(3);
/// Upcoming queue entries whose streams are resolved ahead of time: only the next one, never
/// a whole playlist (fast-start spec).
pub(crate) const WARM_AHEAD: usize = 1;

impl Core {
    // ---- commands ------------------------------------------------------------------------

    pub fn command(&mut self, cmd: PlayerCommand) {
        match cmd {
            PlayerCommand::Play(track) => {
                let entry = self.entry(track);
                self.queue = vec![entry];
                self.start_track(0, Duration::ZERO);
            }
            PlayerCommand::SetQueue { tracks, start } => self.set_queue(tracks, start),
            PlayerCommand::Enqueue(track) => {
                let entry = self.entry(track);
                self.queue.push(entry);
                self.queue_edited();
            }
            PlayerCommand::PlayNext(track) => {
                let at = self.index.map_or(0, |i| i + 1).min(self.queue.len());
                let entry = self.entry(track);
                self.queue.insert(at, entry);
                self.queue_edited();
            }
            PlayerCommand::RemoveAt(i) => self.remove_at(i),
            PlayerCommand::Pause => self.pause(),
            PlayerCommand::Resume => self.resume(),
            PlayerCommand::Stop => self.stop_all(),
            PlayerCommand::Seek(to) => self.seek(to),
            PlayerCommand::Next => self.next_track(),
            PlayerCommand::Previous => self.previous_track(),
            PlayerCommand::SetVolume(p) => self.set_volume(p),
            PlayerCommand::SetSkipSegments(on) => self.set_skip(on),
            PlayerCommand::UndoSkip => self.undo_skip(),
            PlayerCommand::SetRepeat(mode) => self.set_repeat(mode),
            PlayerCommand::Park => self.park(),
            PlayerCommand::Unpark { at } => self.unpark(at),
            PlayerCommand::SwitchAudioTrack(language) => self.switch_audio_track(language),
            PlayerCommand::SetEffects(settings) => self.set_effects(settings),
            PlayerCommand::SetTransition(settings) => {
                self.transition = settings;
                self.publish();
            }
            PlayerCommand::ListOutputDevices => {
                if let Ok(devices) = self.list_devices_internal() {
                    let changed = self.output.devices != devices;
                    self.on_devices_changed(&devices);
                    if !changed {
                        self.notify(&PlayerEvent::OutputDevicesChanged(devices));
                    }
                }
            }
            PlayerCommand::SetOutputDevice(chosen) => self.set_output_device(chosen),
            PlayerCommand::SetOutputOptions(_)
            | PlayerCommand::RetryOutput
            | PlayerCommand::ResumeOnReturnedDevice => {}
        }
    }

    /// Store the effects settings and report them. Nothing reaches the audio path yet
    /// (audio-effects task 11 clamps them and runs the chain).
    fn set_effects(&mut self, settings: EffectsSettings) {
        self.effects.settings = settings;
        self.notify(&PlayerEvent::EffectsChanged(self.effects));
    }

    /// Resolve the current stream again with `language` and reopen it where it is now. A paused
    /// track stays paused. Local tracks and tracks without a video id are left alone. While
    /// parked only the request changes; the next unpark resolves with it.
    fn switch_audio_track(&mut self, language: Option<String>) {
        let Some(cur) = self.cur.as_ref() else {
            return;
        };
        if cur.track.local_path.is_some() || cur.track.video_id.is_none() {
            return;
        }
        let at = if cur.loaded {
            self.shared.position()
        } else {
            cur.start
        };
        self.deps.request.audio_language = language;
        let Some(index) = self.index.filter(|_| !self.parked) else {
            return;
        };
        let paused = self.state == PlayState::Paused;
        self.start_track(index, at);
        if paused {
            self.pause();
        }
    }

    /// Release the output device and stop decoding, keeping the queue position. The current
    /// entry stays current (unloaded), so Next, Previous and queue edits work while parked.
    fn park(&mut self) {
        if self.parked {
            return;
        }
        let index = self.index;
        let current = self.cur.take().map(|c| (c.entry, c.track));
        self.stop_all();
        self.parked = true;
        self.index = index;
        if let Some((entry, track)) = current {
            let token = self.new_id();
            self.cur = Some(Run::new(token, entry, track, Duration::ZERO));
        }
        self.notify(&PlayerEvent::ParkedChanged(true));
    }

    /// Leave the parked mode and play the current entry from `at` on the local device.
    fn unpark(&mut self, at: Duration) {
        if !self.parked {
            return;
        }
        self.parked = false;
        self.notify(&PlayerEvent::ParkedChanged(false));
        if let Some(index) = self.index {
            self.start_track(index, at);
        }
    }

    fn set_repeat(&mut self, mode: RepeatMode) {
        if self.repeat == mode {
            return;
        }
        self.repeat = mode;
        self.notify(&PlayerEvent::RepeatChanged(mode));
        // What plays after the current track may have changed: fix the prefetch and the ring.
        self.queue_edited();
    }

    fn set_queue(&mut self, tracks: Vec<Track>, start: usize) {
        self.queue = tracks.into_iter().map(|t| self.entry(t)).collect();
        if self.queue.is_empty() {
            self.stop_all();
        } else {
            self.start_track(start.min(self.queue.len() - 1), Duration::ZERO);
        }
    }

    fn remove_at(&mut self, i: usize) {
        if i >= self.queue.len() {
            return;
        }
        let removed_current = self
            .cur
            .as_ref()
            .is_some_and(|c| c.entry == self.queue[i].id);
        self.queue.remove(i);
        if removed_current {
            self.notify(&PlayerEvent::QueueChanged);
            if i < self.queue.len() {
                self.start_track(i, Duration::ZERO);
            } else {
                self.stop_all();
            }
        } else {
            self.queue_edited();
        }
    }

    fn pause(&mut self) {
        if let (Some(pipe), PlayState::Playing | PlayState::Buffering) = (&self.pipe, self.state) {
            pipe.shared.paused.store(true, Ordering::SeqCst);
            self.set_state(PlayState::Paused);
        }
    }

    fn resume(&mut self) {
        if self.state != PlayState::Paused {
            return;
        }
        if let Some(pipe) = &self.pipe {
            pipe.shared.paused.store(false, Ordering::SeqCst);
        }
        let primed = self.cur.as_ref().is_some_and(|c| c.started)
            && self
                .pipe
                .as_ref()
                .is_some_and(|p| !p.shared.hold.load(Ordering::SeqCst));
        self.set_state(if primed {
            PlayState::Playing
        } else {
            PlayState::Buffering
        });
    }

    fn set_volume(&mut self, percent: u8) {
        let percent = percent.min(100);
        self.gain
            .store(gain_for_percent(percent).to_bits(), Ordering::Relaxed);
        self.volume = percent;
        self.notify(&PlayerEvent::VolumeChanged(percent));
        if let Some(path) = self.signal_path() {
            self.notify(&PlayerEvent::SignalPathChanged(path));
        }
    }

    fn set_skip(&mut self, on: bool) {
        self.skip = on && self.deps.segments.is_some();
        if let Some(p) = &self.pipe {
            p.send(FeedCmd::SetSkipping(self.skip));
        }
        self.publish();
        self.fetch_segments();
    }

    fn undo_skip(&mut self) {
        let Some((token, segment)) = self.last_skip.take() else {
            return;
        };
        let Some(cur) = self.cur.as_ref().filter(|c| c.token == token) else {
            return;
        };
        let entry = cur.entry;
        self.ignored
            .entry(entry)
            .or_default()
            .insert(segment.id.clone());
        if let Some(p) = &self.pipe {
            p.send(FeedCmd::Ignore {
                token,
                id: segment.id.clone(),
            });
        }
        self.seek(segment.start);
    }

    fn seek(&mut self, to: Duration) {
        let Some(cur) = self.cur.as_ref().filter(|c| c.loaded) else {
            return;
        };
        let to = cur.duration.map_or(to, |d| to.min(d));
        let (token, finished) = (cur.token, cur.finished.is_some());
        let Some(index) = self.index else { return };
        self.skips.clear();
        if finished || self.pipe.is_none() {
            self.start_track(index, to);
            return;
        }
        if let Some(p) = &self.pipe {
            p.send(FeedCmd::Seek { token, to });
        }
        let pos = &self.shared.pos;
        pos.start.store(u64::MAX, Ordering::SeqCst);
        pos.base_us.store(micros(to), Ordering::SeqCst);
    }

    fn next_track(&mut self) {
        match self.index {
            Some(i) if i + 1 < self.queue.len() => self.start_track(i + 1, Duration::ZERO),
            Some(_) if self.repeat == RepeatMode::All => self.start_track(0, Duration::ZERO),
            Some(_) => self.stop_all(),
            None if !self.queue.is_empty() => self.start_track(0, Duration::ZERO),
            None => {}
        }
    }

    fn previous_track(&mut self) {
        match self.index {
            Some(i) if i > 0 && self.shared.position() <= RESTART_AFTER => {
                self.start_track(i - 1, Duration::ZERO);
            }
            Some(_) => self.seek(Duration::ZERO),
            None if !self.queue.is_empty() => {
                self.start_track(self.queue.len() - 1, Duration::ZERO);
            }
            None => {}
        }
    }

    // ---- queue ---------------------------------------------------------------------------

    /// Recompute the index of the current track and fix the prefetch after a queue edit.
    fn queue_edited(&mut self) {
        if let Some(cur) = &self.cur {
            self.index = self.queue.iter().position(|e| e.id == cur.entry);
        }
        self.notify(&PlayerEvent::QueueChanged);
        let want = self.next_entry_index().map(|i| self.queue[i].id);
        if let Some(f) = &self.following
            && Some(f.entry) != want
            && let Some(index) = self.index
        {
            // The next track is already in the ring; reopen the current one at its position.
            let at = self.shared.position();
            self.start_track(index, at);
            return;
        }
        if self.next.as_ref().is_some_and(|n| Some(n.entry) != want) {
            self.next = None;
        }
        self.prefetch_next();
    }

    /// Index of the entry that plays after the current one: the first later entry that has
    /// not failed; with repeat All it wraps to the start, with repeat One it is the current
    /// entry itself (so the replay is gapless too). `None` when nothing follows.
    pub fn next_entry_index(&self) -> Option<usize> {
        let current = self.index?;
        let ok = |i: &usize| !self.failed.contains(&self.queue[*i].id);
        match self.repeat {
            RepeatMode::One => Some(current).filter(ok),
            RepeatMode::Off => (current + 1..self.queue.len()).find(ok),
            RepeatMode::All => (current + 1..self.queue.len()).chain(0..=current).find(ok),
        }
    }

    /// Start opening the next track while the current one plays, and hint the resolver about
    /// the entry after the current one ([`WARM_AHEAD`]) so its stream is resolved early. Only
    /// the next song, never further ahead.
    pub fn prefetch_next(&mut self) {
        let loaded = self.cur.as_ref().is_some_and(|c| c.loaded);
        if !loaded || self.following.is_some() {
            return;
        }
        self.warm_ahead();
        let Some(i) = self.next_entry_index() else {
            self.next = None;
            return;
        };
        if self
            .next
            .as_ref()
            .is_some_and(|n| n.entry == self.queue[i].id)
        {
            return;
        }
        let token = self.new_id();
        let track = self.queue[i].track.clone();
        self.next = Some(NextSlot {
            token,
            entry: self.queue[i].id,
            track: track.clone(),
            state: NextState::Preparing,
        });
        self.spawn_prepare(token, track);
    }

    pub fn spawn_prepare(&self, token: u64, track: Track) {
        let deps = PrepareDeps {
            resolver: Arc::clone(&self.deps.resolver),
            request: self.deps.request.clone(),
            opener: Arc::clone(&self.deps.opener),
            local: self.deps.local.clone(),
            offline: self.deps.offline.clone(),
        };
        spawn_prepare(deps, self.tx.clone(), token, track);
    }

    /// Queue indices of the entries that play after the current one, in order, at most
    /// [`WARM_AHEAD`], skipping failed ones (wraps with repeat All).
    pub fn upcoming(&self) -> Vec<usize> {
        let Some(current) = self.index else {
            return Vec::new();
        };
        let ok = |i: &usize| *i != current && !self.failed.contains(&self.queue[*i].id);
        let order: Vec<usize> = match self.repeat {
            RepeatMode::One => Vec::new(),
            RepeatMode::Off => (current + 1..self.queue.len()).collect(),
            RepeatMode::All => (current + 1..self.queue.len()).chain(0..current).collect(),
        };
        order.into_iter().filter(ok).take(WARM_AHEAD).collect()
    }

    /// Tell the resolver which YouTube tracks come next. Returns at once. A saved song is left
    /// out: it plays from the cache with no stream lookup (offline-cache criterion 3).
    fn warm_ahead(&self) {
        for i in self.upcoming() {
            if let Some(video) = &self.queue[i].track.video_id
                && self.queue[i].track.local_path.is_none()
                && !self.is_saved(video)
            {
                self.deps.resolver.warm(video);
            }
        }
    }

    /// Whether the offline cache holds a saved copy of `video`.
    fn is_saved(&self, video: &VideoId) -> bool {
        self.deps
            .offline
            .as_ref()
            .is_some_and(|store| store.open(video).is_some())
    }

    fn list_devices_internal(&self) -> Result<Vec<OutputDevice>, String> {
        if let Some(ref enumerator) = self.deps.device_enumerator {
            enumerator.list_devices()
        } else {
            self.deps
                .sink_factory
                .list_devices()
                .map_err(|e| e.to_string())
        }
    }

    /// Update output status fields based on the sink factory's current state.
    pub fn refresh_output_status(&mut self) {
        if let Ok(devices) = self.list_devices_internal() {
            self.output.devices = devices;
        }
        if let Some(ref chosen) = self.output.chosen {
            if self.output.devices.is_empty() {
                self.output.saved_missing = self.deps.sink_factory.is_saved_missing();
            } else {
                self.output.saved_missing = !self.output.devices.iter().any(|d| d.id == chosen.id);
            }
        } else {
            self.output.saved_missing = false;
        }
        self.output.active = if let Some(ref pipe) = self.pipe {
            let active_name = &pipe.output_info().device_name;
            self.output
                .devices
                .iter()
                .find(|d| &d.name == active_name)
                .cloned()
                .or_else(|| {
                    Some(OutputDevice {
                        id: DeviceId(active_name.clone()),
                        name: active_name.clone(),
                        is_default: false,
                    })
                })
        } else if self.output.saved_missing || self.output.chosen.is_none() {
            self.output.devices.iter().find(|d| d.is_default).cloned()
        } else if let Some(ref chosen) = self.output.chosen {
            self.output
                .devices
                .iter()
                .find(|d| d.id == chosen.id)
                .cloned()
                .or_else(|| {
                    Some(OutputDevice {
                        id: chosen.id.clone(),
                        name: chosen.name.clone(),
                        is_default: false,
                    })
                })
        } else {
            None
        };
    }

    /// Update output devices and handle defaults or returning devices.
    pub fn on_devices_changed(&mut self, devices: &[OutputDevice]) {
        if self.output.devices == devices {
            return;
        }

        self.output.devices = devices.to_vec();
        let new_default = devices.iter().find(|d| d.is_default).map(|d| &d.id);

        self.notify(&PlayerEvent::OutputDevicesChanged(devices.to_vec()));

        // Check if a saved device has returned or gone missing
        if let Some(ref chosen) = self.output.chosen {
            let is_present = devices.iter().any(|d| d.id == chosen.id);
            if self.output.saved_missing && is_present {
                self.output.saved_missing = false;
                if self.state == PlayState::Stopped || self.state == PlayState::Paused {
                    self.set_output_device(Some(chosen.clone()));
                } else {
                    self.refresh_output_status();
                    self.notify(&PlayerEvent::OutputChanged(self.output.clone()));
                    self.notify(&PlayerEvent::SavedDeviceReturned);
                }
            } else if !self.output.saved_missing && !is_present {
                self.output.saved_missing = true;
                self.refresh_output_status();
                self.notify(&PlayerEvent::OutputChanged(self.output.clone()));
            }
        }

        // With no device chosen, follow the default device if it changed
        if self.output.chosen.is_none() && self.output.active.as_ref().map(|d| &d.id) != new_default
        {
            self.set_output_device(None);
        }
    }

    /// Set the output device, opening the new sink and migrating playback if active.
    pub fn set_output_device(&mut self, chosen: Option<SavedDevice>) {
        let old_selection = OutputSelection {
            device: self.output.chosen.as_ref().map(|d| d.id.clone()),
            options: self.output.options,
        };
        let new_selection = OutputSelection {
            device: chosen.as_ref().map(|d| d.id.clone()),
            options: self.output.options,
        };

        let format = if let Some(pipe) = &self.pipe {
            pipe.output_info().format
        } else if let Some(cur) = &self.cur {
            cur.source
                .as_ref()
                .and_then(|s| s.format)
                .unwrap_or(AudioFormat {
                    sample_rate: 44100,
                    channels: 2,
                    bit_depth: Some(16),
                })
        } else {
            AudioFormat {
                sample_rate: 44100,
                channels: 2,
                bit_depth: Some(16),
            }
        };

        self.deps.sink_factory.select(new_selection);
        let new_sink = match self.deps.sink_factory.open(format) {
            Ok(sink) => sink,
            Err(err) => {
                self.deps.sink_factory.select(old_selection);
                self.notify(&PlayerEvent::OutputOpenFailed {
                    reason: err.to_string(),
                });
                return;
            }
        };

        if let Err(reason) = self.migrate_pipeline(new_sink) {
            self.notify(&PlayerEvent::OutputOpenFailed { reason });
            return;
        }

        self.output.chosen = chosen;
        self.refresh_output_status();
        self.notify(&PlayerEvent::OutputChanged(self.output.clone()));
        self.publish();
        if let Some(path) = self.signal_path() {
            self.notify(&PlayerEvent::SignalPathChanged(path));
        }
    }

    fn migrate_pipeline(&mut self, new_sink: Box<dyn AudioSink>) -> Result<(), String> {
        let Some(old_pipe) = self.pipe.take() else {
            return Ok(());
        };
        let pos = self.shared.position();
        self.underruns_base += old_pipe.shared.underruns.load(Ordering::Relaxed);
        let mut jobs = old_pipe.stop_and_take();

        let source_format = jobs.first().map(|j| j.decoder.info().format);

        if let Some(first_job) = jobs.first_mut() {
            first_job.start = pos;
            let rate = first_job.decoder.info().format.sample_rate;
            let span = first_job.span.map(|s| FrameSpan::new(s, rate));
            let from = file_time(span, pos, rate);
            let _ = first_job.decoder.seek(from);
        }

        let pipe = Pipeline::start(new_sink, &self.shared.pos.played, &self.gain)
            .map_err(|e| e.to_string())?;

        self.shared
            .pos
            .rate
            .store(pipe.output_info().format.sample_rate, Ordering::SeqCst);
        self.shared.pos.start.store(0, Ordering::SeqCst);
        self.shared.pos.base_us.store(micros(pos), Ordering::SeqCst);
        self.shared
            .pos
            .latency
            .store(pipe.latency_frames(), Ordering::SeqCst);
        pipe.send(FeedCmd::SetSkipping(self.skip));

        if self.state == PlayState::Paused {
            pipe.shared.paused.store(true, Ordering::Relaxed);
        }

        for job in jobs {
            pipe.send(FeedCmd::Load(job));
        }

        if let Some(cur) = &self.cur {
            for id in self.ignored.get(&cur.entry).into_iter().flatten() {
                pipe.send(FeedCmd::Ignore {
                    token: cur.token,
                    id: id.clone(),
                });
            }
        }

        if let (Some(cur), Some(format)) = (self.cur.as_mut(), source_format) {
            let replay_stage = cur
                .stages
                .iter()
                .find(|s| matches!(s, ProcessingStage::ReplayGain { .. }))
                .copied();
            cur.stages = plan_stages(format, pipe.output_info().format);
            cur.stages.extend(replay_stage);
        }

        self.pipe = Some(pipe);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
