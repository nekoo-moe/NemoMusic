//! Where the lyric lines sit in the list (`lyrics-rebuild` spec). Every line has one slot of its
//! wrapped rows plus [`Metrics::gap`] above and below, all in one size, so the layout never
//! depends on which line is lit. Pure: the reducer anchors the scroll on the same numbers the
//! list widget draws with.

use std::ops::Range;

use super::ANCHOR;
use super::metrics::Metrics;
use super::note::{Focus, GapNote};

/// Wrapped row counts of the lines, one per line.
pub type Rows = u8;

/// The top of every slot, px from the top of the list, and the total height as the last entry:
/// `tops(..).len() == rows.len() + 1`.
#[must_use]
pub fn tops(rows: &[Rows], m: &Metrics) -> Vec<f32> {
    let mut out = Vec::with_capacity(rows.len() + 1);
    let mut y = 0.0;
    out.push(y);
    for r in rows {
        y += m.slot_height(*r);
        out.push(y);
    }
    out
}

/// Height of the intro or gap note row: one line slot times the note's presence, so it folds away
/// while it fades out.
#[must_use]
pub fn note_height(m: &Metrics, n: &GapNote) -> f32 {
    m.slot_height(1) * n.presence
}

/// Extra space the note takes above line `i`: its row height for the lines it sits before.
#[must_use]
pub fn shift(note: Option<&GapNote>, m: &Metrics, i: usize) -> f32 {
    note.filter(|n| i >= n.before)
        .map_or(0.0, |n| note_height(m, n))
}

/// Top of slot `i` in the list, px, with the note's row counted.
#[must_use]
pub fn top_of(tops: &[f32], note: Option<&GapNote>, m: &Metrics, i: usize) -> f32 {
    tops.get(i).copied().unwrap_or(0.0) + shift(note, m, i)
}

/// The scroll offset (px from the top of the list) that puts the top of the focused line's text
/// (or of the waiting note) at [`ANCHOR`] of the viewport. With nothing focused yet the first
/// line takes the anchor. The offset goes below 0 for the first lines, so they sit at the anchor
/// too instead of under the fade at the top. A line too tall for the room under the anchor moves
/// up until its last row shows.
#[must_use]
pub fn scroll_target(tops: &[f32], m: &Metrics, focus: &Focus, viewport_h: f32) -> f32 {
    if tops.len() < 2 {
        return 0.0;
    }
    let (top, own) = match (focus.active, focus.note) {
        (None, Some(n)) => (top_raw(tops, n.before), m.line_h),
        (Some(a), note) => {
            let slot = tops.get(a + 1).copied().unwrap_or(0.0) - top_raw(tops, a);
            (
                top_raw(tops, a) + shift(note.as_ref(), m, a),
                (slot - 2.0 * m.gap).max(m.line_h),
            )
        }
        (None, None) => {
            let slot = tops.get(1).copied().unwrap_or(0.0) - top_raw(tops, 0);
            (top_raw(tops, 0), (slot - 2.0 * m.gap).max(m.line_h))
        }
    };
    let text_top = top + m.gap;
    let anchored = text_top - ANCHOR * viewport_h;
    let fits = text_top + own - viewport_h;
    anchored.max(fits).min(text_top)
}

fn top_raw(tops: &[f32], i: usize) -> f32 {
    tops.get(i).copied().unwrap_or(0.0)
}

/// The lines whose slots touch the viewport at `scroll`, widened by `margin` px on both sides.
#[must_use]
pub fn visible(
    tops: &[f32],
    note: Option<&GapNote>,
    m: &Metrics,
    scroll: f32,
    viewport_h: f32,
    margin: f32,
) -> Range<usize> {
    let n = tops.len().saturating_sub(1);
    let (lo, hi) = (scroll - margin, scroll + viewport_h + margin);
    let first = (0..n)
        .find(|i| top_of(tops, note, m, i + 1) > lo)
        .unwrap_or(n);
    let last = (first..n)
        .find(|i| top_of(tops, note, m, *i) >= hi)
        .unwrap_or(n);
    first..last
}

#[cfg(test)]
mod tests;
