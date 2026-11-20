use super::{ASSET_APP, Browsing, Place, PresenceState, Song, Source, build, cover_address};
use crate::discord::activity::{Activity, ActivityKind};

fn song(source: Source) -> Song {
    Song {
        title: "Blue in Green".into(),
        artist: "Miles Davis".into(),
        source,
        cover: None,
        codec: None,
    }
}

fn listening(song: Song, place: Place, sleep_timer: bool) -> PresenceState {
    PresenceState::Listening {
        song,
        start: 1_000,
        end: Some(1_337),
        place,
        sleep_timer,
    }
}

fn large(a: &Activity) -> (&str, &str) {
    let img = a.large.as_ref().expect("large image");
    (&img.key, &img.text)
}

fn small(a: &Activity) -> Option<(&str, &str)> {
    a.small.as_ref().map(|i| (i.key.as_str(), i.text.as_str()))
}

#[test]
fn listening_shows_title_artist_and_a_progress_bar() {
    let a = build(
        &listening(song(Source::YouTubeMusic), Place::ThisDevice, false),
        false,
    );
    assert_eq!(a.kind, ActivityKind::Listening);
    assert_eq!(a.details, "Blue in Green");
    assert_eq!(a.state.as_deref(), Some("Miles Davis"));
    assert_eq!((a.start, a.end), (Some(1_000), Some(1_337)));
    assert_eq!(large(&a), (ASSET_APP, "NemoMusic"));
    assert_eq!(small(&a), Some(("youtube-music", "YouTube Music")));
}

#[test]
fn a_live_stream_has_a_start_and_no_end() {
    let state = PresenceState::Listening {
        song: song(Source::SoundCloud),
        start: 500,
        end: None,
        place: Place::ThisDevice,
        sleep_timer: false,
    };
    let a = build(&state, false);
    assert_eq!((a.start, a.end), (Some(500), None));
    assert_eq!(small(&a), Some(("soundcloud", "SoundCloud")));
}

#[test]
fn an_end_not_after_the_start_is_dropped() {
    let state = PresenceState::Listening {
        song: song(Source::Local),
        start: 500,
        end: Some(500),
        place: Place::ThisDevice,
        sleep_timer: false,
    };
    assert_eq!(build(&state, false).end, None);
}

#[test]
fn paused_has_no_times_and_the_paused_picture() {
    let state = PresenceState::Paused {
        song: song(Source::YouTubeMusic),
        place: Place::ThisDevice,
        sleep_timer: false,
    };
    let a = build(&state, false);
    assert_eq!(a.details, "Blue in Green");
    assert_eq!(a.state.as_deref(), Some("Paused \u{b7} Miles Davis"));
    assert_eq!((a.start, a.end), (None, None));
    assert_eq!(small(&a), Some(("paused", "Paused")));
}

#[test]
fn quiet_states_say_nothing_is_playing() {
    let idle = build(&PresenceState::Idle { since: 77 }, false);
    assert_eq!(idle.kind, ActivityKind::Playing);
    assert_eq!(idle.details, "Idle");
    assert_eq!(idle.state.as_deref(), Some("Nothing playing"));
    assert_eq!((idle.start, idle.end), (Some(77), None));
    assert_eq!(large(&idle).0, ASSET_APP);

    let library = build(
        &PresenceState::Browsing {
            section: Browsing::Library,
            since: 5,
        },
        false,
    );
    assert_eq!(library.details, "Browsing Library");
    assert_eq!(library.start, Some(5));
    let search = build(
        &PresenceState::Browsing {
            section: Browsing::Search,
            since: 5,
        },
        false,
    );
    assert_eq!(search.details, "Searching");
    assert_eq!(search.state.as_deref(), Some("Nothing playing"));
}

#[test]
fn sleeping_shows_away_without_times() {
    let a = build(&PresenceState::Sleeping, false);
    assert_eq!(a.details, "Away");
    assert_eq!(a.state.as_deref(), Some("Sleeping"));
    assert_eq!((a.start, a.end), (None, None));
}

#[test]
fn a_video_watches_and_a_paused_video_drops_its_times() {
    let playing = PresenceState::Video {
        title: "Concert".into(),
        start: 10,
        end: Some(100),
        paused: false,
    };
    let a = build(&playing, false);
    assert_eq!(a.kind, ActivityKind::Watching);
    assert_eq!(a.details, "Concert");
    assert_eq!(a.state.as_deref(), Some("Watching a video"));
    assert_eq!((a.start, a.end), (Some(10), Some(100)));

    let paused = PresenceState::Video {
        title: "Concert".into(),
        start: 10,
        end: Some(100),
        paused: true,
    };
    let b = build(&paused, false);
    assert_eq!(b.state.as_deref(), Some("Watching a video \u{b7} Paused"));
    assert_eq!((b.start, b.end), (None, None));
}

