//! The hub with a fake player and recording observers.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use nemo_core::audio::Origin;
use nemo_core::hooks::{PlayHookEvent, PlayObserver};
use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent, PlayerSnapshot};
use nemo_core::services::{EventReceiver, PlayerControl};
use nemo_core::track::{Artwork, Track, TrackId};

use super::{Timing, spawn_with};

const FAST: Timing = Timing {
    loop_every: Duration::from_millis(5),
    sample_every: Duration::from_millis(10),
};

/// A player whose events, snapshot and position the test sets.
#[derive(Default)]
struct FakePlayer {
    events: Arc<Mutex<VecDeque<PlayerEvent>>>,
    snapshot: Mutex<PlayerSnapshot>,
    position: Mutex<Duration>,
}

struct FakeReceiver(Arc<Mutex<VecDeque<PlayerEvent>>>);

impl EventReceiver for FakeReceiver {
    fn try_next(&mut self) -> Option<PlayerEvent> {
        self.0.lock().expect("test lock").pop_front()
    }
}

impl PlayerControl for FakePlayer {
    fn send(&self, _command: PlayerCommand) {}
    fn position(&self) -> Duration {
        *self.position.lock().expect("test lock")
    }
    fn snapshot(&self) -> PlayerSnapshot {
        self.snapshot.lock().expect("test lock").clone()
    }
    fn subscribe(&self) -> Box<dyn EventReceiver> {
        Box::new(FakeReceiver(Arc::clone(&self.events)))
    }
}

impl FakePlayer {
    /// Make `track` current and report it, then report `Playing`.
    fn play(&self, track: &Track) {
        {
            let mut s = self.snapshot.lock().expect("test lock");
            // The tracker finds a track in the queue too, as the hub may read the snapshot
            // after the player moved on.
            s.queue.push(track.clone());
            s.current = Some(track.clone());
            s.state = PlayState::Playing;
        }
        *self.position.lock().expect("test lock") = Duration::ZERO;
        let mut q = self.events.lock().expect("test lock");
        q.push_back(PlayerEvent::TrackChanged(track.id.clone()));
        q.push_back(PlayerEvent::StateChanged(PlayState::Playing));
    }
}

/// Records every event. A gated recorder holds each event until the gate opens.
#[derive(Default)]
struct Recorder {
    seen: Mutex<Vec<PlayHookEvent>>,
    gate: Option<Arc<AtomicBool>>,
}

impl Recorder {
    fn gated(gate: &Arc<AtomicBool>) -> Self {
        Self {
            gate: Some(Arc::clone(gate)),
            ..Self::default()
        }
    }
    fn seen(&self) -> Vec<PlayHookEvent> {
        self.seen.lock().expect("test lock").clone()
    }
}

impl PlayObserver for Recorder {
    fn on_event(&self, event: &PlayHookEvent) {
        while self
            .gate
            .as_ref()
            .is_some_and(|g| !g.load(Ordering::SeqCst))
        {
            thread::sleep(Duration::from_millis(1));
        }
        self.seen.lock().expect("test lock").push(event.clone());
    }
}

fn track(n: u32) -> Track {
    Track {
        id: TrackId(format!("t{n}")),
        title: format!("Song {n}"),
        artists: vec!["Artist".into()],
        album: None,
        duration: Some(Duration::from_secs(200)),
        origin: Origin::Local,
        video_id: None,
        local_path: None,
        artwork: Artwork::None,
        source_url: None,
    }
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "timed out: {what}"
        );
        thread::sleep(Duration::from_millis(2));
    }
}

/// A short name per event kind, with the track for start and end.
fn kinds(events: &[PlayHookEvent]) -> Vec<String> {
    events
        .iter()
        .map(|e| match e {
            PlayHookEvent::TrackStarted { track, .. } => format!("start {}", track.id.0),
            PlayHookEvent::StateChanged { state, .. } => format!("{state:?}"),
            PlayHookEvent::PositionJumped { .. } => "jump".to_owned(),
            PlayHookEvent::DurationKnown(_) => "duration".to_owned(),
            PlayHookEvent::TrackEnded(f) => format!("end {}", f.track.id.0),
        })
        .collect()
}

#[test]
fn events_reach_observers_in_order_and_stop_flushes_the_last_play() {
    let player = Arc::new(FakePlayer::default());
    let observer = Arc::new(Recorder::default());
    let hub = spawn_with(
        Arc::clone(&player) as Arc<dyn PlayerControl>,
        vec![Arc::clone(&observer) as Arc<dyn PlayObserver>],
        FAST,
    )
    .expect("hub starts");
    player.play(&track(1));
    wait_until("the first play", || observer.seen().len() >= 2);
    player.play(&track(2));
    wait_until("the second play", || observer.seen().len() >= 5);
    hub.stop();
    assert_eq!(
        kinds(&observer.seen()),
        [
            "start t1", "Playing", "end t1", "start t2", "Playing", "end t2"
        ]
    );
}

#[test]
fn a_slow_observer_never_blocks_the_hub_or_the_others() {
    /// Opens the gate when dropped, so a failed assertion does not leave the stuck observer
    /// holding the hub's shutdown.
    struct OpenOnDrop(Arc<AtomicBool>);
    impl Drop for OpenOnDrop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let player = Arc::new(FakePlayer::default());
    let gate = Arc::new(AtomicBool::new(false));
    let slow = Arc::new(Recorder::gated(&gate));
    let fast = Arc::new(Recorder::default());
    let hub = spawn_with(
        Arc::clone(&player) as Arc<dyn PlayerControl>,
        vec![
            Arc::clone(&slow) as Arc<dyn PlayObserver>,
            Arc::clone(&fast) as Arc<dyn PlayObserver>,
        ],
        FAST,
    )
    .expect("hub starts");
    let opener = OpenOnDrop(Arc::clone(&gate));
    // 40 plays make 120 events, past the stuck observer's 64 slots.
    for n in 0..40 {
        player.play(&track(n));
        thread::sleep(Duration::from_millis(2));
    }
    // If the hub waited for the stuck observer, the fast one would never get here.
    wait_until("the fast observer", || fast.seen().len() >= 119);
    assert!(slow.seen().is_empty(), "the slow one is still stuck");
    // Once it moves again it gets what was queued for it; the rest was dropped.
    drop(opener);
    drop(hub);
    let got = slow.seen().len();
    assert!(
        got > 0 && got <= 1 + super::OUTLET_CAPACITY,
        "slow got {got}"
    );
    assert_eq!(fast.seen().len(), 120, "the flush adds the last TrackEnded");
}

#[test]
fn position_samples_count_heard_time() {
    let player = Arc::new(FakePlayer::default());
    let observer = Arc::new(Recorder::default());
    let hub = spawn_with(
        Arc::clone(&player) as Arc<dyn PlayerControl>,
        vec![Arc::clone(&observer) as Arc<dyn PlayObserver>],
        FAST,
    )
    .expect("hub starts");
    player.play(&track(1));
    wait_until("the play", || observer.seen().len() >= 2);
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(300) {
        *player.position.lock().expect("test lock") = start.elapsed();
        thread::sleep(Duration::from_millis(5));
    }
    hub.stop();
    let Some(PlayHookEvent::TrackEnded(f)) = observer.seen().last().cloned() else {
        panic!("the flush ends the play");
    };
    assert!(f.heard >= Duration::from_millis(200), "heard {:?}", f.heard);
    assert!(f.heard <= Duration::from_millis(400), "heard {:?}", f.heard);
    assert!(!f.counts, "0.3 s of a 200 s song does not count");
}
