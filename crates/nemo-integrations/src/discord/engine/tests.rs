use std::time::{Duration, Instant, SystemTime};

use nemo_core::audio::Origin;
use nemo_core::hooks::PlayHookEvent;
use nemo_core::player::PlayState;
use nemo_core::track::{Artwork, Track, TrackId};

use super::{Action, Engine, Input, Timing};
use crate::discord::{Place, PresenceState, build};

const WALL: i64 = 1_000_000;

struct Clock {
    t0: Instant,
    secs: u64,
}

impl Clock {
    fn new() -> Self {
        Self {
            t0: Instant::now(),
            secs: 0,
        }
    }

    fn at(&mut self, secs: u64) -> Instant {
        self.secs = secs;
        self.t0 + Duration::from_secs(secs)
    }

    fn wall(&self) -> i64 {
        WALL + i64::try_from(self.secs).unwrap_or(0)
    }
}

fn step(e: &mut Engine, c: &mut Clock, at: u64, input: Input) -> Vec<Action> {
    let now = c.at(at);
    e.step(now, c.wall(), input)
}

fn track() -> Track {
    Track {
        id: TrackId("t".into()),
        title: "Song".into(),
        artists: vec!["Artist".into()],
        album: None,
        duration: Some(Duration::from_secs(200)),
        origin: Origin::YoutubeMusic,
        video_id: None,
        local_path: None,
        artwork: Artwork::None,
        source_url: None,
    }
}

fn started() -> Input {
    Input::Play(PlayHookEvent::TrackStarted {
        track: track(),
        started_at: SystemTime::UNIX_EPOCH,
    })
}

fn state(state: PlayState, secs: u64) -> Input {
    Input::Play(PlayHookEvent::StateChanged {
        state,
        position: Duration::from_secs(secs),
    })
}

fn jump(secs: u64) -> Input {
    Input::Play(PlayHookEvent::PositionJumped {
        position: Duration::from_secs(secs),
    })
}

fn sets(actions: &[Action]) -> Vec<&str> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::Set(act) => Some(act.details.as_str()),
            _ => None,
        })
        .collect()
}

/// An engine that is on and connected.
fn ready(c: &mut Clock) -> Engine {
    let mut e = Engine::new(Timing::default(), WALL);
    assert_eq!(step(&mut e, c, 0, Input::Switch(true)), [Action::Connect]);
    let first = step(&mut e, c, 0, Input::Connected(true));
    assert_eq!(sets(&first), ["Idle"]);
    e
}

#[test]
fn while_off_nothing_is_asked_of_anyone() {
    let mut c = Clock::new();
    let mut e = Engine::new(Timing::default(), WALL);
    for input in [
        started(),
        state(PlayState::Playing, 0),
        Input::Window { active: false },
        Input::Tick,
    ] {
        assert!(step(&mut e, &mut c, 1, input).is_empty());
    }
    assert_eq!(e.wake_in(c.at(2)), None);
    // Turning it off when it was off does nothing, not even a clear.
    assert!(step(&mut e, &mut c, 3, Input::Switch(false)).is_empty());
}

#[test]
fn turning_on_connects_and_the_first_activity_goes_at_once() {
    let mut c = Clock::new();
    let mut e = Engine::new(Timing::default(), WALL);
    assert_eq!(
        step(&mut e, &mut c, 0, Input::Switch(true)),
        [Action::Connect]
    );
    // A tick while the attempt runs does not start a second one.
    assert!(step(&mut e, &mut c, 1, Input::Tick).is_empty());
    let first = step(&mut e, &mut c, 1, Input::Connected(true));
    assert_eq!(first.len(), 1);
    assert_eq!(
        first,
        [Action::Set(Box::new(build(
            &PresenceState::Idle { since: WALL },
            false
        )))]
    );
}

#[test]
fn a_change_inside_the_slot_waits_and_the_newest_wins() {
    let mut c = Clock::new();
    let mut e = ready(&mut c);
    // Idle went out at t=0. The song starts at t=2: held until t=15.
    assert!(step(&mut e, &mut c, 2, started()).is_empty());
    assert!(step(&mut e, &mut c, 2, state(PlayState::Playing, 0)).is_empty());
    assert_eq!(e.wake_in(c.at(5)), Some(Duration::from_secs(10)));
    // A seek at t=9 replaces what waits.
    assert!(step(&mut e, &mut c, 9, jump(100)).is_empty());
    assert!(step(&mut e, &mut c, 14, Input::Tick).is_empty());
    let sent = step(&mut e, &mut c, 15, Input::Tick);
    assert_eq!(sets(&sent), ["Song"]);
    let Action::Set(a) = &sent[0] else {
        panic!("not a set")
    };
    // Seeked to 100 s at wall WALL + 9: the start is 100 s earlier.
    assert_eq!(a.start, Some(WALL + 9 - 100));
    assert_eq!(a.end, Some(WALL + 9 - 100 + 200));
    assert_eq!(e.wake_in(c.at(16)), None);
}

#[test]
fn an_update_equal_to_the_last_one_is_not_sent_again() {
    let mut c = Clock::new();
    let mut e = ready(&mut c);
    step(&mut e, &mut c, 20, started());
    let sent = step(&mut e, &mut c, 20, state(PlayState::Playing, 0));
    assert_eq!(sets(&sent), ["Song"]);
    // The same facts again, a minute later (the position sample moved with the clock).
    assert!(step(&mut e, &mut c, 80, Input::Tick).is_empty());
    assert!(step(&mut e, &mut c, 80, Input::Window { active: true }).is_empty());
}

