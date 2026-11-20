//! The SoundCloud group of `All` in Search (soundcloud-account, owner decision 2026-11-20): under
//! YouTube Music's results, a heading and the same sections (songs as rows, albums, artists and
//! playlists as cards) of SoundCloud's answer. A search that runs, one that failed and one with
//! no results each say so in a plain sentence.

use iced::Element;
use iced::widget::column;
use nemo_core::catalog::SearchResults;

use super::super::super::app::{App, Message, ScGroupPhase};
use super::super::super::assets::Icon;
use super::super::kit::{page_title, paragraph, status, vspace};
use super::section::{self, Frame};
use crate::gui::style::{col, gap};
use crate::state::Event;
use crate::theme::{color, type_scale};

/// The group of the text and chip on screen, or `None` when the page is not set to `All` (or
/// the chip has no SoundCloud kind, such as Videos).
pub(super) fn view<'a>(app: &'a App, frame: &Frame) -> Option<Element<'a, Message>> {
    let group = app.sc_group_now()?;
    let dim = col(color::on_surface_variant());
    let body: Element<'a, Message> = match &group.phase {
        ScGroupPhase::Loading => paragraph(
            "Searching SoundCloud\u{2026}",
            type_scale::BODY_MD,
            dim,
            frame.width,
        ),
        ScGroupPhase::Failed => status(
            Icon::Info,
            "SoundCloud did not answer",
            "The YouTube Music results above are not affected.",
            Some(("Try again", Message::Ui(Event::SearchRetry))),
        ),
        ScGroupPhase::Done(r) if r.is_empty() => paragraph(
            format!("No SoundCloud results for \u{201c}{}\u{201d}.", group.query),
            type_scale::BODY_MD,
            dim,
            frame.width,
        ),
        ScGroupPhase::Done(r) => sections(app, r, frame),
    };
    Some(
        column![page_title("SoundCloud"), vspace(gap::MD), body]
            .push(vspace(gap::XXL))
            .into(),
    )
}

/// Whether the group on screen has something to show (the YouTube Music "no results" sentence
/// then stays away).
pub(super) fn has_results(app: &App) -> bool {
    matches!(
        app.sc_group_now().map(|g| &g.phase),
        Some(ScGroupPhase::Done(r)) if !r.is_empty()
    )
}

/// SoundCloud's sections in the order its search ranked them. The songs play from the group's
/// own list, not YouTube Music's.
fn sections<'a>(app: &'a App, r: &SearchResults, frame: &Frame) -> Element<'a, Message> {
    let mut c = column![];
    for (i, kind) in r.sections().into_iter().enumerate() {
        if let Some(s) = section::sc_section(app, r, kind, frame, i) {
            c = c.push(s);
        }
    }
    c.into()
}
