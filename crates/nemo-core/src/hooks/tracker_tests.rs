//! Table and property tests of [`PlayTracker`]. listening-stats reuses them.

use std::time::{Duration, Instant, SystemTime};

use proptest::prelude::*;

use super::{FinishedPlay, PlayHookEvent, PlayTracker};
use crate::audio::Origin;
use crate::player::{PlayState, PlayerEvent, PlayerSnapshot};
use crate::track::{Artwork, Track, TrackId};

fn track(id: &str, secs: u64) -> Track {
    Track {
        id: TrackId(id.to_owned()),
        title: id.to_owned(),
        artists: vec!["Artist".to_owned()],
        album: None,
        duration: Some(Duration::from_secs(secs)),
        origin: Origin::Local,
        video_id: None,
        local_path: None,
        artwork: Artwork::default(),
    }
}

const fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

struct Sim {
    tracker: PlayTracker,
    snap: PlayerSnapshot,
    t0: Instant,
    now: Duration,
    out: Vec<PlayHookEvent>,
}

impl Sim {
    fn new() -> Self {
        Self {
            tracker: PlayTracker::new(),
            snap: PlayerSnapshot::default(),
            t0: Instant::now(),
            now: Duration::ZERO,
            out: Vec::new(),
        }
    }

    fn instant(&self) -> Instant {
        self.t0 + self.now
    }

    fn event(&mut self, ev: &PlayerEvent) {
        let (now, wall) = (self.instant(), SystemTime::UNIX_EPOCH + self.now);
        self.tracker
            .on_player_event(ev, &self.snap, now, wall, &mut self.out);
    }

    fn start(&mut self, t: Track) {
        let id = t.id.clone();
        self.snap.current = Some(t);
        self.event(&PlayerEvent::TrackChanged(id));
    }

    fn state(&mut self, s: PlayState) {
        self.event(&PlayerEvent::StateChanged(s));
    }

    /// Let `wall` pass, then sample `position`.
    fn tick(&mut self, wall: Duration, position: Duration) {
        self.now += wall;
        let now = self.instant();
        self.tracker.tick(position, now, &mut self.out);
    }

    /// Play on for `secs` seconds from `from`, sampling every 500 ms.
    fn play_for(&mut self, from: Duration, secs: u64) -> Duration {
        let mut pos = from;
        for _ in 0..secs * 2 {
            pos += ms(500);
            self.tick(ms(500), pos);
        }
        pos
    }

    fn flush(&mut self) {
        let now = self.instant();
        self.tracker.flush(now, &mut self.out);
    }

    fn ended(&self) -> Vec<&FinishedPlay> {
        self.out
            .iter()
            .filter_map(|e| match e {
                PlayHookEvent::TrackEnded(p) => Some(p),
                _ => None,
            })
            .collect()
    }

    fn jumps(&self) -> usize {
        self.out
            .iter()
            .filter(|e| matches!(e, PlayHookEvent::PositionJumped { .. }))
            .count()
    }
}

#[test]
fn paused_time_is_not_heard() {
    let mut s = Sim::new();
    s.start(track("a", 180));
    s.state(PlayState::Playing);
    let pos = s.play_for(Duration::ZERO, 20);
    s.state(PlayState::Paused);
    for _ in 0..120 {
        s.tick(ms(500), pos);
    }
    s.state(PlayState::Playing);
    s.play_for(pos, 20);
    s.flush();
    let ended = s.ended();
    assert_eq!(ended.len(), 1);
    assert_eq!(ended[0].heard, Duration::from_secs(40));
    assert!(!ended[0].counts);
    assert_eq!(s.jumps(), 0);
}

#[test]
fn a_forward_seek_adds_no_heard_time() {
    let mut s = Sim::new();
    s.start(track("a", 600));
    s.state(PlayState::Playing);
    s.play_for(Duration::ZERO, 10);
    s.tick(ms(500), Duration::from_mins(2));
    assert_eq!(s.jumps(), 1);
    s.play_for(Duration::from_mins(2), 10);
    s.flush();
    assert_eq!(s.ended()[0].heard, Duration::from_secs(20));
}