#[test]
fn cast_together_and_sleep_timer_are_facts_in_the_large_tooltip() {
    let cast = build(
        &listening(song(Source::YouTubeMusic), Place::Cast, false),
        false,
    );
    assert_eq!(large(&cast).1, "Listening on a cast device");
    let together = build(
        &listening(song(Source::YouTubeMusic), Place::Together, false),
        false,
    );
    assert_eq!(large(&together).1, "Listening together");
    let timer = build(
        &listening(song(Source::YouTubeMusic), Place::ThisDevice, true),
        false,
    );
    assert_eq!(large(&timer).1, "Sleep timer on");
    let both = build(
        &listening(song(Source::YouTubeMusic), Place::Together, true),
        false,
    );
    assert_eq!(large(&both).1, "Listening together \u{b7} Sleep timer on");
}

#[test]
fn a_local_lossless_file_names_its_codec_and_other_sources_never_do() {
    let mut local = song(Source::Local);
    local.codec = Some("FLAC".into());
    let a = build(&listening(local, Place::ThisDevice, false), false);
    assert_eq!(small(&a), Some(("local", "Local file, FLAC")));

    let mut youtube = song(Source::YouTubeMusic);
    youtube.codec = Some("FLAC".into());
    let b = build(&listening(youtube, Place::ThisDevice, false), false);
    assert_eq!(small(&b), Some(("youtube-music", "YouTube Music")));
    let all = format!(
        "{:?}",
        build(
            &listening(song(Source::SoundCloud), Place::ThisDevice, false),
            false
        )
    );
    assert!(!all.to_lowercase().contains("lossless"));
}

#[test]
fn only_a_public_https_cover_on_a_cover_host_is_used() {
    let good = [
        "https://i.ytimg.com/vi/abc/hq.jpg",
        "https://lh3.googleusercontent.com/x=w544-h544",
        "https://yt3.ggpht.com/a",
        "https://i1.sndcdn.com/artworks-1-t500x500.jpg",
    ];
    for url in good {
        assert_eq!(cover_address(url), Some(url), "{url}");
    }
    let long = format!("https://i.ytimg.com/{}", "a".repeat(260));
    let bad = [
        "http://i.ytimg.com/a.jpg",
        "https://example.com/a.jpg",
        "https://i.ytimg.com.evil.example/a.jpg",
        "https://user@i.ytimg.com/a.jpg",
        "file:///home/me/cover.jpg",
        "/home/me/cover.jpg",
        "",
        long.as_str(),
    ];
    for url in bad {
        assert_eq!(cover_address(url), None, "{url}");
    }
}

#[test]
fn the_cover_becomes_the_large_image_and_a_bad_one_falls_back_to_the_app_icon() {
    let mut s = song(Source::YouTubeMusic);
    s.cover = Some("https://i.ytimg.com/vi/abc/hq.jpg".into());
    let a = build(&listening(s.clone(), Place::ThisDevice, false), false);
    assert_eq!(
        large(&a),
        ("https://i.ytimg.com/vi/abc/hq.jpg", "NemoMusic")
    );
    s.cover = Some("/home/me/Music/cover.jpg".into());
    let b = build(&listening(s, Place::ThisDevice, false), false);
    assert_eq!(large(&b).0, ASSET_APP);
}

#[test]
fn private_mode_strips_every_state_that_holds_a_title() {
    let mut s = song(Source::Local);
    s.cover = Some("https://i.ytimg.com/vi/abc/hq.jpg".into());
    s.codec = Some("FLAC".into());
    let states = [
        listening(s.clone(), Place::Together, true),
        PresenceState::Paused {
            song: s,
            place: Place::Cast,
            sleep_timer: true,
        },
        PresenceState::Video {
            title: "Concert".into(),
            start: 1,
            end: Some(9),
            paused: false,
        },
    ];
    for state in states {
        let a = build(&state, true);
        assert_eq!(a.kind, ActivityKind::Listening);
        assert_eq!(a.details, "Listening to NemoMusic");
        assert_eq!(a.state, None);
        assert_eq!((a.start, a.end), (None, None));
        assert_eq!(large(&a), (ASSET_APP, "NemoMusic"));
        assert_eq!(a.small, None);
        let all = format!("{a:?}");
        for secret in [
            "Blue in Green",
            "Miles",
            "ytimg",
            "FLAC",
            "Concert",
            "cast",
            "together",
            "Sleep",
        ] {
            assert!(!all.contains(secret), "{secret}");
        }
    }
}

#[test]
fn private_mode_leaves_states_without_a_title_alone() {
    for state in [
        PresenceState::Idle { since: 1 },
        PresenceState::Browsing {
            section: Browsing::Search,
            since: 1,
        },
        PresenceState::Sleeping,
    ] {
        assert_eq!(build(&state, true), build(&state, false));
    }
}

#[test]
fn every_text_fits_discord_and_empty_ones_get_a_stand_in() {
    let mut s = song(Source::YouTubeMusic);
    s.title = "\u{e9}".repeat(300);
    s.artist = String::new();
    let a = build(&listening(s, Place::ThisDevice, false), false);
    assert!(a.details.len() <= 128 && a.details.len() >= 2);
    assert_eq!(a.state.as_deref(), Some("Unknown artist"));
    let mut t = song(Source::Local);
    t.title = "  ".into();
    assert_eq!(
        build(&listening(t, Place::ThisDevice, false), false).details,
        "Unknown title"
    );
}
