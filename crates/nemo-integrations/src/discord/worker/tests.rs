use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use nemo_core::audio::Origin;
use nemo_core::hooks::PlayHookEvent;
use nemo_core::player::PlayState;
use nemo_core::track::{Artwork, Track, TrackId};

use super::{Config, Mailbox, Msg, Worker};
use crate::discord::Activity;
use crate::discord::engine::{Input, Timing};
use crate::discord::ipc::{Connector, Incoming, Link};

/// What the fake Discord saw: `Some(details)` for a set, `None` for a clear.
type Seen = Arc<Mutex<Vec<Option<String>>>>;

#[derive(Default)]
struct Fake {
    seen: Seen,
    /// Connects that fail before one works.
    refuse: AtomicUsize,
    opens: AtomicUsize,
    /// The number of the next set that fails with a lost connection (1-based), if any.
    break_at: AtomicUsize,
    sets: Arc<AtomicUsize>,
    closed: Arc<AtomicUsize>,
}

struct FakeLink {
    seen: Seen,
    sets: Arc<AtomicUsize>,
    break_at: usize,
    closed: Arc<AtomicUsize>,
}

impl Link for FakeLink {
    fn set_activity(&mut self, activity: Option<&Activity>) -> io::Result<Incoming> {
        let n = self.sets.fetch_add(1, Ordering::SeqCst) + 1;
        if self.break_at != 0 && n == self.break_at {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "gone"));
        }
        self.seen
            .lock()
            .expect("lock")
            .push(activity.map(|a| a.details.clone()));
        Ok(Incoming::Other)
    }

    fn close(self: Box<Self>) {
        self.closed.fetch_add(1, Ordering::SeqCst);
    }
}

impl Connector for Fake {
    fn open(&self, _client_id: &str) -> io::Result<Box<dyn Link>> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        let left = self.refuse.load(Ordering::SeqCst);
        if left > 0 {
            self.refuse.store(left - 1, Ordering::SeqCst);
            return Err(io::Error::new(io::ErrorKind::NotFound, "no Discord"));
        }
        Ok(Box::new(FakeLink {
            seen: Arc::clone(&self.seen),
            sets: Arc::clone(&self.sets),
            break_at: self.break_at.swap(0, Ordering::SeqCst),
            closed: Arc::clone(&self.closed),
        }))
    }
}

fn config(fake: &Arc<Fake>) -> Config {
    Config {
        client_id: "1".into(),
        timing: Timing {
            min_gap: Duration::from_millis(40),
            retry: Duration::from_millis(60),
            away_after: Duration::from_secs(600),
        },
        connector: Arc::clone(fake) as Arc<dyn Connector>,
    }
}

