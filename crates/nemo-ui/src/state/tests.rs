//! Unit tests of the UI state reducer.

use super::*;
use crate::keys::KeyAction;
use crate::mock;
use nemo_core::lyrics::{Granularity, LyricLine};
use nemo_core::lyrics::{LyricsDoc, ProviderId};
use nemo_core::services::LyricsLookup;

fn playing(index: usize) -> PlayerSnapshot {
    let t = mock::track(index);
    PlayerSnapshot {
        state: PlayState::Playing,
        duration: t.duration,
        queue: vec![t.clone()],
        queue_index: Some(0),
        current: Some(t),
        volume_percent: 50,
        ..PlayerSnapshot::default()
    }
}

fn state() -> UiState {
    let mut s = UiState::new(Route::Home, PlayerSnapshot::default(), false);
    s.apply(Event::Snapshot(playing(0)));
    s
}

fn doc() -> LyricsDoc {
    mock::lyrics_for(0, Duration::from_secs(200)).expect("lyrics")
}

fn lookup(doc: Option<LyricsDoc>) -> LyricsLookup {
    LyricsLookup {
        doc,
        tried: vec![ProviderId::new("A")],
    }
}

#[test]
fn a_new_track_requests_cover_and_lyrics_once() {
    let mut s = UiState::new(Route::Home, PlayerSnapshot::default(), false);
    let fx = s.apply(Event::Snapshot(playing(0)));
    assert_eq!(fx.len(), 2);
    assert!(matches!(fx[0], Effect::LoadCover(_)) && matches!(fx[1], Effect::LoadLyrics(_)));
    assert_eq!(s.lyrics, LyricsState::Loading);
    assert!(
        s.apply(Event::Snapshot(playing(0))).is_empty(),
        "same track"
    );
    assert_eq!(s.apply(Event::Snapshot(playing(1))).len(), 2);
    assert_eq!(s.apply(Event::Snapshot(PlayerSnapshot::default())).len(), 0);
    assert_eq!(s.lyrics, LyricsState::Idle);
}

#[test]
fn stale_cover_and_lyrics_are_ignored() {
    let mut s = state();
    let other = mock::track(9).id;
    s.apply(Event::CoverReady {
        track: other.clone(),
        palette: Palette::fallback(),
    });
    assert!(s.accent.is_done(), "no fade started for another track");
    s.apply(Event::LyricsReady {
        track: other,
        lookup: lookup(Some(doc())),
        offset_ms: 0,
    });
    assert_eq!(s.lyrics, LyricsState::Loading);
}

#[test]
fn cover_ready_starts_a_crossfade() {
    let mut s = state();
    let mut p = Palette::fallback();
    p.primary.r = p.primary.r.wrapping_add(40);
    s.apply(Event::CoverReady {
        track: mock::track(0).id,
        palette: p,
    });
    assert!(!s.accent.is_done());
    assert!(s.needs_frames());
    for _ in 0..30 {
        s.apply(Event::Frame { dt_ms: 16.0 });
    }
    assert!(s.accent.is_done());
    assert_eq!(s.accent.current(), p);
}

#[test]
fn reduced_motion_swaps_the_accent_at_once() {
    let mut s = UiState::new(Route::Home, playing(0), true);
    let mut p = Palette::fallback();
    p.primary.b = p.primary.b.wrapping_add(30);
    s.apply(Event::CoverReady {
        track: mock::track(0).id,
        palette: p,
    });
    assert!(s.accent.is_done());
}

#[test]
fn lyrics_missing_and_ready() {
    let mut s = state();
    s.apply(Event::LyricsReady {
        track: mock::track(0).id,
        lookup: lookup(None),
        offset_ms: 0,
    });
    assert_eq!(s.lyrics, LyricsState::Missing(vec![ProviderId::new("A")]));
    s.apply(Event::LyricsReady {
        track: mock::track(0).id,
        lookup: lookup(Some(doc())),
        offset_ms: 250,
    });
    assert!(matches!(s.lyrics, LyricsState::Ready(_)));
    assert_eq!(s.lyrics_offset_ms, 250);
}

