//! The streaming player: one control thread owns the queue and the output, everything else
//! talks to it through [`PlayerControl`].

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use nemo_core::library::LocalTrackLookup;
use nemo_core::player::{PlayerCommand, PlayerEvent, PlayerSnapshot};
use nemo_core::segment::{Segment, SegmentSource};
use nemo_core::services::{EventReceiver, PlayerControl, StreamOpener};
use nemo_core::stream::{StreamRequest, StreamResolver};

use crate::control::Core;
use crate::prepare::Prepared;
use crate::replay_gain::ReplayGainSwitch;
use crate::sink::SinkFactory;

/// Everything the player needs from the rest of the app.
#[derive(Clone)]
pub struct PlayerDeps {
    /// Turns a video id into a stream.
    pub resolver: Arc<dyn StreamResolver>,
    /// What every stream is resolved with (audio language, Premium, codec hint). The prepare
    /// thread passes it to [`StreamResolver::resolve_with`]; the audio callback never reads it.
    /// [`StreamRequest::default`] resolves exactly as [`StreamResolver::resolve`] does.
    pub request: StreamRequest,
    /// Opens a resolved stream for reading.
    pub opener: Arc<dyn StreamOpener>,
    /// SponsorBlock segments. `None` switches skipping off for good.
    pub segments: Option<Arc<dyn SegmentSource>>,
    /// Opens the output sink.
    pub sink_factory: Arc<dyn SinkFactory>,
    /// Span and gain of local library tracks (a CUE track plays one span of its file). Asked on
    /// the prepare thread only, never in the audio callback. `None` plays every file whole.
    pub local: Option<Arc<dyn LocalTrackLookup>>,
}

/// Messages to the control thread.
// A command (a whole `Track`) is the largest variant; each message is moved once through the
// channel, so boxing it would only add an allocation.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Msg {
    /// A command from the UI.
    Cmd(PlayerCommand),
    /// A helper thread finished opening a track.
    Prepared {
        token: u64,
        result: Result<Box<Prepared>, String>,
    },
    /// A helper thread fetched SponsorBlock segments.
    Segments { token: u64, segments: Vec<Segment> },
    /// The last handle was dropped.
    Shutdown,
}

/// Position bookkeeping that `position()` reads without a lock.
pub(crate) struct PosShared {
    /// Output frames handed to the device since the last flush (written by the callback).
    pub played: Arc<AtomicU64>,
    /// Value of `played` when the current track began. `u64::MAX` while unknown.
    pub start: AtomicU64,
    /// Track position at `start`, in microseconds.
    pub base_us: AtomicU64,
    /// Output latency in frames.
    pub latency: AtomicU64,
    /// Output sample rate.
    pub rate: AtomicU32,
    /// Track duration in microseconds, `0` when unknown.
    pub duration_us: AtomicU64,
    /// A track is current.
    pub active: AtomicBool,
}

/// State shared between the control thread and the handle.
pub(crate) struct Shared {
    pub snapshot: Mutex<PlayerSnapshot>,
    subscribers: Mutex<Vec<Sender<PlayerEvent>>>,
    pub pos: PosShared,
    /// The `ReplayGain` setting, read each time a song is handed to the feeder.
    pub replay_gain: ReplayGainSwitch,
}

impl Shared {
    fn new(replay_gain: ReplayGainSwitch) -> Self {
        Self {
            snapshot: Mutex::new(PlayerSnapshot::default()),
            subscribers: Mutex::new(Vec::new()),
            pos: PosShared {
                played: Arc::new(AtomicU64::new(0)),
                start: AtomicU64::new(u64::MAX),
                base_us: AtomicU64::new(0),
                latency: AtomicU64::new(0),
                rate: AtomicU32::new(1),
                duration_us: AtomicU64::new(0),
                active: AtomicBool::new(false),
            },
            replay_gain,
        }
    }