#[test]
fn a_change_that_is_undone_before_its_slot_sends_nothing() {
    let mut c = Clock::new();
    let mut e = ready(&mut c);
    step(&mut e, &mut c, 20, started());
    step(&mut e, &mut c, 20, state(PlayState::Playing, 0));
    // Pause at t=22 (held), resume at t=24 at the same start time (anchor 20 + 4 s position).
    assert!(step(&mut e, &mut c, 22, state(PlayState::Paused, 2)).is_empty());
    assert!(step(&mut e, &mut c, 24, state(PlayState::Playing, 4)).is_empty());
    assert!(step(&mut e, &mut c, 40, Input::Tick).is_empty());
    assert_eq!(e.wake_in(c.at(40)), None);
}

#[test]
fn a_lost_connection_is_retried_every_thirty_seconds_and_the_activity_comes_back() {
    let mut c = Clock::new();
    let mut e = ready(&mut c);
    assert!(step(&mut e, &mut c, 10, Input::Connected(false)).is_empty());
    assert_eq!(e.wake_in(c.at(10)), Some(Duration::from_secs(30)));
    assert!(step(&mut e, &mut c, 39, Input::Tick).is_empty());
    assert_eq!(step(&mut e, &mut c, 40, Input::Tick), [Action::Connect]);
    // The attempt fails: the next one is 30 s later.
    assert!(step(&mut e, &mut c, 41, Input::Connected(false)).is_empty());
    assert!(step(&mut e, &mut c, 70, Input::Tick).is_empty());
    assert_eq!(step(&mut e, &mut c, 71, Input::Tick), [Action::Connect]);
    // It works: the current state goes out at once, not after a slot.
    let back = step(&mut e, &mut c, 72, Input::Connected(true));
    assert_eq!(sets(&back), ["Idle"]);
}

#[test]
fn turning_off_clears_disconnects_and_goes_quiet() {
    let mut c = Clock::new();
    let mut e = ready(&mut c);
    let off = step(&mut e, &mut c, 5, Input::Switch(false));
    assert_eq!(off, [Action::Clear, Action::Disconnect]);
    assert!(step(&mut e, &mut c, 6, started()).is_empty());
    assert!(step(&mut e, &mut c, 60, Input::Tick).is_empty());
    assert_eq!(e.wake_in(c.at(60)), None);
}

#[test]
fn turning_off_before_discord_answered_only_disconnects() {
    let mut c = Clock::new();
    let mut e = Engine::new(Timing::default(), WALL);
    step(&mut e, &mut c, 0, Input::Switch(true));
    assert_eq!(
        step(&mut e, &mut c, 1, Input::Switch(false)),
        [Action::Disconnect]
    );
    // A late answer of the abandoned attempt changes nothing.
    assert!(step(&mut e, &mut c, 2, Input::Connected(true)).is_empty());
}

#[test]
fn ten_quiet_minutes_out_of_focus_show_away_and_the_focus_leaves_it_at_once() {
    let mut c = Clock::new();
    let mut e = ready(&mut c);
    assert!(step(&mut e, &mut c, 30, Input::Window { active: false }).is_empty());
    assert_eq!(e.wake_in(c.at(30)), Some(Duration::from_mins(10)));
    assert!(step(&mut e, &mut c, 629, Input::Tick).is_empty());
    let away = step(&mut e, &mut c, 630, Input::Tick);
    assert_eq!(sets(&away), ["Away"]);
    // Back at once, although the last update was 3 s ago.
    let back = step(&mut e, &mut c, 633, Input::Window { active: true });
    assert_eq!(sets(&back), ["Idle"]);
}

#[test]
fn a_song_that_plays_in_a_hidden_window_never_goes_away() {
    let mut c = Clock::new();
    let mut e = ready(&mut c);
    step(&mut e, &mut c, 20, Input::Window { active: false });
    step(&mut e, &mut c, 20, started());
    step(&mut e, &mut c, 20, state(PlayState::Playing, 0));
    assert!(step(&mut e, &mut c, 4_000, Input::Tick).is_empty());
}

#[test]
fn a_paused_song_in_a_hidden_window_goes_away_after_ten_minutes() {
    let mut c = Clock::new();
    let mut e = ready(&mut c);
    step(&mut e, &mut c, 20, started());
    step(&mut e, &mut c, 20, state(PlayState::Playing, 0));
    step(&mut e, &mut c, 100, Input::Window { active: false });
    step(&mut e, &mut c, 100, state(PlayState::Paused, 80));
    let away = step(&mut e, &mut c, 700, Input::Tick);
    assert_eq!(sets(&away), ["Away"]);
}

#[test]
fn the_private_switch_rebuilds_what_is_shown() {
    let mut c = Clock::new();
    let mut e = ready(&mut c);
    step(&mut e, &mut c, 20, started());
    let playing = step(&mut e, &mut c, 20, state(PlayState::Playing, 0));
    assert_eq!(sets(&playing), ["Song"]);
    let private = step(&mut e, &mut c, 36, Input::Private(true));
    assert_eq!(sets(&private), ["Listening to NemoMusic"]);
    let public = step(&mut e, &mut c, 60, Input::Private(false));
    assert_eq!(sets(&public), ["Song"]);
}

#[test]
fn place_and_section_change_the_activity() {
    let mut c = Clock::new();
    let mut e = ready(&mut c);
    let browsing = step(
        &mut e,
        &mut c,
        20,
        Input::Section(Some(crate::discord::Browsing::Library)),
    );
    assert_eq!(sets(&browsing), ["Browsing Library"]);
    step(&mut e, &mut c, 40, started());
    step(&mut e, &mut c, 40, state(PlayState::Playing, 0));
    let cast = step(&mut e, &mut c, 60, Input::Place(Place::Cast));
    let Action::Set(a) = &cast[0] else {
        panic!("not a set")
    };
    assert_eq!(
        a.large.as_ref().map(|i| i.text.as_str()),
        Some("Listening on a cast device")
    );
}
