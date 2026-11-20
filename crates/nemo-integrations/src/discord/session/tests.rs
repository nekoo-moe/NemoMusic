use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use nemo_core::audio::Origin;
use nemo_core::hooks::PlayHookEvent;
use nemo_core::player::PlayState;
use nemo_core::track::{Artwork, Thumbnail, Track, TrackId};

use super::{Browsing, Place, PresenceState, Session, Source, lossless_codec};

fn track(origin: Origin) -> Track {
    Track {
        id: TrackId("t1".into()),
        title: "Blue in Green".into(),
        artists: vec!["Miles Davis".into(), "Bill Evans".into()],
        album: Some("Kind of Blue".into()),
        duration: Some(Duration::from_secs(337)),
        origin,
        video_id: None,
        local_path: None,
        artwork: Artwork::None,
        source_url: None,
    }
}

fn started(track: Track) -> PlayHookEvent {
    PlayHookEvent::TrackStarted {
        track,
        started_at: SystemTime::UNIX_EPOCH,
    }
}

fn state(state: PlayState, secs: u64) -> PlayHookEvent {
    PlayHookEvent::StateChanged {
        state,
        position: Duration::from_secs(secs),
    }
}

#[test]
fn a_new_session_is_idle_since_its_start() {
    let s = Session::new(1_000);
    assert_eq!(s.state(false), PresenceState::Idle { since: 1_000 });
    assert!(!s.playing());
}

#[test]
fn a_song_that_plays_shows_progress_from_its_position() {
    let mut s = Session::new(1_000);
    s.on_play(&started(track(Origin::YoutubeMusic)), 2_000);
    s.on_play(&state(PlayState::Playing, 0), 2_000);
    let PresenceState::Listening {
        song, start, end, ..
    } = s.state(false)
    else {
        panic!("not listening");
    };
    assert_eq!(song.title, "Blue in Green");
    assert_eq!(song.artist, "Miles Davis, Bill Evans");
    assert_eq!(song.source, Source::YouTubeMusic);
    assert_eq!((start, end), (2_000, Some(2_337)));
    assert!(s.playing());
}

#[test]
fn a_seek_moves_the_start_and_keeps_the_length() {
    let mut s = Session::new(0);
    s.on_play(&started(track(Origin::YoutubeMusic)), 1_000);
    s.on_play(&state(PlayState::Playing, 0), 1_000);
    s.on_play(
        &PlayHookEvent::PositionJumped {
            position: Duration::from_secs(200),
        },
        1_050,
    );
    let PresenceState::Listening { start, end, .. } = s.state(false) else {
        panic!("not listening");
    };
    assert_eq!((start, end), (850, Some(1_187)));
}

#[test]
fn pause_and_resume_follow_the_position() {
    let mut s = Session::new(0);
    s.on_play(&started(track(Origin::SoundCloud)), 100);
    s.on_play(&state(PlayState::Playing, 0), 100);
    s.on_play(&state(PlayState::Paused, 30), 130);
    assert!(matches!(s.state(false), PresenceState::Paused { .. }));
    assert!(!s.playing());
    // Ten minutes later the song resumes at 30 s.
    s.on_play(&state(PlayState::Playing, 30), 730);
    let PresenceState::Listening { start, .. } = s.state(false) else {
        panic!("not listening");
    };
    assert_eq!(start, 700);
}

#[test]
fn buffering_keeps_what_is_shown() {
    let mut s = Session::new(0);
    s.on_play(&started(track(Origin::YoutubeMusic)), 10);
    s.on_play(&state(PlayState::Playing, 0), 10);
    let before = s.state(false);
    s.on_play(&state(PlayState::Buffering, 0), 10);
    assert_eq!(s.state(false), before);
}

#[test]
fn stopping_goes_idle_from_the_time_it_stopped() {
    let mut s = Session::new(0);
    s.on_play(&started(track(Origin::YoutubeMusic)), 10);
    s.on_play(&state(PlayState::Playing, 0), 10);
    s.on_play(&state(PlayState::Stopped, 0), 400);
    assert_eq!(s.state(false), PresenceState::Idle { since: 400 });
}

