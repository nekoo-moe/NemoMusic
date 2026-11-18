use super::*;
use nemo_core::lyrics::{LyricWord, ProviderId};

fn line(start: u64, end: Option<u64>, text: &str) -> LyricLine {
    LyricLine {
        start_ms: Some(start),
        end_ms: end,
        text: text.to_owned(),
        words: Vec::new(),
    }
}

fn timed(start: u64, words: &[(u64, u64)]) -> LyricLine {
    LyricLine {
        start_ms: Some(start),
        end_ms: None,
        text: "la la".to_owned(),
        words: words
            .iter()
            .map(|(s, e)| LyricWord {
                start_ms: *s,
                end_ms: Some(*e),
                text: "la ".to_owned(),
            })
            .collect(),
    }
}

fn doc(lines: Vec<LyricLine>, granularity: Granularity) -> LyricsDoc {
    LyricsDoc {
        lines,
        granularity,
        provider: ProviderId::new("Mock"),
        matched: None,
        instrumental: false,
    }
}

#[test]
fn the_sync_offsets_are_115_ms_for_lines_and_150_ms_for_words() {
    assert_eq!(sync_offset_ms(Granularity::Line), 115);
    assert_eq!(sync_offset_ms(Granularity::Word), 150);
    assert_eq!(sync_offset_ms(Granularity::Syllable), 150);
    assert_eq!(sync_offset_ms(Granularity::Unsynced), 0);
}

#[test]
fn the_scroll_clock_runs_half_a_second_ahead_of_the_audio_clock() {
    let c = Clocks::new(Granularity::Line, 10_000, 0);
    assert_eq!(c.audio_ms, 10_115);
    assert_eq!(c.scroll_ms, 10_615);
    let w = Clocks::new(Granularity::Word, 10_000, -250);
    assert_eq!((w.audio_ms, w.scroll_ms), (9_900, 10_400));
    let big = Clocks::new(Granularity::Word, i64::MAX, i64::MAX);
    assert_eq!(big.scroll_ms, i64::MAX, "saturates");
}

#[test]
fn the_active_line_changes_615_ms_early_for_line_synced_lyrics() {
    let d = doc(
        vec![
            line(1000, None, "a"),
            line(5000, None, "b"),
            line(9000, None, "c"),
        ],
        Granularity::Line,
    );
    let t = Timeline::new(&d);
    let at = |p: i64| t.active(&Clocks::new(Granularity::Line, p, 0));
    assert_eq!(at(0), None);
    assert_eq!(at(384), None);
    assert_eq!(at(385), Some(0));
    assert_eq!(at(4_384), Some(0));
    assert_eq!(at(4_385), Some(1));
    assert_eq!(at(99_000), Some(2));
}

#[test]
fn a_timed_line_stays_lit_until_its_last_word_has_finished() {
    let d = doc(
        vec![
            timed(1000, &[(1000, 1500), (1500, 2600)]),
            timed(2200, &[(2200, 3000)]),
        ],
        Granularity::Word,
    );
    let t = Timeline::new(&d);
    let lit = |p: i64| -> Vec<usize> { t.lit(&Clocks::new(Granularity::Word, p, 0)).collect() };
    assert!(lit(0).is_empty());
    // Line 0 opens 500 ms before its start (audio clock = position + 150).
    assert_eq!(lit(349), Vec::<usize>::new());
    assert_eq!(lit(350), vec![0]);
    // Line 1 becomes active at 1550 while line 0 still sings until 2600 on the audio clock.
    assert_eq!(lit(1_600), vec![1, 0], "two lines lit at once");
    assert_eq!(lit(2_449), vec![1, 0]);
    assert_eq!(lit(2_450), vec![1], "line 0 is done at 2600");
    assert_eq!(lit(2_850), Vec::<usize>::new(), "line 1 is done at 3000");
}

#[test]
fn a_line_synced_line_hands_over_when_the_next_one_becomes_active() {
    let d = doc(
        vec![line(1000, None, "a"), line(5000, None, "b")],
        Granularity::Line,
    );
    let t = Timeline::new(&d);
    let w = t.window(0).expect("window");
    assert_eq!((w.start_ms, w.end_ms), (1000, 4500));
    let c = |p: i64| Clocks::new(Granularity::Line, p, 0);
    assert!(w.lit(&c(4_384)));
    assert!(!w.lit(&c(4_385)));
    // Only one line is lit in the middle of a song.
    assert_eq!(t.lit(&c(4_385)).collect::<Vec<_>>(), vec![1]);
}

