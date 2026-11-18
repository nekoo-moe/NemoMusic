use super::*;
use crate::lyrics_view::{LineRows, scroll_target_focus, scroll_target_rows};
use nemo_core::lyrics::{LyricWord, ProviderId};

/// A line `(start, end, text)`.
fn line(start: u64, end: Option<u64>, text: &str) -> LyricLine {
    LyricLine {
        start_ms: Some(start),
        end_ms: end,
        text: text.to_owned(),
        words: Vec::new(),
    }
}

fn doc(lines: Vec<LyricLine>) -> LyricsDoc {
    LyricsDoc {
        lines,
        granularity: Granularity::Line,
        provider: ProviderId::new("Mock"),
        matched: None,
        instrumental: false,
    }
}

/// Intro to 10 s, lines at 10 s and 13 s (short pause), then a 7 s break after 16 s.
fn song() -> LyricsDoc {
    doc(vec![
        line(10_000, Some(12_500), "first line"),
        line(13_000, Some(16_000), "second line"),
        line(23_000, Some(26_000), "after the break"),
    ])
}

/// Positions below were written for a 150 ms lead; the scroll clock now leads the position by
/// 615 ms for line-synced lyrics (500 ms scroll lead + 115 ms sync offset).
const SHIFT: i64 = 465;

