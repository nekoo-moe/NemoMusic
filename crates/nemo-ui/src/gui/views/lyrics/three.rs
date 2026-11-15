//! Three lyric lines (previous, current, next) for the compact full-window layout and the
//! Classic style. While the intro or gap note waits, it takes the middle slot, above the line
//! it waits for (at the very start of the song it takes the top slot, as line 1 does).

use iced::widget::column;
use iced::{Alignment, Element, Length};
use nemo_core::lyrics::LyricsDoc;

use super::super::super::app::{App, Message};
use super::{Look, lyric_line, note};
use crate::lyrics_view::LineRole;
use crate::theme::{TextStyle, type_scale};

/// Type style of a role here: one step smaller than the wide layout, so three lines fit.
fn style(role: LineRole) -> TextStyle {
    match role {
        LineRole::Active => type_scale::TITLE_MD,
        _ => type_scale::BODY_MD,
    }
}

/// The three lines of `doc`, `width` px wide.
pub fn lines<'a>(app: &App, doc: &'a LyricsDoc, width: f32) -> Element<'a, Message> {
    let focus = app.ui.lyric_focus();
    let active = focus.active;
    let role_of = |i: usize, at: Option<usize>| {
        if at.is_some() {
            LineRole::of(i, at)
        } else {
            LineRole::Other
        }
    };
    let before = if app.anim.line() < 1.0 {
        app.prev_line
    } else {
        active
    };
    let note = focus.note.filter(|n| n.presence > 0.0);
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
    let mut c = column![].align_x(Alignment::Center).width(Length::Fill);
    for (i, line) in doc.lines.iter().enumerate().skip(first).take(count) {
        if let Some(n) = note.filter(|n| n.before == i) {
            c = c.push(note::row_for(n, style(LineRole::Active)).0);
        }
        let (from, to) = (role_of(i, before), role_of(i, active));
        let look = Look {
            from: (from, style(from)),
            to: (to, style(to)),
        };
        c = c.push(lyric_line(app, line, width, look).0);
    }
    c.into()
}