#[test]
fn idle_needs_no_frames_and_playing_does() {
    let mut s = state();
    assert!(s.needs_frames(), "playing");
    let mut paused = playing(0);
    paused.state = PlayState::Paused;
    s.apply(Event::Snapshot(paused));
    assert!(!s.needs_frames(), "paused and settled: zero redraws");
}

#[test]
fn toggle_play_follows_the_state() {
    let mut s = state();
    assert_eq!(
        s.apply(Event::Key(KeyAction::TogglePlay)),
        vec![Effect::Send(PlayerCommand::Pause)]
    );
    let mut snap = playing(0);
    snap.state = PlayState::Paused;
    s.apply(Event::Snapshot(snap.clone()));
    assert_eq!(
        s.apply(Event::Key(KeyAction::TogglePlay)),
        vec![Effect::Send(PlayerCommand::Resume)]
    );
    snap.state = PlayState::Stopped;
    s.apply(Event::Snapshot(snap));
    assert_eq!(
        s.apply(Event::Key(KeyAction::TogglePlay)),
        vec![Effect::Send(PlayerCommand::Resume)]
    );
}

#[test]
fn seek_and_volume_keys_clamp() {
    let mut s = state();
    s.apply(Event::Position(Duration::from_secs(10)));
    assert_eq!(
        s.apply(Event::Key(KeyAction::SeekBy(5))),
        vec![Effect::Send(PlayerCommand::Seek(Duration::from_secs(15)))]
    );
    assert_eq!(
        s.apply(Event::Key(KeyAction::SeekBy(-30))),
        vec![Effect::Send(PlayerCommand::Seek(Duration::ZERO))]
    );
    s.apply(Event::Position(Duration::from_secs(1000)));
    let d = s.snapshot.duration.expect("duration");
    assert_eq!(
        s.apply(Event::Key(KeyAction::SeekBy(30))),
        vec![Effect::Send(PlayerCommand::Seek(d))]
    );
    assert_eq!(
        s.apply(Event::Key(KeyAction::VolumeBy(5))),
        vec![Effect::Send(PlayerCommand::SetVolume(55))]
    );
    s.snapshot.volume_percent = 3;
    assert_eq!(
        s.apply(Event::Key(KeyAction::VolumeBy(-5))),
        vec![Effect::Send(PlayerCommand::SetVolume(0))]
    );
    s.snapshot.volume_percent = 98;
    assert_eq!(
        s.apply(Event::Key(KeyAction::VolumeBy(5))),
        vec![Effect::Send(PlayerCommand::SetVolume(100))]
    );
}

#[test]
fn seek_fraction_uses_the_duration() {
    let mut s = state();
    let d = s.snapshot.duration.expect("duration");
    assert_eq!(
        s.apply(Event::SeekFraction(0.5)),
        vec![Effect::Send(PlayerCommand::Seek(d.mul_f32(0.5)))]
    );
    assert_eq!(
        s.apply(Event::SeekFraction(7.0)),
        vec![Effect::Send(PlayerCommand::Seek(d))]
    );
    let mut empty = UiState::new(Route::Home, PlayerSnapshot::default(), false);
    assert_eq!(empty.apply(Event::SeekFraction(0.5)), vec![]);
}

#[test]
fn navigation_keys_and_close() {
    let mut s = state();
    s.apply(Event::Key(KeyAction::OpenSearch));
    assert!(matches!(s.route(), Route::Search(_)));
    s.apply(Event::Key(KeyAction::OpenSettings));
    assert_eq!(s.route(), &Route::Settings);
    s.apply(Event::ToggleQueue);
    s.apply(Event::Key(KeyAction::Close));
    assert!(!s.queue_open, "Esc closes the panel first");
    assert_eq!(s.route(), &Route::Settings);
    s.apply(Event::Key(KeyAction::Close));
    assert!(matches!(s.route(), Route::Search(_)));
    s.apply(Event::Forward);
    assert_eq!(s.route(), &Route::Settings);
}

