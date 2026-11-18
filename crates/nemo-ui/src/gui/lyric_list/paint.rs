//! Painting a cached lyric line without word timing: one colour.

use iced::advanced::text::Renderer as _;
use iced::{Color, Point, Rectangle};

use super::shape::Line;

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
