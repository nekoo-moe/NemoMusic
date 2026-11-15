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

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use nemo_core::hooks::{PlayHookEvent, PlayObserver, PlayTracker};
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
        .spawn(move || run(player.as_ref(), events, outlets, &flag, timing))?;
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

/// The hub thread body.
fn run(
    player: &dyn PlayerControl,
    mut events: Box<dyn EventReceiver>,
    mut outlets: Vec<Outlet>,
    stop: &AtomicBool,
    timing: Timing,
) {
    let mut tracker = PlayTracker::new();
    let mut out = Vec::new();
    let mut next_sample = Instant::now() + timing.sample_every;
    loop {
        let stopping = stop.load(Ordering::SeqCst);
        while let Some(event) = events.try_next() {
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
            tracker.tick(player.position(), now, &mut out);
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