#[test]
fn a_backward_seek_resyncs() {
    let mut s = Sim::new();
    s.start(track("a", 600));
    s.state(PlayState::Playing);
    s.play_for(Duration::ZERO, 30);
    s.tick(ms(500), Duration::from_secs(5));
    assert_eq!(s.jumps(), 1);
    s.play_for(Duration::from_secs(5), 10);
    s.flush();
    assert_eq!(s.ended()[0].heard, Duration::from_secs(40));
}

#[test]
fn repeat_one_closes_and_opens_a_play() {
    let mut s = Sim::new();
    let t = track("a", 40);
    s.start(t.clone());
    s.state(PlayState::Playing);
    s.play_for(Duration::ZERO, 40);
    s.start(t);
    s.play_for(Duration::ZERO, 5);
    s.flush();
    let ended = s.ended();
    assert_eq!(ended.len(), 2);
    assert!(ended[0].counts);
    assert_eq!(ended[1].heard, Duration::from_secs(5));
    let started = s
        .out
        .iter()
        .filter(|e| matches!(e, PlayHookEvent::TrackStarted { .. }))
        .count();
    assert_eq!(started, 2);
}

#[test]
fn gapless_handover_credits_the_time_up_to_the_end() {
    let mut s = Sim::new();
    s.start(track("a", 40));
    s.state(PlayState::Playing);
    let pos = s.play_for(Duration::ZERO, 39);
    s.tick(ms(500), pos + ms(500));
    // The next song starts 600 ms later; only the 500 ms left of `a` count.
    s.now += ms(600);
    s.start(track("b", 200));
    let ended = s.ended();
    assert_eq!(ended[0].track.id.0, "a");
    assert_eq!(ended[0].heard, Duration::from_secs(40));
    assert!(ended[0].counts);
    assert!(matches!(
        s.out.last(),
        Some(PlayHookEvent::TrackStarted { track, .. }) if track.id.0 == "b"
    ));
}

#[test]
fn a_skip_ends_the_play_without_counting_it() {
    let mut s = Sim::new();
    s.start(track("a", 8 * 60));
    s.state(PlayState::Playing);
    s.play_for(Duration::ZERO, 60);
    s.start(track("b", 200));
    let ended = s.ended();
    assert_eq!(ended[0].heard, Duration::from_mins(1));
    assert!(!ended[0].counts);
    assert_eq!(ended[0].duration, Some(Duration::from_mins(8)));
}

#[test]
fn duration_known_updates_the_rule() {
    let mut s = Sim::new();
    s.start(track("a", 20));
    s.event(&PlayerEvent::DurationKnown(Duration::from_mins(1)));
    s.state(PlayState::Playing);
    s.play_for(Duration::ZERO, 30);
    s.flush();
    assert!(
        s.out
            .contains(&PlayHookEvent::DurationKnown(Duration::from_mins(1)))
    );
    assert!(s.ended()[0].counts);
}

#[derive(Debug, Clone)]
enum Op {
    Tick { wall_ms: u64, position_ms: u64 },
    State(u8),
    Track(u8),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (0_u64..3_000, 0_u64..700_000)
            .prop_map(|(wall_ms, position_ms)| Op::Tick { wall_ms, position_ms }),
        1 => (0_u8..4).prop_map(Op::State),
        1 => (0_u8..3).prop_map(Op::Track),
    ]
}

proptest! {
    #[test]
    fn heard_time_never_exceeds_wall_time(ops in proptest::collection::vec(op(), 0..200)) {
        let mut s = Sim::new();
        for o in ops {
            match o {
                Op::Tick { wall_ms, position_ms } => s.tick(ms(wall_ms), ms(position_ms)),
                Op::State(n) => s.state(match n {
                    0 => PlayState::Playing,
                    1 => PlayState::Paused,
                    2 => PlayState::Buffering,
                    _ => PlayState::Stopped,
                }),
                Op::Track(n) => s.start(track(&format!("t{n}"), 60 * u64::from(n + 1))),
            }
        }
        s.flush();
        let heard: Duration = s.ended().iter().map(|p| p.heard).sum();
        prop_assert!(heard <= s.now, "heard {heard:?} > wall {:?}", s.now);
        for p in s.ended() {
            let started = p.started_at.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
            prop_assert!(p.heard <= s.now.saturating_sub(started));
        }
    }
}
