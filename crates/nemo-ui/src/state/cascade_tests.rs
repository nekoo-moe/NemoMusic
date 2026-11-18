//! The scroll cascade in the reducer (`lyrics-rebuild`): a line change glides, a seek jumps.

use super::*;
use crate::mock;
use nemo_core::lyrics::ProviderId;
use nemo_core::services::LyricsLookup;

fn ready(reduced: bool) -> UiState {
    let t = mock::track(0);
    let snapshot = PlayerSnapshot {
        state: PlayState::Playing,
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
            doc: mock::lyrics_for(0, Duration::from_secs(200)),
            tried: vec![ProviderId::new("A")],
        },
        offset_ms: 0,
    });
    s
}

/// Play on in 100 ms steps from `from` until the active line changes; returns the new position.
fn play_to_next_line(s: &mut UiState, from: Duration) -> Duration {
    let line = s.active_lyric();
    let mut pos = from;
    for _ in 0..400 {
        pos += Duration::from_millis(100);
        s.apply(Event::Position(pos));
        s.apply(Event::Frame { dt_ms: 16.0 });
        if s.active_lyric() != line {
            return pos;
        }
    }
    pos
}

#[test]
fn a_line_change_lays_out_at_once_and_glides_in_a_cascade() {
    let mut s = ready(false);
    s.apply(Event::Position(Duration::from_secs(60)));
    s.apply(Event::Frame { dt_ms: 16.0 });
    assert!(s.lyric_scroll.is_settled(), "the first jump is a seek");
    let before = s.lyric_scroll.value;
    let pos = play_to_next_line(&mut s, Duration::from_secs(60));
    assert!(pos > Duration::from_secs(60));
    assert!(s.lyric_scroll.value > before, "laid out at the new place");
    assert!(!s.lyric_scroll.is_settled(), "the lines are still gliding");
    assert!(s.needs_frames());
    let active = s.active_lyric().expect("active line");
    let below = s.lyric_scroll.offset(active + 2);
    let at = s.lyric_scroll.offset(active);
    assert!(at > 0.0 && below >= at, "{below} vs {at}: lines below lag");
    for _ in 0..200 {
        s.apply(Event::Frame { dt_ms: 16.0 });
    }
    assert!(s.lyric_scroll.is_settled());
    assert!(s.lyric_scroll.offset(active + 2).abs() < 1e-3);
}

#[test]
fn a_seek_jumps_without_the_cascade() {
    let mut s = ready(false);
    s.apply(Event::Position(Duration::from_secs(30)));
    s.apply(Event::Frame { dt_ms: 16.0 });
    let pos = play_to_next_line(&mut s, Duration::from_secs(30));
    for _ in 0..200 {
        s.apply(Event::Frame { dt_ms: 16.0 });
    }
    assert!(s.lyric_scroll.is_settled());
    // A jump forward by half a minute and one back by ten seconds.
    for to in [pos + Duration::from_secs(30), pos + Duration::from_secs(20)] {
        let before = s.lyric_scroll.value;
        s.apply(Event::Position(to));
        s.apply(Event::Frame { dt_ms: 16.0 });
        assert!(s.lyric_scroll.is_settled(), "a seek has no glide");
        assert!((s.lyric_scroll.value - before).abs() > 1.0, "but it moved");
    }
}

#[test]
fn reduced_motion_jumps_on_every_line_change() {
    let mut s = ready(true);
    s.apply(Event::Position(Duration::from_secs(60)));
    s.apply(Event::Frame { dt_ms: 16.0 });
    let before = s.lyric_scroll.value;
    play_to_next_line(&mut s, Duration::from_secs(60));
    assert!(s.lyric_scroll.value > before);
    assert!(s.lyric_scroll.is_settled(), "no glide");
}

#[test]
fn manual_scrolling_cancels_the_glides_and_following_resumes_with_one() {
    let mut s = ready(false);
    s.apply(Event::Position(Duration::from_secs(60)));
    s.apply(Event::Frame { dt_ms: 16.0 });
    play_to_next_line(&mut s, Duration::from_secs(60));
    assert!(!s.lyric_scroll.is_settled());
    s.apply(Event::LyricsScrolled(80.0));
    assert!(s.lyric_scroll.is_settled(), "the hand takes over");
    s.apply(Event::LyricsFollow);
    assert!(
        !s.lyric_scroll.is_settled(),
        "back to the line with a glide"
    );
}
