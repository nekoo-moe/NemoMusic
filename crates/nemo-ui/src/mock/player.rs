//! The mock player: a clock that advances while playing and emits the matching events.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use super::{signal_path, track};
use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent, PlayerSnapshot, RepeatMode};
use nemo_core::services::{EventReceiver, PlayerControl};
use nemo_core::track::Track;

type Subscribers = Vec<Arc<Mutex<VecDeque<PlayerEvent>>>>;

struct PlayerState {
    snapshot: PlayerSnapshot,
    /// Position when the clock was last stopped or re-anchored.
    base: Duration,
    /// When the clock started running; `None` while not playing.
    since: Option<Instant>,
    subscribers: Subscribers,
}

/// A fake player: its clock advances while playing and every [`PlayerCommand`] changes its state
/// and emits the matching [`PlayerEvent`]s. Tracks end by themselves and move to the next one.
pub struct MockPlayer {
    state: Mutex<PlayerState>,
}

struct MockReceiver(Arc<Mutex<VecDeque<PlayerEvent>>>);

impl EventReceiver for MockReceiver {
    fn try_next(&mut self) -> Option<PlayerEvent> {
        self.0.lock().ok()?.pop_front()
    }
}

impl MockPlayer {
    /// A player with a 12-track queue, paused at the start of the first track.
    #[must_use]
    pub fn new() -> Self {
        let queue: Vec<Track> = (0..12).map(track).collect();
        let current = queue[0].clone();
        let snapshot = PlayerSnapshot {
            state: PlayState::Paused,
            duration: current.duration,
            signal_path: Some(signal_path(&current)),
            current: Some(current),
            queue,
            queue_index: Some(0),
            volume_percent: 80,
            ..PlayerSnapshot::default()
        };
        Self {
            state: Mutex::new(PlayerState {
                snapshot,
                base: Duration::ZERO,
                since: None,
                subscribers: Vec::new(),
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, PlayerState> {
        // A poisoned lock only means another test thread panicked; the data is still usable.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Default for MockPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerState {
    #[allow(clippy::needless_pass_by_value)] // events are built at the call site and moved in
    fn emit(&mut self, event: PlayerEvent) {
        self.subscribers.retain(|s| Arc::strong_count(s) > 1);
        for s in &self.subscribers {
            if let Ok(mut q) = s.lock() {
                q.push_back(event.clone());
            }
        }
    }

    fn position(&self) -> Duration {
        let run = self.since.map_or(Duration::ZERO, |t| t.elapsed());
        let pos = self.base + run;
        self.snapshot.duration.map_or(pos, |d| pos.min(d))
    }

    fn set_state(&mut self, state: PlayState) {
        if self.snapshot.state != state {
            self.snapshot.state = state;
            self.emit(PlayerEvent::StateChanged(state));
        }
    }

    /// Make queue entry `index` the current track and start it.
    fn start(&mut self, index: usize) {
        let Some(track) = self.snapshot.queue.get(index).cloned() else {
            return;
        };
        let path = signal_path(&track);
        self.snapshot.queue_index = Some(index);
        self.snapshot.duration = track.duration;
        self.snapshot.signal_path = Some(path.clone());
        let id = track.id.clone();
        self.snapshot.current = Some(track);
        self.base = Duration::ZERO;
        self.since = Some(Instant::now());
        self.emit(PlayerEvent::TrackChanged(id));
        if let Some(d) = self.snapshot.duration {
            self.emit(PlayerEvent::DurationKnown(d));
        }
        self.emit(PlayerEvent::SignalPathChanged(path));
        self.emit(PlayerEvent::QueueChanged);
        self.set_state(PlayState::Playing);
    }

    fn stop(&mut self) {
        self.snapshot.current = None;
        self.snapshot.queue_index = None;
        self.snapshot.duration = None;
        self.snapshot.signal_path = None;
        self.base = Duration::ZERO;
        self.since = None;
        self.set_state(PlayState::Stopped);
        self.emit(PlayerEvent::QueueChanged);
    }

    fn next(&mut self) {
        match self.snapshot.queue_index {
            Some(i) if i + 1 < self.snapshot.queue.len() => self.start(i + 1),
            Some(_) if self.snapshot.repeat == RepeatMode::All => self.start(0),
            _ => self.stop(),
        }
    }

    /// Move to the next track when the current one has run out; repeat One plays it again.
    fn advance(&mut self) {
        if self.snapshot.state == PlayState::Playing
            && self.snapshot.duration.is_some_and(|d| self.position() >= d)
        {
            match (self.snapshot.repeat, self.snapshot.queue_index) {
                (RepeatMode::One, Some(i)) => self.start(i),
                _ => self.next(),
            }
        }
    }

    fn apply(&mut self, command: PlayerCommand) {
        match command {
            PlayerCommand::Play(track) => {
                self.snapshot.queue = vec![track];
                self.start(0);
            }
            PlayerCommand::SetQueue { tracks, start } => {
                self.snapshot.queue = tracks;
                self.start(start);
            }
            PlayerCommand::Enqueue(track) => {
                self.snapshot.queue.push(track);
                self.emit(PlayerEvent::QueueChanged);
            }
            PlayerCommand::PlayNext(track) => {
                let at = self.snapshot.queue_index.map_or(0, |i| i + 1);
                self.snapshot
                    .queue
                    .insert(at.min(self.snapshot.queue.len()), track);
                self.emit(PlayerEvent::QueueChanged);
            }
            PlayerCommand::RemoveAt(i) => self.remove_at(i),
            PlayerCommand::Pause => {
                if self.snapshot.state == PlayState::Playing {
                    self.base = self.position();
                    self.since = None;
                    self.set_state(PlayState::Paused);
                }
            }
            PlayerCommand::Resume => match self.snapshot.state {
                PlayState::Paused => {
                    self.since = Some(Instant::now());
                    self.set_state(PlayState::Playing);
                }
                PlayState::Stopped if !self.snapshot.queue.is_empty() => self.start(0),
                _ => {}
            },
            PlayerCommand::Stop => self.stop(),
            PlayerCommand::Seek(to) => {
                if self.snapshot.current.is_some() {
                    self.base = self.snapshot.duration.map_or(to, |d| to.min(d));
                    if self.since.is_some() {
                        self.since = Some(Instant::now());
                    }
                }
            }
            PlayerCommand::Next => self.next(),
            PlayerCommand::Previous => {
                let restart = self.position() > Duration::from_secs(3);
                match self.snapshot.queue_index {
                    Some(i) if !restart && i > 0 => self.start(i - 1),
                    Some(i) => self.start(i),
                    None => {}
                }
            }
            PlayerCommand::SetVolume(v) => {
                let v = v.min(100);
                self.snapshot.volume_percent = v;
                self.emit(PlayerEvent::VolumeChanged(v));
            }
            PlayerCommand::SetSkipSegments(on) => self.snapshot.skip_segments = on,
            // The mock skips no segment, has no remote device to hand playback to and plays no
            // stream with several audio tracks.
            PlayerCommand::UndoSkip
            | PlayerCommand::Park
            | PlayerCommand::Unpark { .. }
            | PlayerCommand::SwitchAudioTrack(_) => {}
            PlayerCommand::SetRepeat(mode) => {
                self.snapshot.repeat = mode;
                self.emit(PlayerEvent::RepeatChanged(mode));
            }
        }
    }

    fn remove_at(&mut self, i: usize) {
        if i >= self.snapshot.queue.len() {
            return;
        }
        self.snapshot.queue.remove(i);
        match self.snapshot.queue_index {
            Some(cur) if cur == i => {
                if i < self.snapshot.queue.len() {
                    self.start(i);
                } else {
                    self.stop();
                }
            }
            Some(cur) if cur > i => {
                self.snapshot.queue_index = Some(cur - 1);
                self.emit(PlayerEvent::QueueChanged);
            }
            _ => self.emit(PlayerEvent::QueueChanged),
        }
    }
}

impl PlayerControl for MockPlayer {
    fn send(&self, command: PlayerCommand) {
        let mut s = self.lock();
        s.advance();
        s.apply(command);
    }

    fn position(&self) -> Duration {
        let mut s = self.lock();
        s.advance();
        s.position()
    }

    fn snapshot(&self) -> PlayerSnapshot {
        let mut s = self.lock();
        s.advance();
        s.snapshot.clone()
    }

    fn subscribe(&self) -> Box<dyn EventReceiver> {
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        self.lock().subscribers.push(Arc::clone(&queue));
        Box::new(MockReceiver(queue))
    }
}