    /// Send an event to every subscriber, dropping the ones that went away.
    pub fn emit(&self, event: &PlayerEvent) {
        if let Ok(mut subs) = self.subscribers.lock() {
            subs.retain(|s| s.send(event.clone()).is_ok());
        }
    }

    /// Position in the current track at the speakers.
    pub fn position(&self) -> Duration {
        let p = &self.pos;
        if !p.active.load(Ordering::SeqCst) {
            return Duration::ZERO;
        }
        let frames = p
            .played
            .load(Ordering::SeqCst)
            .saturating_sub(p.start.load(Ordering::SeqCst))
            .saturating_sub(p.latency.load(Ordering::SeqCst));
        let rate = f64::from(p.rate.load(Ordering::SeqCst).max(1));
        let pos = Duration::from_micros(p.base_us.load(Ordering::SeqCst))
            + Duration::from_secs_f64(crate::units::f64_of(frames) / rate);
        match p.duration_us.load(Ordering::SeqCst) {
            0 => pos,
            d => pos.min(Duration::from_micros(d)),
        }
    }
}

struct Subscriber(Receiver<PlayerEvent>);

impl EventReceiver for Subscriber {
    fn try_next(&mut self) -> Option<PlayerEvent> {
        self.0.try_recv().ok()
    }
}

/// The handle the UI holds.
struct Handle {
    tx: Sender<Msg>,
    shared: Arc<Shared>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl PlayerControl for Handle {
    fn send(&self, command: PlayerCommand) {
        // The control thread is gone only after shutdown; there is nobody to tell.
        let _ = self.tx.send(Msg::Cmd(command));
    }

    fn position(&self) -> Duration {
        self.shared.position()
    }

    fn snapshot(&self) -> PlayerSnapshot {
        self.shared
            .snapshot
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default()
    }

    fn subscribe(&self) -> Box<dyn EventReceiver> {
        let (tx, rx) = channel();
        if let Ok(mut subs) = self.shared.subscribers.lock() {
            subs.push(tx);
        }
        Box::new(Subscriber(rx))
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // The control thread is gone only after shutdown; there is nobody to tell.
        let _ = self.tx.send(Msg::Shutdown);
        if let Ok(mut t) = self.thread.lock()
            && let Some(handle) = t.take()
        {
            // A panicked control thread has already stopped the player; nothing to recover.
            let _ = handle.join();
        }
    }
}

/// The streaming player.
pub struct Player;

impl Player {
    /// Start the control thread and return the handle that drives it. Dropping the last clone of
    /// the handle stops the player and releases the output device. `ReplayGain` stays off; use
    /// [`Player::spawn_with_replay_gain`] to change it later.
    ///
    /// # Panics
    /// Panics if the operating system refuses to start a thread.
    #[must_use]
    pub fn spawn(deps: PlayerDeps) -> Arc<dyn PlayerControl> {
        Self::spawn_with_replay_gain(deps, ReplayGainSwitch::default())
    }

    /// As [`Player::spawn`], reading the `ReplayGain` setting from `replay_gain`. The app keeps
    /// a clone and sets it; a change applies from the next song handed to the feeder.
    ///
    /// # Panics
    /// Panics if the operating system refuses to start a thread.
    #[must_use]
    pub fn spawn_with_replay_gain(
        deps: PlayerDeps,
        replay_gain: ReplayGainSwitch,
    ) -> Arc<dyn PlayerControl> {
        let shared = Arc::new(Shared::new(replay_gain));
        let (tx, rx) = channel();
        let (core_shared, core_tx) = (Arc::clone(&shared), tx.clone());
        // The core holds the output sink, which may not be Send, so it is built on its thread.
        let thread = thread::Builder::new()
            .name("nemo-player".into())
            .spawn(move || Core::new(deps, core_shared, core_tx).run(&rx))
            .expect("invariant: the OS can start one thread");
        Arc::new(Handle {
            tx,
            shared,
            thread: Mutex::new(Some(thread)),
        })
    }
}
