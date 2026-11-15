//! The filter chips under the search box, like YouTube Music's (All first), and the answer of
//! one chip: that kind in full, in YouTube Music's order.
//!
//! Each chip is a pill button (32 px high, `shape.full`), so Tab reaches it and Enter or Space
//! picks it. The picked chip takes the cover's `secondary-container`.

use iced::Element;
use iced::widget::{column, row};
use nemo_core::catalog::SearchKind;

use super::super::super::app::{App, Message};
use super::super::super::assets::Icon;
use super::super::kit::{CHIP_GAP, chip, status};
use super::section::{self, Frame};
use super::{no_results, searching};
use crate::state::Event;

/// The label of a chip.
#[must_use]
pub(super) fn label(kind: Option<SearchKind>) -> &'static str {
    match kind {
        None => "All",
        Some(SearchKind::Songs) => "Songs",
        Some(SearchKind::Videos) => "Videos",
        Some(SearchKind::Albums) => "Albums",
        Some(SearchKind::Artists) => "Artists",
        Some(SearchKind::Playlists) => "Community playlists",
        Some(SearchKind::Channels) => "Channels",
    }
}

/// All, then one chip per [`SearchKind::FILTERS`].
pub(super) fn chips(app: &App, picked: Option<SearchKind>) -> Element<'_, Message> {
    let accent = app.accent();
    let mut r = row![].spacing(CHIP_GAP);
    let kinds = std::iter::once(None).chain(SearchKind::FILTERS.into_iter().map(Some));
    for kind in kinds {
        r = r.push(chip(
            label(kind),
            kind == picked,
            accent,
            Message::Ui(Event::SearchFilter(kind)),
        ));
    }
    r.into()
}

/// The answer of chip `kind`: its section in full, or the state of its search.
pub(super) fn filtered<'a>(app: &'a App, kind: SearchKind, frame: &Frame) -> Element<'a, Message> {
    let search = &app.ui.search;
    let Some(answer) = search.filtered_now() else {
        return searching();
    };
    match &answer.result {
        Ok(r) if r.count(kind) == 0 => no_results(search.query()),
        Ok(r) => section::section(app, r, kind, frame, 0).unwrap_or_else(|| column![].into()),
        Err(e) => status(
            Icon::Info,
            "Search did not work",
            format!("{e} Check your connection and try again."),
            Some(("Try again", Message::Ui(Event::SearchFilter(Some(kind))))),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_filter_has_its_own_label_and_all_comes_first() {
        let labels: Vec<&str> = std::iter::once(None)
            .chain(SearchKind::FILTERS.into_iter().map(Some))
            .map(label)
            .collect();
        assert_eq!(
            labels,
            [
                "All",
                "Songs",
                "Videos",
                "Albums",
                "Artists",
                "Community playlists"
            ]
        );
    }
}