fn wait_for(what: &str, mut ok: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(5);
    while !ok() {
        assert!(Instant::now() < until, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn seen(fake: &Fake) -> Vec<Option<String>> {
    fake.seen.lock().expect("lock").clone()
}

fn play(title: &str) -> [Input; 2] {
    let track = Track {
        id: TrackId("t".into()),
        title: title.into(),
        artists: vec!["Artist".into()],
        album: None,
        duration: Some(Duration::from_secs(100)),
        origin: Origin::YoutubeMusic,
        video_id: None,
        local_path: None,
        artwork: Artwork::None,
        source_url: None,
    };
    [
        Input::Play(PlayHookEvent::TrackStarted {
            track,
            started_at: SystemTime::UNIX_EPOCH,
        }),
        Input::Play(PlayHookEvent::StateChanged {
            state: PlayState::Playing,
            position: Duration::ZERO,
        }),
    ]
}

#[test]
fn the_worker_connects_shows_idle_then_the_song_and_clears_on_stop() {
    let fake = Arc::new(Fake::default());
    let worker = Worker::start(config(&fake)).expect("start");
    wait_for("idle", || seen(&fake) == [Some("Idle".to_owned())]);
    for input in play("Song") {
        worker.send(input);
    }
    wait_for("the song", || {
        seen(&fake).last() == Some(&Some("Song".to_owned()))
    });
    worker.stop(Duration::from_secs(5));
    assert_eq!(seen(&fake).last(), Some(&None), "cleared on stop");
    assert_eq!(fake.closed.load(Ordering::SeqCst), 1);
}

#[test]
fn updates_inside_the_slot_are_spaced_and_the_newest_wins() {
    let fake = Arc::new(Fake::default());
    let worker = Worker::start(config(&fake)).expect("start");
    wait_for("idle", || seen(&fake).len() == 1);
    let t0 = Instant::now();
    for title in ["One", "Two", "Three"] {
        for input in play(title) {
            worker.send(input);
        }
    }
    wait_for("the last song", || {
        seen(&fake).last() == Some(&Some("Three".to_owned()))
    });
    worker.stop(Duration::from_secs(5));
    let all = seen(&fake);
    // Idle, maybe the first song at once, and the last: "Two" never goes out on its own.
    assert!(!all.contains(&Some("Two".to_owned())), "{all:?}");
    assert!(t0.elapsed() >= Duration::from_millis(30));
}

#[test]
fn while_discord_is_not_running_it_retries_and_then_shows_the_state() {
    let fake = Arc::new(Fake::default());
    fake.refuse.store(2, Ordering::SeqCst);
    let worker = Worker::start(config(&fake)).expect("start");
    wait_for("the third connect", || {
        fake.opens.load(Ordering::SeqCst) >= 3
    });
    wait_for("idle after connecting", || !seen(&fake).is_empty());
    assert_eq!(fake.opens.load(Ordering::SeqCst), 3);
    worker.stop(Duration::from_secs(5));
}

#[test]
fn a_lost_connection_is_reopened_and_the_state_is_sent_again() {
    let fake = Arc::new(Fake::default());
    // The second set (the first update after Idle) breaks the connection.
    fake.break_at.store(2, Ordering::SeqCst);
    let worker = Worker::start(config(&fake)).expect("start");
    wait_for("idle", || seen(&fake).len() == 1);
    for input in play("Song") {
        worker.send(input);
    }
    wait_for("reconnect", || fake.opens.load(Ordering::SeqCst) >= 2);
    wait_for("the song after reconnecting", || {
        seen(&fake).last() == Some(&Some("Song".to_owned()))
    });
    worker.stop(Duration::from_secs(5));
}

#[test]
fn turning_off_by_input_clears_and_a_stop_after_that_is_quiet() {
    let fake = Arc::new(Fake::default());
    let worker = Worker::start(config(&fake)).expect("start");
    wait_for("idle", || seen(&fake).len() == 1);
    worker.send(Input::Switch(false));
    wait_for("clear", || seen(&fake).last() == Some(&None));
    for input in play("Song") {
        worker.send(input);
    }
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(seen(&fake).len(), 2, "nothing is sent while off");
    worker.stop(Duration::from_secs(5));
}

#[test]
fn the_mailbox_drops_the_oldest_play_event_and_keeps_control_inputs() {
    let mailbox = Mailbox::new(3);
    let tick = || Msg::Input(Input::Tick);
    mailbox.push(Msg::Input(Input::Switch(true)));
    for input in play("A") {
        mailbox.push(Msg::Input(input));
    }
    mailbox.push(tick());
    mailbox.push(Msg::Exit);
    // Full at 3: pushing dropped the oldest play event each time, never the switch.
    let mut kinds = Vec::new();
    while let Some(msg) = mailbox.pop(Some(Duration::ZERO)) {
        kinds.push(match msg {
            Msg::Input(Input::Switch(_)) => "switch",
            Msg::Input(Input::Play(_)) => "play",
            Msg::Input(_) => "other",
            Msg::Exit => "exit",
        });
    }
    assert_eq!(kinds, ["switch", "other", "exit"]);
}

#[test]
fn a_pop_with_nothing_waiting_times_out() {
    let mailbox = Mailbox::new(3);
    let t0 = Instant::now();
    assert!(mailbox.pop(Some(Duration::from_millis(30))).is_none());
    assert!(t0.elapsed() >= Duration::from_millis(25));
}