/// The note at `pos` ms (written for the old 150 ms lead), motion on.
fn at(d: &LyricsDoc, pos: i64) -> Option<GapNote> {
    gap_note(d, pos - SHIFT, 0, false)
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn the_intro_note_fills_up_to_the_first_line() {
    let d = song();
    // The first line starts (becomes active) at 10 000 - 150 ms on the position clock.
    let n = at(&d, 0).expect("note in the intro");
    assert_eq!(n.before, 0);
    assert!(n.waiting && close(n.presence, 1.0));
    assert!(close(n.fill, 150.0 / 10_000.0), "{}", n.fill);
    let half = at(&d, 4_850).expect("note");
    assert!(close(half.fill, 0.5), "{}", half.fill);
    let late = at(&d, 9_849).expect("note");
    assert!(late.waiting && late.fill < 1.0 && late.fill > 0.999);
    assert_eq!(focus(&d, 4_850 - SHIFT, 0, false).active, None);
}

#[test]
fn the_note_is_full_at_the_exact_line_start_then_fades_with_motion_fast() {
    let d = song();
    let start = at(&d, 9_850).expect("note at the line start");
    assert!(!start.waiting);
    assert!(close(start.fill, 1.0), "full at the line start");
    assert!(close(start.presence, 1.0));
    // The line is active in the same frame.
    assert_eq!(focus(&d, 9_850 - SHIFT, 0, false).active, Some(0));
    let mid = at(&d, 9_900).expect("fading");
    assert!(close(mid.presence, 0.5) && close(mid.fill, 1.0));
    assert_eq!(at(&d, 9_950), None, "gone after motion.fast");
}

#[test]
fn reduced_motion_keeps_the_fill_and_drops_the_fades() {
    let d = song();
    let n = gap_note(&d, 4_850 - SHIFT, 0, true).expect("note");
    assert!(close(n.fill, 0.5) && close(n.presence, 1.0));
    assert_eq!(gap_note(&d, 9_850 - SHIFT, 0, true), None, "no fade-out");
    // In a gap, no fade-in either.
    let g = gap_note(&d, 15_851 - SHIFT, 0, true).expect("note");
    assert!(close(g.presence, 1.0));
}

#[test]
fn a_long_gap_gets_the_note_from_the_end_of_the_line() {
    let d = song();
    // Line 1 ends at 16 000 (15 850 on the position clock); line 2 starts at 22 850.
    assert_eq!(at(&d, 15_840), None, "still singing");
    let n = at(&d, 15_900).expect("gap note");
    assert_eq!(n.before, 2);
    assert!(close(n.presence, 0.5), "fades in with motion.fast");
    let half = at(&d, 19_350).expect("note");
    assert!(close(half.fill, 0.5), "{}", half.fill);
    assert_eq!(focus(&d, 19_350 - SHIFT, 0, false).active, None);
    let full = at(&d, 22_850).expect("note");
    assert!(close(full.fill, 1.0) && !full.waiting);
    assert_eq!(focus(&d, 22_850 - SHIFT, 0, false).active, Some(2));
}

#[test]
fn a_short_gap_gets_no_note() {
    let d = song();
    // 12 500 to 13 000: half a second.
    assert_eq!(at(&d, 12_600), None);
    // Just under the threshold: no note either.
    let near = doc(vec![line(1_000, Some(2_000), "a"), line(6_999, None, "b")]);
    assert_eq!(at(&near, 4_000), None);
    let exact = doc(vec![line(1_000, Some(2_000), "a"), line(7_000, None, "b")]);
    assert!(at(&exact, 4_000).is_some());
}

#[test]
fn a_blank_line_marks_the_break_when_lines_have_no_end() {
    let d = doc(vec![
        line(1_000, None, "sung"),
        line(4_000, None, ""),
        line(12_000, None, "back"),
    ]);
    // The break runs from the blank line (4 000) to 12 000.
    assert_eq!(at(&d, 3_000), None);
    let n = at(&d, 7_850).expect("note");
    assert_eq!(n.before, 2);
    assert!(close(n.fill, 0.5), "{}", n.fill);
    // Without the blank line nothing tells a break from a long line.
    let plain = doc(vec![line(1_000, None, "sung"), line(12_000, None, "back")]);
    assert_eq!(at(&plain, 7_850), None);
}

#[test]
fn word_timing_gives_the_end_of_a_line() {
    let mut first = line(1_000, None, "one two");
    first.words = vec![
        LyricWord {
            start_ms: 1_000,
            end_ms: Some(1_500),
            text: "one ".to_owned(),
        },
        LyricWord {
            start_ms: 1_500,
            end_ms: Some(2_000),
            text: "two".to_owned(),
        },
    ];
    let mut d = doc(vec![first, line(9_000, None, "three")]);
    d.granularity = Granularity::Word;
    assert!(at(&d, 5_000).is_some());
}

#[test]
fn unsynced_instrumental_and_short_intros_get_no_note() {
    let mut d = song();
    d.granularity = Granularity::Unsynced;
    assert_eq!(at(&d, 0), None);
    let mut inst = song();
    inst.instrumental = true;
    assert_eq!(at(&inst, 0), None);
    let quick = doc(vec![line(600, None, "right away")]);
    assert_eq!(at(&quick, 0), None);
}

#[test]
fn the_offset_moves_the_note_with_the_lines() {
    let d = song();
    // +2 s shows lines 2 s earlier, so the intro is half done 2 s sooner.
    let n = gap_note(&d, 2_850 - SHIFT, 2_000, false).expect("note");
    assert!(close(n.fill, 0.5), "{}", n.fill);
}

#[test]
fn the_waiting_note_takes_the_anchor() {
    let lines: Vec<LyricLine> = (0..20)
        .map(|i| line(30_000 + 4_000 * i, Some(32_000 + 4_000 * i), "la la"))
        .chain([line(200_000, Some(203_000), "after")])
        .collect();
    let d = doc(lines);
    let rows: Vec<LineRows> = vec![[1, 1, 1]; d.lines.len()];
    let vh = 600.0;
    // In the long break before line 20 the note row sits at 35%, below 20 "other" lines.
    let f = focus(&d, 150_000 - SHIFT, 0, false);
    assert_eq!((f.active, f.note.map(|n| n.before)), (None, Some(20)));
    let above = 20.0 * crate::lyrics_view::row_height(LineRole::Other);
    let off = scroll_target_focus(&f, vh, false, &rows);
    assert!((above - off - 0.35 * vh).abs() < 1e-3, "{off}");
    // Without a note the target is the usual one.
    let plain = focus(&d, 31_000 - SHIFT, 0, false);
    assert_eq!(plain.note, None);
    assert!(
        (scroll_target_focus(&plain, vh, false, &rows)
            - scroll_target_rows(plain.active, vh, false, &rows))
        .abs()
            < 1e-3
    );
    // While the note fades out, its shrinking row still sits above the line.
    let fading = focus(&d, 199_900 - SHIFT, 0, false);
    let n = fading.note.expect("fading note");
    assert!(!n.waiting && n.presence > 0.0);
    let with = scroll_target_focus(&fading, vh, false, &rows);
    let without = scroll_target_rows(fading.active, vh, false, &rows);
    assert!((with - without - row_height(false) * n.presence).abs() < 1e-3);
}
