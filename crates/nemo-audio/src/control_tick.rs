//! The control thread's periodic work: feeder events, state changes, track boundaries.

use std::sync::atomic::Ordering;
use std::time::Duration;

use nemo_core::player::{PlayState, PlayerEvent};

use crate::control::{Core, NextState, Run, micros};
use crate::pipeline::{FeedCmd, FeedEvent, Job, plan_stages};
use crate::replay_gain::ReplayGain;

impl Core {
    /// One round of housekeeping. Called after every message and at least every few ms.
    pub fn tick(&mut self) {
        self.drain_feeder();
        self.sync_pipeline();
        self.flush_due_skips();
        self.check_boundaries();
    }

    fn drain_feeder(&mut self) {
        let mut events = Vec::new();
        if let Some(p) = &self.pipe {
            while let Ok(e) = p.events.try_recv() {
                events.push(e);
            }
        }
        for event in events {
            self.on_feed_event(event);
        }
    }

    fn on_feed_event(&mut self, event: FeedEvent) {
        match event {
            FeedEvent::Started {
                token,
                at_frame,
                base,
            } => self.on_started(token, at_frame, base),
            FeedEvent::Finished { token, end_frame } => {
                if let Some(run) = self.run_mut(token) {
                    run.finished = Some(end_frame);
                }
                self.try_load_following();
            }
            FeedEvent::Skipped {
                token,
                segment,
                at_frame,
            } => self.skips.push_back((token, at_frame, segment)),
            FeedEvent::Failed { token, message } => self.on_feed_failed(token, &message),
            FeedEvent::SeekMiss { token, to } => {
                if let (Some(index), true) = (self.index, self.is_current(token)) {
                    self.start_track(index, to);
                }
            }
        }
    }

    fn is_current(&self, token: u64) -> bool {
        self.cur.as_ref().is_some_and(|c| c.token == token)
    }

    fn run_mut(&mut self, token: u64) -> Option<&mut Run> {
        self.cur
            .iter_mut()
            .chain(self.following.iter_mut())
            .find(|r| r.token == token)
    }

    fn on_started(&mut self, token: u64, at_frame: u64, base: Duration) {
        if self.is_current(token) {
            let pos = &self.shared.pos;
            pos.start.store(at_frame, Ordering::SeqCst);
            pos.base_us.store(micros(base), Ordering::SeqCst);
            if let Some(c) = self.cur.as_mut() {
                c.started = true;
            }
        } else if let Some(f) = self.following.as_mut().filter(|f| f.token == token) {
            f.at_frame = Some(at_frame);
        }
    }

    fn on_feed_failed(&mut self, token: u64, message: &str) {
        if self.is_current(token) {
            self.fail_current(message);
        } else if self.following.as_ref().is_some_and(|f| f.token == token) {
            self.fail(message);
            if let Some(f) = self.following.take() {
                self.failed.insert(f.entry);
            }
        }
    }

    /// Copy device facts into the shared atomics and move Buffering to Playing.
    fn sync_pipeline(&mut self) {
        let Some(pipe) = &self.pipe else { return };
        self.shared
            .pos
            .latency
            .store(pipe.latency_frames(), Ordering::SeqCst);
        let held = pipe.shared.hold.load(Ordering::SeqCst);
        let total = self.underruns_base + pipe.shared.underruns.load(Ordering::Relaxed);
        let started = self.cur.as_ref().is_some_and(|c| c.started);
        if total > self.last_underruns {
            self.last_underruns = total;
            self.notify(&PlayerEvent::UnderrunsCounted { total });
        }
        if self.state == PlayState::Buffering && started && !held {
            self.set_state(PlayState::Playing);
        }
    }

    /// Announce skips once the audio before the cut has been played.
    fn flush_due_skips(&mut self) {
        let played = self.shared.pos.played.load(Ordering::SeqCst);
        while let Some((token, at, _)) = self.skips.front() {
            if played < *at {
                break;
            }
            let token = *token;
            if let Some((_, _, segment)) = self.skips.pop_front() {
                self.last_skip = Some((token, segment.clone()));
                self.notify(&PlayerEvent::SegmentSkipped(segment));
            }
        }
    }

