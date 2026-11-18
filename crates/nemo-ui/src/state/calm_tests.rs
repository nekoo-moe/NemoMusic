//! The lyrics ask for frames only while something moves (`lyrics-rebuild`): paused, with no line
//! lit and settled, or reduced, they ask for none.

use super::*;
use crate::mock;
use nemo_core::lyrics::{LyricLine, LyricWord, LyricsDoc, ProviderId};
use nemo_core::services::LyricsLookup;

/// One timed line at 10 s to 12 s with two words, so there is an intro note before it (and
/// nothing after it).
fn doc() -> LyricsDoc {
    LyricsDoc {
        lines: vec![LyricLine {
            start_ms: Some(10_000),
            end_ms: Some(12_000),
            text: "la la".to_owned(),
            words: vec![
                LyricWord {
                    start_ms: 10_000,
                    end_ms: Some(11_000),
                    text: "la ".to_owned(),
                },
                LyricWord {
                    start_ms: 11_000,
                    end_ms: Some(12_000),
                    text: "la".to_owned(),
                },
            ],
        }],
        granularity: Granularity::Word,
        provider: ProviderId::new("A"),
        matched: None,
        instrumental: false,
    }
}

fn app_state(playing: bool, reduced: bool) -> UiState {
    let t = mock::track(0);
    let snapshot = PlayerSnapshot {
        state: if playing {
            PlayState::Playing
        } else {
            PlayState::Paused
        },
        duration: t.duration,
        queue: vec![t.clone()],
        queue_index: Some(0),
        current: Some(t.clone()),
        volume_percent: 50,
        ..PlayerSnapshot::default()
    };
    let mut s = UiState::new(Route::Home, PlayerSnapshot::default(), reduced);
    s.apply(Event::Snapshot(snapshot));
    s.apply(Event::LyricsReady {
        track: t.id,
        lookup: LyricsLookup {
            doc: Some(doc()),
            tried: Vec::new(),
        },
        offset_ms: 0,
    });
    s
}

/// Settle every glide and the slide-in at `at`.
fn settle(s: &mut UiState, at_ms: u64) {
    s.apply(Event::Position(Duration::from_millis(at_ms)));
    for _ in 0..80 {
        s.apply(Event::Frame { dt_ms: 16.0 });
    }
}

#[test]
fn the_lyrics_ask_for_frames_while_a_line_is_lit_and_only_while_playing() {
    let mut s = app_state(true, false);
    settle(&mut s, 10_500);
    assert!(s.lyric_timeline.busy(&s.lyric_clocks().expect("clocks")));
    assert!(s.lyrics_frames(), "a lit line moves");
    // Long after the only line: nothing is lit, nothing waits.
    settle(&mut s, 40_000);
    assert!(!s.lyrics_frames(), "playing, but no line is lit");
    // Back in the line, then paused: the position stands still, so nothing moves.
    settle(&mut s, 10_500);
    s.apply(Event::Snapshot(PlayerSnapshot {
        state: PlayState::Paused,
        ..s.snapshot.clone()
    }));
    settle(&mut s, 10_500);
    assert!(!s.lyrics_frames(), "paused: zero lyric frames");
}

#[test]
fn the_intro_note_fills_with_the_position_so_it_asks_for_frames() {
    let mut s = app_state(true, false);
    settle(&mut s, 3_000);
    assert!(s.lyric_focus().note.is_some());
    assert!(s.lyrics_frames());
}

#[test]
fn the_list_sliding_in_asks_for_frames_until_it_is_in_place() {
    let mut s = app_state(false, false);
    assert!(s.lyric_enter_ms.abs() < f32::EPSILON, "just arrived");
    assert!(s.lyrics_frames());
    for _ in 0..30 {
        s.apply(Event::Frame { dt_ms: 16.0 });
    }
    assert!(s.lyrics_frames(), "480 ms: still sliding");
    for _ in 0..10 {
        s.apply(Event::Frame { dt_ms: 16.0 });
    }
    assert!(!s.lyrics_frames(), "640 ms: in place, paused, calm");
}

#[test]
fn reduced_motion_has_no_slide_and_no_shimmer() {
    let s = app_state(false, true);
    assert!(s.lyric_enter_ms >= enter::ENTER_MS);
    assert!(!s.lyrics_frames());
    let mut loading = UiState::new(Route::Home, PlayerSnapshot::default(), true);
    loading.lyrics = LyricsState::Loading;
    assert!(!loading.lyrics_frames(), "a still loading line");
    let mut moving = app_state(true, false);
    moving.lyrics = LyricsState::Loading;
    assert!(
        moving.lyrics_frames(),
        "the shimmer moves while a song plays"
    );
    let mut paused = app_state(false, false);
    paused.lyrics = LyricsState::Loading;
    assert!(!paused.lyrics_frames(), "and stands still while paused");
    for _ in 0..400 {
        moving.apply(Event::Frame { dt_ms: 16.0 });
    }
    assert!(moving.lyric_loading_ms < enter::SHIMMER_MS, "it loops");
}
