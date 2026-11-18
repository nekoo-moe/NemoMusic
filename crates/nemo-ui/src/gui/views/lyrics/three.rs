//! Three lyric lines (previous, current, next) for the compact full-window layout and the
//! Classic style. Each line is a one-line [`LyricList`] in the same size and weight as the rest
//! of the lyrics and lit by the same clocks. While the intro or gap note waits, it takes the
//! middle slot, above the line it waits for (at the very start of the song it takes the top
//! slot, as line 1 does).

use std::borrow::Cow;

use iced::widget::{column, container};
use iced::{Alignment, Element, Length};
use nemo_core::lyrics::LyricsDoc;

use super::super::super::app::{App, Message};
use super::super::super::assets::display_font;
use super::super::super::lyric_list::LyricList;
use super::super::super::style::col;
use super::{note, rows_at, wrap_width};
use crate::lyrics_view::metrics::Metrics;
use crate::theme::color;

/// The three lines of `doc`, `width` px wide.
pub fn lines<'a>(app: &'a App, doc: &'a LyricsDoc, width: f32) -> Element<'a, Message> {
    let focus = app.ui.lyric_focus();
    let active = focus.active;
    let note = focus.note.filter(|n| n.presence > 0.0);
    let m = Metrics::compact();
    // With the note waiting, the line it waits for is the centre; while it folds away, the
    // window stays put so that line slides up into the note's place.
    let (first, count) = match note {
        Some(n) if n.before > 0 => (n.before - 1, 2),
        Some(_) => (0, 2),
        None => (
            active
                .unwrap_or(0)
                .saturating_sub(1)
                .min(doc.lines.len().saturating_sub(3)),
            3,
        ),
    };
    let clocks = app.ui.lyric_clocks();
    let mut c = column![]
        .align_x(Alignment::Center)
        .spacing(m.gap)
        .width(Length::Fill);
    for (i, line) in doc.lines.iter().enumerate().skip(first).take(count) {
        if let Some(n) = note.filter(|n| n.before == i) {
            c = c.push(note::row(n, &m, 0.0).0);
        }
        let Some(clocks) = clocks else {
            continue;
        };
        let rows = rows_at(&line.text, width, m.size);
        let slot = m.slot_height(rows);
        let list = LyricList {
            doc,
            rev: app.ui.lyrics_rev,
            first: i,
            tops: Cow::Owned(vec![0.0, slot]),
            metrics: m,
            wrap_w: wrap_width(width),
            // The text starts `gap` below the slot top: scroll the slot up so it fills the row.
            scroll: m.gap,
            timeline: &app.ui.lyric_timeline,
            clocks,
            note: None,
            ink: col(color::on_surface()),
            font: display_font(true),
            reduced: app.ui.reduced_motion,
            fade_edges: false,
            cascade: None,
        };
        c = c.push(
            container(Element::from(list))
                .width(Length::Fill)
                .height(Length::Fixed(m.text_height(rows))),
        );
    }
    c.into()
}