#[test]
fn a_line_with_its_own_end_dims_when_it_ends_and_the_last_line_has_a_fallback() {
    let d = doc(
        vec![line(1000, Some(2500), "a"), line(20_000, None, "b")],
        Granularity::Line,
    );
    let t = Timeline::new(&d);
    assert_eq!(t.window(0).map(|w| w.end_ms), Some(2500));
    assert_eq!(t.window(1).map(|w| w.end_ms), Some(24_000));
}

#[test]
fn animating_opens_two_seconds_before_the_line() {
    let w = LineWindow {
        start_ms: 10_000,
        end_ms: 12_000,
    };
    let at = |a: i64| Clocks {
        audio_ms: a,
        scroll_ms: a + SCROLL_LEAD_MS,
    };
    assert!(!w.animating(&at(7_999)));
    assert!(w.animating(&at(8_000)));
    assert!(!w.lit(&at(8_000)), "prepared but not yet lit");
    assert!(w.animating(&at(11_999)) && !w.animating(&at(12_000)));
}

#[test]
fn the_lit_progress_rises_and_falls_over_166_ms() {
    let w = LineWindow {
        start_ms: 10_000,
        end_ms: 12_000,
    };
    let at = |a: i64| {
        w.lit_progress(&Clocks {
            audio_ms: a,
            scroll_ms: a + SCROLL_LEAD_MS,
        })
    };
    assert!(at(9_499).abs() < 1e-6);
    assert!(at(9_500).abs() < 1e-6);
    let mid = at(9_583);
    assert!(mid > 0.3 && mid < 0.9, "{mid}");
    assert!((at(9_666) - 1.0).abs() < 1e-3);
    assert!((at(11_999) - 1.0).abs() < 1e-6);
    let falling = at(12_083);
    assert!(falling > 0.1 && falling < 0.9, "{falling}");
    assert!(at(12_166).abs() < 1e-3);
    // Monotonic rise.
    let mut last = 0.0;
    for a in 9_500..=9_666 {
        let p = at(a);
        assert!(p >= last - 1e-6, "{a}");
        last = p;
    }
}

#[test]
fn busy_is_true_only_while_a_line_is_lit_or_dimming() {
    let d = doc(vec![line(10_000, Some(12_000), "a")], Granularity::Line);
    let t = Timeline::new(&d);
    let c = |p: i64| Clocks::new(Granularity::Line, p, 0);
    assert!(!t.busy(&c(0)), "before the line: calm");
    assert!(!t.busy(&c(9_384)));
    assert!(t.busy(&c(10_000)));
    assert!(t.busy(&c(11_800)));
    assert!(t.busy(&c(12_000)), "still dimming for 166 ms");
    assert!(!t.busy(&c(12_500)), "done: calm");
    assert!(!t.busy(&c(60_000)));
}

#[test]
fn unsynced_documents_and_lines_without_start_never_light() {
    let plain = doc(vec![line(0, None, "a")], Granularity::Unsynced);
    let tl = Timeline::new(&plain);
    let c = Clocks::new(Granularity::Unsynced, 5_000, 0);
    assert_eq!(
        (tl.window(0), tl.active(&c), tl.busy(&c)),
        (None, None, false)
    );
    let mut untimed = line(0, None, "b");
    untimed.start_ms = None;
    let d = doc(
        vec![line(1000, Some(2000), "a"), untimed],
        Granularity::Line,
    );
    let t = Timeline::new(&d);
    assert!(t.window(1).is_none());
    assert_eq!(
        t.active(&Clocks::new(Granularity::Line, 90_000, 0)),
        Some(0)
    );
}

#[test]
fn a_line_without_word_timing_fades_its_ink_in_over_330_ms() {
    let w = LineWindow {
        start_ms: 10_000,
        end_ms: 12_000,
    };
    let at = |a: i64| Clocks {
        audio_ms: a,
        scroll_ms: a + SCROLL_LEAD_MS,
    };
    assert!(w.ink_progress(&at(9_500)).abs() < 1e-6);
    // 166 ms in, the scale and alpha of the line are done but the ink is only part way.
    assert!((w.lit_progress(&at(9_666)) - 1.0).abs() < 1e-3);
    let part = w.ink_progress(&at(9_666));
    assert!(part > 0.2 && part < 0.9, "{part}");
    assert!((w.ink_progress(&at(9_830)) - 1.0).abs() < 1e-3);
    // It falls as fast as the line dims.
    assert!((w.ink_progress(&at(11_999)) - 1.0).abs() < 1e-6);
    assert!(w.ink_progress(&at(12_166)).abs() < 1e-3);
}