#[test]
fn the_browsed_section_shows_only_while_nothing_plays() {
    let mut s = Session::new(5);
    s.set_section(Some(Browsing::Search));
    assert_eq!(
        s.state(false),
        PresenceState::Browsing {
            section: Browsing::Search,
            since: 5
        }
    );
    s.on_play(&started(track(Origin::YoutubeMusic)), 10);
    s.on_play(&state(PlayState::Playing, 0), 10);
    assert!(matches!(s.state(false), PresenceState::Listening { .. }));
}

#[test]
fn asleep_wins_over_everything() {
    let mut s = Session::new(0);
    s.on_play(&started(track(Origin::YoutubeMusic)), 10);
    s.on_play(&state(PlayState::Paused, 4), 10);
    assert_eq!(s.state(true), PresenceState::Sleeping);
}

#[test]
fn place_and_sleep_timer_are_part_of_the_state() {
    let mut s = Session::new(0);
    s.on_play(&started(track(Origin::YoutubeMusic)), 10);
    s.on_play(&state(PlayState::Playing, 0), 10);
    s.set_place(Place::Cast);
    s.set_sleep_timer(true);
    let PresenceState::Listening {
        place, sleep_timer, ..
    } = s.state(false)
    else {
        panic!("not listening");
    };
    assert_eq!((place, sleep_timer), (Place::Cast, true));
}

#[test]
fn the_length_may_become_known_later() {
    let mut t = track(Origin::YoutubeMusic);
    t.duration = None;
    let mut s = Session::new(0);
    s.on_play(&started(t), 10);
    s.on_play(&state(PlayState::Playing, 0), 10);
    let PresenceState::Listening { end, .. } = s.state(false) else {
        panic!("not listening");
    };
    assert_eq!(end, None);
    s.on_play(&PlayHookEvent::DurationKnown(Duration::from_secs(60)), 11);
    let PresenceState::Listening { end, .. } = s.state(false) else {
        panic!("not listening");
    };
    assert_eq!(end, Some(70));
}

#[test]
fn a_remote_cover_is_picked_and_a_local_one_is_never_sent() {
    let mut t = track(Origin::YoutubeMusic);
    t.artwork = Artwork::Remote(vec![
        Thumbnail {
            url: "https://i.ytimg.com/small".into(),
            width: 120,
            height: 120,
        },
        Thumbnail {
            url: "https://i.ytimg.com/big".into(),
            width: 544,
            height: 544,
        },
    ]);
    let mut s = Session::new(0);
    s.on_play(&started(t), 10);
    s.on_play(&state(PlayState::Playing, 0), 10);
    let PresenceState::Listening { song, .. } = s.state(false) else {
        panic!("not listening");
    };
    assert_eq!(song.cover.as_deref(), Some("https://i.ytimg.com/big"));

    let mut local = track(Origin::Local);
    local.artwork = Artwork::LocalFile(PathBuf::from("/home/me/Music/cover.jpg"));
    local.local_path = Some(PathBuf::from("/home/me/Music/secret song.flac"));
    let mut s = Session::new(0);
    s.on_play(&started(local), 10);
    s.on_play(&state(PlayState::Playing, 0), 10);
    let PresenceState::Listening { song, .. } = s.state(false) else {
        panic!("not listening");
    };
    assert_eq!(song.cover, None);
    assert_eq!(song.codec.as_deref(), Some("FLAC"));
    assert!(!format!("{song:?}").contains("/home/me"));
}

#[test]
fn only_lossless_extensions_name_a_codec() {
    for (path, want) in [
        ("a.flac", Some("FLAC")),
        ("a.WAV", Some("WAV")),
        ("a.aif", Some("AIFF")),
        ("a.aiff", Some("AIFF")),
        ("a.ape", Some("APE")),
        ("a.wv", Some("WavPack")),
        ("a.mp3", None),
        ("a.m4a", None),
        ("a", None),
    ] {
        assert_eq!(
            lossless_codec(std::path::Path::new(path)).as_deref(),
            want,
            "{path}"
        );
    }
}
