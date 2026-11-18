//! The intro and gap note row (lyrics-polish task 1): one filled music note where the next
//! line will appear, filling from the bottom with the playback position. The note is drawn in
//! the "coming" ink, and the filled part in `on-surface`, the ink of the active line, so a full
//! note looks like the line it hands over to.
//!
//! The fill is a hard-stop gradient inside the SVG (bounding-box units, so 0 and 1 are the
//! bottom and top of the note shape itself). The level is rounded to [`FILL_STEPS`] steps, so
//! the renderer's SVG cache holds a bounded set of rasters for one song.

use iced::widget::{container, svg};
use iced::{Element, Length, Padding};

use super::super::super::app::Message;
use crate::lyrics_view::metrics::{INACTIVE_ALPHA, Metrics};
use crate::lyrics_view::note::GapNote;
use crate::theme::{Rgba, color};

/// The Material Symbols "music-note" glyph (Apache-2.0), the same file as `Icon::Note`.
const NOTE_SVG: &str = include_str!("../../../../assets/icons/music-note.svg");
/// Fill levels the note can show: about one level per device pixel of a 28 px glyph at 2x.
pub const FILL_STEPS: f32 = 64.0;

fn hex(c: Rgba) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
}

/// The note glyph as SVG, filled from the bottom up to `fill` (0 to 1) in `ink` at `filled`
/// opacity, and in `ink` at `dim` opacity above it.
#[must_use]
pub fn note_svg(fill: f32, ink: Rgba, filled: f32, dim: f32) -> String {
    let level = (fill.clamp(0.0, 1.0) * FILL_STEPS).round() / FILL_STEPS;
    let c = hex(ink);
    let defs = format!(
        "<defs><linearGradient id=\"f\" x1=\"0\" y1=\"1\" x2=\"0\" y2=\"0\">\
         <stop offset=\"{level:.4}\" stop-color=\"{c}\" stop-opacity=\"{filled:.3}\"/>\
         <stop offset=\"{level:.4}\" stop-color=\"{c}\" stop-opacity=\"{dim:.3}\"/>\
         </linearGradient></defs><path fill=\"url(#f)\" "
    );
    NOTE_SVG.replacen("<path ", &defs, 1)
}

/// The note row and its height in px: one line row, with `pad` px above and below it, times the
/// note's presence, so it folds away while it fades out. The glyph is as tall as the text.
pub fn row<'a>(n: GapNote, m: &Metrics, pad: f32) -> (Element<'a, Message>, f32) {
    let height = (m.line_h + 2.0 * pad) * n.presence;
    let ink = color::on_surface();
    let base = ink.alpha();
    let source = note_svg(n.fill, ink, base * n.presence, INACTIVE_ALPHA * n.presence);
    let glyph = svg(svg::Handle::from_memory(source.into_bytes()))
        .width(Length::Fixed(m.size))
        .height(Length::Fixed(m.size));
    let el = container(glyph)
        .padding(Padding {
            top: pad + ((m.line_h - m.size) / 2.0).max(0.0),
            ..Padding::ZERO
        })
        .width(Length::Fill)
        .height(Length::Fixed(height))
        .clip(true);
    (el.into(), height)
}
