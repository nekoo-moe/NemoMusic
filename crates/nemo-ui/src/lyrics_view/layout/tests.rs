use super::*;
use crate::lyrics_view::note::GapNote;

fn m() -> Metrics {
    Metrics::full(1366.0)
}

fn focus(active: Option<usize>) -> Focus {
    Focus { active, note: None }
}

fn note(before: usize, presence: f32, waiting: bool) -> GapNote {
    GapNote {
        before,
        fill: 0.5,
        presence,
        waiting,
    }
}

#[test]
fn slots_add_up_and_wrapped_lines_are_taller() {
    let m = m();
    let t = tops(&[1, 2, 1], &m);
    assert_eq!(t.len(), 4);
    assert!(t[0].abs() < 1e-6);
    assert!((t[1] - m.slot_height(1)).abs() < 1e-3);
    assert!((t[2] - t[1] - m.slot_height(2)).abs() < 1e-3);
    assert!((t[3] - t[2] - m.slot_height(1)).abs() < 1e-3);
    assert_eq!(tops(&[], &m), vec![0.0]);
}

#[test]
fn the_active_line_text_sits_at_35_percent() {
    let m = m();
    let t = tops(&[1; 40], &m);
    let vh = 600.0;
    let off = scroll_target(&t, &m, &focus(Some(20)), vh);
    let text_top = t[20] + m.gap;
    assert!((text_top - off - 0.35 * vh).abs() < 1e-3, "{off}");
    assert!(scroll_target(&t, &m, &focus(Some(21)), vh) > off);
    // The first lines are above the anchor already: no negative scroll.
    assert!(scroll_target(&t, &m, &focus(Some(0)), vh).abs() < 1e-6);
    assert!(scroll_target(&t, &m, &focus(None), vh).abs() < 1e-6);
}

#[test]
fn wrapped_lines_above_move_the_anchor_by_their_real_height() {
    let m = m();
    let one = tops(&[1; 30], &m);
    let mut rows = [1_u8; 30];
    rows[5] = 3;
    let wrapped = tops(&rows, &m);
    let a = scroll_target(&one, &m, &focus(Some(20)), 600.0);
    let b = scroll_target(&wrapped, &m, &focus(Some(20)), 600.0);
    assert!((b - a - 2.0 * m.line_h).abs() < 1e-3, "{a} {b}");
}

#[test]
fn a_line_too_tall_for_the_room_under_the_anchor_moves_up_to_show_its_last_row() {
    let m = m();
    let mut rows = [1_u8; 5];
    rows[2] = 5;
    let t = tops(&rows, &m);
    let vh = 420.0;
    let off = scroll_target(&t, &m, &focus(Some(2)), vh);
    let text_top = t[2] + m.gap;
    let text_bottom = text_top + 5.0 * m.line_h;
    assert!(text_bottom - off <= vh + 1e-3, "{off}");
    assert!(off <= text_top + 1e-3, "its top is never cut off");
}

#[test]
fn the_waiting_note_takes_the_anchor_and_the_fading_note_still_counts_above_its_line() {
    let m = m();
    let t = tops(&[1; 30], &m);
    let vh = 600.0;
    let waiting = Focus {
        active: None,
        note: Some(note(20, 1.0, true)),
    };
    let off = scroll_target(&t, &m, &waiting, vh);
    assert!((t[20] + m.gap - off - 0.35 * vh).abs() < 1e-3);
    // Fading out: the line has started, the note row shrinks and the line moves up.
    let full = Focus {
        active: Some(20),
        note: Some(note(20, 1.0, false)),
    };
    let half = Focus {
        active: Some(20),
        note: Some(note(20, 0.5, false)),
    };
    let gone = focus(Some(20));
    let (at_full, at_half, at_gone) = (
        scroll_target(&t, &m, &full, vh),
        scroll_target(&t, &m, &half, vh),
        scroll_target(&t, &m, &gone, vh),
    );
    assert!(
        at_full > at_half && at_half > at_gone,
        "{at_full} {at_half} {at_gone}"
    );
    assert!((at_full - at_gone - m.slot_height(1)).abs() < 1e-3);
}

#[test]
fn top_of_counts_the_note_row_for_the_lines_after_it() {
    let m = m();
    let t = tops(&[1; 5], &m);
    let n = note(2, 0.5, true);
    assert!((top_of(&t, Some(&n), &m, 1) - t[1]).abs() < 1e-6);
    assert!((top_of(&t, Some(&n), &m, 2) - t[2] - 0.5 * m.slot_height(1)).abs() < 1e-3);
    assert!((top_of(&t, None, &m, 4) - t[4]).abs() < 1e-6);
    assert!(top_of(&t, None, &m, 99).abs() < 1e-6, "out of range is 0");
}

#[test]
fn visible_returns_the_lines_touching_the_viewport() {
    let m = m();
    let t = tops(&[1; 100], &m);
    let slot = m.slot_height(1);
    let r = visible(&t, None, &m, 10.0 * slot, 3.0 * slot, 0.0);
    assert_eq!(r, 10..13);
    let wide = visible(&t, None, &m, 10.0 * slot, 3.0 * slot, slot);
    assert_eq!(wide, 9..14);
    assert_eq!(visible(&t, None, &m, 0.0, 2.0 * slot, 0.0), 0..2);
    assert_eq!(visible(&t, None, &m, 1e9, 500.0, 0.0), 100..100);
    assert_eq!(visible(&[0.0], None, &m, 0.0, 500.0, 0.0), 0..0);
}
