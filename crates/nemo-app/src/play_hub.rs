//! The play hub: one thread that watches the player and hands play events to the observers
//! (integrations design, Data Flow).
//!
//! The hub drains a `PlayerControl::subscribe` receiver, samples `position()` every 500 ms,
//! feeds the `nemo-core` `PlayTracker` and passes each `PlayHookEvent` to every registered
//! `PlayObserver`. Each observer sits behind its own bounded channel and thread, so the hub only
//! ever does a non-blocking `try_send`: a full channel drops the event, and a slow observer
//! never holds up the hub or the other observers. At exit the hub flushes the tracker, so the
//! last play ends with a `TrackEnded`. Nothing here runs in the audio callback; the hub only
//! reads the player's atomics and snapshot.
//!
//! While a remote plays (Cast, a parked player), the local position does not move. Given a
//! `RemotePlayback`, the hub then samples `remote_status()` instead of `position()`, and takes the
//! play state from the remote too, so the OS session and counted plays follow the remote device
//! (criterion 5). In `RemoteMode::Local` and in a room guest's own playback nothing changes.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use nemo_core::hooks::{PlayHookEvent, PlayObserver, PlayTracker};
use nemo_core::player::{PlayState, PlayerEvent, PlayerSnapshot};
use nemo_core::remote::{RemoteMode, RemotePlayback};
use nemo_core::services::{EventReceiver, PlayerControl};

/// Events an observer may have waiting before the hub drops new ones for it.
const OUTLET_CAPACITY: usize = 64;

/// How often the hub wakes and how often it samples the position.
#[derive(Debug, Clone, Copy)]
pub struct Timing {
    /// Wake-up interval: player events are read at least this often.
    pub loop_every: Duration,
    /// Position sample interval.
    pub sample_every: Duration,
}

impl Default for Timing {
    /// Wake every 250 ms and sample the position every 500 ms, so the OS session follows the
    /// player within a second (integrations criterion 5).
    fn default() -> Self {
        Self {
            loop_every: Duration::from_millis(250),
            sample_every: Duration::from_millis(500),
        }
    }
}

/// The running hub. [`PlayHub::stop`] flushes the last play and ends the threads.
pub struct PlayHub {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl PlayHub {
    /// Stop the hub: the open play ends with a `TrackEnded`, every observer gets the events
    /// already queued for it, then the threads end.
    pub fn stop(mut self) {
        self.shut_down();
    }

    fn shut_down(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            if thread.join().is_err() {
                tracing::warn!("the play hub thread panicked");
            }
        }
    }
}

impl Drop for PlayHub {
    fn drop(&mut self) {
        self.shut_down();
    }
}

/// One observer behind its own channel and thread.
struct Outlet {
    tx: SyncSender<PlayHookEvent>,
    thread: JoinHandle<()>,
    warned: bool,
}

/// Start the hub with the default [`Timing`].
///
/// # Errors
/// Returns the I/O error when a thread cannot be started.
pub fn spawn(
    player: Arc<dyn PlayerControl>,
    observers: Vec<Arc<dyn PlayObserver>>,
) -> std::io::Result<PlayHub> {
    spawn_with(player, observers, Timing::default())
}

/// Start the hub with `timing`.
///
/// # Errors
/// Returns the I/O error when a thread cannot be started.
pub fn spawn_with(
    player: Arc<dyn PlayerControl>,
    observers: Vec<Arc<dyn PlayObserver>>,
    timing: Timing,
) -> std::io::Result<PlayHub> {
    spawn_with_remote(player, None, observers, timing)
}

/// Start the hub with `timing`, following `remote` while it is not local.
///
/// # Errors
/// Returns the I/O error when a thread cannot be started.
pub fn spawn_with_remote(
    player: Arc<dyn PlayerControl>,
    remote: Option<Arc<dyn RemotePlayback>>,
    observers: Vec<Arc<dyn PlayObserver>>,
    timing: Timing,
) -> std::io::Result<PlayHub> {
    // Subscribe here, so no event between this call and the thread start is lost.
    let events = player.subscribe();
    let mut outlets = Vec::with_capacity(observers.len());
    for (n, observer) in observers.into_iter().enumerate() {
        let (tx, rx) = sync_channel(OUTLET_CAPACITY);
        let thread = thread::Builder::new()
            .name(format!("play-observer-{n}"))
            .spawn(move || forward(&rx, observer.as_ref()))?;
        outlets.push(Outlet {
            tx,
            thread,
            warned: false,
        });
    }
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    let thread = thread::Builder::new()
        .name("play-hub".into())
        .spawn(move || {
            let mut follow = Sampler::new(remote, timing);
            run(player.as_ref(), &mut follow, events, outlets, &flag);
        })?;
    Ok(PlayHub {
        stop,
        thread: Some(thread),
    })
}