    /// Hand a Ready next track to the feeder right behind the current one (no gap).
    pub fn try_load_following(&mut self) {
        let finished = self.cur.as_ref().is_some_and(|c| c.finished.is_some());
        if self.following.is_some() || !finished {
            return;
        }
        let (Some(pipe), Some(NextState::Ready(prepared))) =
            (self.pipe.as_ref(), self.next.as_ref().map(|n| &n.state))
        else {
            return;
        };
        let format = prepared.decoder.info().format;
        let out = pipe.output_info().format;
        let differs = out.sample_rate != format.sample_rate || out.channels != format.channels;
        if differs && self.deps.sink_factory.can_follow_source() {
            return; // a different format: wait for the audible end, then reopen the sink
        }
        let Some(slot) = self.next.take() else { return };
        let NextState::Ready(prepared) = slot.state else {
            return;
        };
        let mut run = Run::new(slot.token, slot.entry, slot.track, Duration::ZERO);
        run.stages = plan_stages(format, out);
        let replay_gain = ReplayGain::plan(&self.shared.replay_gain.get(), &prepared.playback.gain);
        run.stages.extend(replay_gain.map(|g| g.stage()));
        run.duration = prepared.duration().or(run.track.duration);
        run.source = Some(prepared.info);
        run.loaded = true;
        pipe.send(FeedCmd::Load(Job {
            token: run.token,
            decoder: prepared.decoder,
            start: Duration::ZERO,
            span: prepared.playback.span,
            segments: Vec::new(),
            replay_gain,
        }));
        self.following = Some(run);
        self.fetch_segments();
    }

    fn check_boundaries(&mut self) {
        let played = self.shared.pos.played.load(Ordering::SeqCst);
        let latency = self.shared.pos.latency.load(Ordering::SeqCst);
        if let Some(at) = self.following.as_ref().and_then(|f| f.at_frame)
            && played >= at + latency
        {
            self.promote_following(at);
            return;
        }
        let drained = self
            .cur
            .as_ref()
            .and_then(|c| c.finished)
            .is_some_and(|end| played >= end + latency);
        if drained && self.following.is_none() {
            self.on_track_drained();
        }
    }

    /// The next track became audible: it is now the current one.
    fn promote_following(&mut self, at_frame: u64) {
        let Some(run) = self.following.take() else {
            return;
        };
        self.index = self.queue.iter().position(|e| e.id == run.entry);
        let id = run.track.id.clone();
        let duration = run.duration;
        self.cur = Some(run);
        if let Some(c) = self.cur.as_mut() {
            c.started = true;
        }
        let pos = &self.shared.pos;
        pos.start.store(at_frame, Ordering::SeqCst);
        pos.base_us.store(0, Ordering::SeqCst);
        pos.duration_us
            .store(duration.map_or(0, micros), Ordering::SeqCst);
        self.notify(&PlayerEvent::TrackChanged(id));
        if let Some(d) = duration {
            self.notify(&PlayerEvent::DurationKnown(d));
        }
        if let Some(path) = self.signal_path() {
            self.notify(&PlayerEvent::SignalPathChanged(path));
        }
        self.notify(&PlayerEvent::QueueChanged);
        self.prefetch_next();
        self.try_load_following();
    }

    /// The current track has been played to the end and nothing follows it in the ring.
    fn on_track_drained(&mut self) {
        match self.next.as_ref().map(|n| &n.state) {
            Some(NextState::Ready(_)) => self.switch_to_next_slot(),
            Some(NextState::Preparing) => {}
            Some(NextState::Failed) => {
                if let Some(n) = self.next.take() {
                    self.failed.insert(n.entry);
                }
                self.advance_or_stop();
            }
            None => self.advance_or_stop(),
        }
    }

    fn advance_or_stop(&mut self) {
        match self.next_entry_index() {
            Some(i) => self.start_track(i, Duration::ZERO),
            None => self.stop_all(),
        }
    }

    /// Make the prefetched track current when it could not follow in the ring (a different
    /// format on a sink that can follow the source, or a late open). Reopens the sink if needed.
    fn switch_to_next_slot(&mut self) {
        let Some(slot) = self.next.take() else { return };
        let NextState::Ready(prepared) = slot.state else {
            return;
        };
        let id = slot.track.id.clone();
        self.cur = Some(Run::new(slot.token, slot.entry, slot.track, Duration::ZERO));
        self.index = self.queue.iter().position(|e| e.id == slot.entry);
        let pos = &self.shared.pos;
        pos.start.store(u64::MAX, Ordering::SeqCst);
        pos.base_us.store(0, Ordering::SeqCst);
        self.skips.clear();
        self.last_skip = None;
        self.set_state(PlayState::Buffering);
        self.notify(&PlayerEvent::TrackChanged(id));
        self.notify(&PlayerEvent::QueueChanged);
        self.on_prepared_current(*prepared);
    }

    fn on_prepared_current(&mut self, prepared: crate::prepare::Prepared) {
        let token = self.cur.as_ref().map_or(0, |c| c.token);
        self.on_prepared(token, Ok(Box::new(prepared)));
    }
}
