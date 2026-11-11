//! Command handlers and queue editing of the control thread.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent, RepeatMode};
use nemo_core::track::Track;

use crate::control::{Core, NextSlot, NextState, micros};
use crate::pipeline::FeedCmd;
use crate::prepare::{PrepareDeps, spawn_prepare};
use crate::render::gain_for_percent;

/// Past this position, Previous restarts the current track instead of going back.
const RESTART_AFTER: Duration = Duration::from_secs(3);

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

    /// Start opening the next track while the current one plays.
    pub fn prefetch_next(&mut self) {
        let loaded = self.cur.as_ref().is_some_and(|c| c.loaded);
        if !loaded || self.following.is_some() {
            return;
        }
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
            opener: Arc::clone(&self.deps.opener),
        };
        spawn_prepare(deps, self.tx.clone(), token, track);
    }
}