#[test]
fn lyrics_offset_keys_clamp_and_persist() {
    let mut s = state();
    let fx = s.apply(Event::Key(KeyAction::LyricsOffsetBy(50)));
    assert_eq!(
        fx,
        vec![Effect::SaveLyricsOffset {
            track_key: "mock-track-0".into(),
            offset_ms: 50
        }]
    );
    s.lyrics_offset_ms = 9_990;
    s.apply(Event::Key(KeyAction::LyricsOffsetBy(50)));
    assert_eq!(s.lyrics_offset_ms, 10_000);
    let fx = s.apply(Event::LyricsOffsetReset);
    assert_eq!(s.lyrics_offset_ms, 0);
    assert_eq!(fx.len(), 1);
    let mut none = UiState::new(Route::Home, PlayerSnapshot::default(), false);
    assert_eq!(
        none.apply(Event::Key(KeyAction::LyricsOffsetBy(50))),
        vec![]
    );
}

#[test]
fn frame_readout_key_is_an_effect() {
    let mut s = state();
    assert_eq!(
        s.apply(Event::Key(KeyAction::ToggleFrameReadout)),
        vec![Effect::ToggleFrameReadout]
    );
}

#[test]
fn the_lyric_scroll_follows_the_active_line_and_pauses_on_manual_scroll() {
    let mut s = state();
    s.apply(Event::LyricsReady {
        track: mock::track(0).id,
        lookup: lookup(Some(doc())),
        offset_ms: 0,
    });
    s.apply(Event::Position(Duration::from_secs(60)));
    for _ in 0..120 {
        s.apply(Event::Frame { dt_ms: 16.0 });
    }
    let active = s.active_lyric().expect("active");
    assert!(active > 5);
    assert!(s.lyric_scroll.is_settled());
    let target = lyrics_view::scroll_target(Some(active), s.lyric_viewport_h);
    assert!((s.lyric_scroll.value - target).abs() < 0.5);

    s.apply(Event::LyricsScrolled(120.0));
    assert!(!s.lyrics_following());
    let held = s.lyric_scroll.value;
    s.apply(Event::Position(Duration::from_secs(100)));
    s.apply(Event::Frame { dt_ms: 16.0 });
    assert!(
        (s.lyric_scroll.value - held).abs() < 0.5,
        "no auto-follow during the pause"
    );
    for _ in 0..300 {
        s.apply(Event::Frame { dt_ms: 16.0 });
    }
    assert!(s.lyrics_following(), "the 4 s pause ended");
    for _ in 0..120 {
        s.apply(Event::Frame { dt_ms: 16.0 });
    }
    let target = lyrics_view::scroll_target(s.active_lyric(), s.lyric_viewport_h);
    assert!(
        (s.lyric_scroll.value - target).abs() < 0.5,
        "glided back to the current line"
    );
    assert!((s.lyric_scroll.value - held).abs() > 1.0);

    s.apply(Event::LyricsScrolled(-200.0));
    assert!(!s.lyrics_following());
    s.apply(Event::LyricsFollow);
    assert!(s.lyrics_following(), "the pill returns at once");
}

#[test]
fn unsynced_lyrics_never_scroll() {
    let mut s = state();
    let d = LyricsDoc {
        lines: vec![
            LyricLine {
                start_ms: None,
                end_ms: None,
                text: "x".into(),
                words: Vec::new(),
            };
            40
        ],
        granularity: Granularity::Unsynced,
        provider: ProviderId::new("A"),
        matched: None,
        instrumental: false,
    };
    s.apply(Event::LyricsReady {
        track: mock::track(0).id,
        lookup: lookup(Some(d)),
        offset_ms: 0,
    });
    s.apply(Event::Position(Duration::from_secs(90)));
    s.apply(Event::Frame { dt_ms: 16.0 });
    assert_eq!(s.active_lyric(), None);
    assert!(s.lyric_scroll.value.abs() < f32::EPSILON);
}
