//! Painting one cached lyric line: plain, or a word-timed line with its bright words over grey.

use iced::advanced::text::Renderer as _;
use iced::{Color, Point, Rectangle};

use super::shape::Line;
use crate::lyric_pieces;

/// Room kept around a bright clip so the side bearings of glyphs are not cut, px.
const PAD: f32 = 2.0;

/// `c` with alpha `a` (0 to 1).
pub(super) fn with_alpha(c: Color, a: f32) -> Color {
    Color {
        a: a.clamp(0.0, 1.0),
        ..c
    }
}

/// A line without word timing: one colour.
pub(super) fn plain(r: &mut iced::Renderer, line: &Line, pos: Point, ink: Color, clip: Rectangle) {
    r.fill_paragraph(&line.para, pos, ink, clip);
}

/// A word-timed line: the whole line in `grey`, then every word that has started drawn again in
/// `bright`, clipped to the part of it that is sung at `t_ms`.
pub(super) fn timed(
    r: &mut iced::Renderer,
    line: &Line,
    pos: Point,
    (grey, bright): (Color, Color),
    t_ms: i64,
    reduced: bool,
    clip: Rectangle,
) {
    r.fill_paragraph(&line.para, pos, grey, clip);
    if bright.a <= 0.0 {
        return;
    }
    for (boxes, timing) in line.boxes.iter().zip(&line.timings) {
        let share = lyric_pieces::fx_at(t_ms, *timing, reduced).fill;
        if share <= 0.0 {
            // Words are in order: none of the later ones has started either.
            break;
        }
        for b in boxes {
            let part = sung_part(b, pos, share, line.rtl);
            if let Some(c) = part.intersection(&clip) {
                r.fill_paragraph(&line.para, pos, bright, c);
            }
        }
    }
}

/// The part of word box `b` (relative to the paragraph at `pos`) that is bright when `share`
/// of it is sung: from the left, or from the right for right-to-left text.
fn sung_part(b: &Rectangle, pos: Point, share: f32, rtl: bool) -> Rectangle {
    let whole = share >= 1.0;
    let (x, w) = (pos.x + b.x, b.width);
    let width = w * share.clamp(0.0, 1.0);
    let (left, right) = if rtl {
        (x + w - width, x + w + PAD)
    } else {
        (x - PAD, x + width)
    };
    // The far side of a finished word gets room for the glyph edge too.
    let (left, right) = match (rtl, whole) {
        (true, true) => (left - PAD, right),
        (false, true) => (left, right + PAD),
        _ => (left, right),
    };
    Rectangle {
        x: left,
        y: pos.y + b.y,
        width: (right - left).max(0.0),
        height: b.height,
    }
}