/// An observer thread: hand each event over until the hub closes the channel.
fn forward(rx: &Receiver<PlayHookEvent>, observer: &dyn PlayObserver) {
    while let Ok(event) = rx.recv() {
        observer.on_event(&event);
    }
}

/// Takes the position samples. Follows the remote playback, when there is one, and remembers
/// what it told the tracker.
struct Sampler {
    remote: Option<Arc<dyn RemotePlayback>>,
    /// The state last fed to the tracker from the remote; `None` while playback is local.
    fed: Option<PlayState>,
    timing: Timing,
}

impl Sampler {
    fn new(remote: Option<Arc<dyn RemotePlayback>>, timing: Timing) -> Self {
        Self {
            remote,
            fed: None,
            timing,
        }
    }

    /// The remote's status, only while playback is not local.
    fn active(&self) -> Option<nemo_core::remote::RemoteStatus> {
        let status = self.remote.as_ref()?.remote_status();
        (status.mode != RemoteMode::Local).then_some(status)
    }

    /// Take one position sample: the remote's while it plays, the player's otherwise.
    fn sample(
        &mut self,
        player: &dyn PlayerControl,
        tracker: &mut PlayTracker,
        now: Instant,
        out: &mut Vec<PlayHookEvent>,
    ) {
        let (position, state) = match self.active() {
            Some(status) => (status.position, Some(status.state)),
            None => (Some(player.position()), None),
        };
        match (state, self.fed) {
            (Some(state), fed) if fed != Some(state) => {
                self.fed = Some(state);
                feed_state(tracker, state, now, out);
            }
            (None, Some(_)) => {
                // Back to local playback: take the player's own state again.
                self.fed = None;
                feed_state(tracker, player.snapshot().state, now, out);
            }
            _ => {}
        }
        if let Some(position) = position {
            tracker.tick(position, now, out);
        }
    }
}

fn feed_state(
    tracker: &mut PlayTracker,
    state: PlayState,
    now: Instant,
    out: &mut Vec<PlayHookEvent>,
) {
    tracker.on_player_event(
        &PlayerEvent::StateChanged(state),
        &PlayerSnapshot::default(),
        now,
        SystemTime::now(),
        out,
    );
}

/// The hub thread body.
fn run(
    player: &dyn PlayerControl,
    follow: &mut Sampler,
    mut events: Box<dyn EventReceiver>,
    mut outlets: Vec<Outlet>,
    stop: &AtomicBool,
) {
    let timing = follow.timing;
    let mut tracker = PlayTracker::new();
    let mut out = Vec::new();
    let mut next_sample = Instant::now() + timing.sample_every;
    loop {
        let stopping = stop.load(Ordering::SeqCst);
        while let Some(event) = events.try_next() {
            // While a remote plays, its state wins over the parked player's own.
            if matches!(event, PlayerEvent::StateChanged(_)) && follow.active().is_some() {
                continue;
            }
            let snapshot = player.snapshot();
            tracker.on_player_event(
                &event,
                &snapshot,
                Instant::now(),
                SystemTime::now(),
                &mut out,
            );
        }
        let now = Instant::now();
        if now >= next_sample {
            follow.sample(player, &mut tracker, now, &mut out);
            next_sample = now + timing.sample_every;
        }
        if stopping {
            tracker.flush(Instant::now(), &mut out);
            deliver(&mut outlets, &mut out);
            break;
        }
        deliver(&mut outlets, &mut out);
        thread::park_timeout(timing.loop_every);
    }
    for outlet in outlets {
        drop(outlet.tx);
        if outlet.thread.join().is_err() {
            tracing::warn!("a play observer panicked");
        }
    }
}

/// Hand every event to every outlet without blocking; a full outlet drops it.
fn deliver(outlets: &mut [Outlet], out: &mut Vec<PlayHookEvent>) {
    for event in out.drain(..) {
        for outlet in outlets.iter_mut() {
            match outlet.tx.try_send(event.clone()) {
                Ok(()) | Err(TrySendError::Disconnected(_)) => {}
                Err(TrySendError::Full(_)) => {
                    if !outlet.warned {
                        outlet.warned = true;
                        tracing::warn!("a play observer is behind; dropping its events");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
