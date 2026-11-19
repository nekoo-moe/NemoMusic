//! The filter chips under the search box, like YouTube Music's (All first), and the answer of
//! one chip: that kind in full, in YouTube Music's order.
//!
//! Each chip is a pill button (32 px high, `shape.full`), so Tab reaches it and Enter or Space
//! picks it. The picked chip takes the cover's `secondary-container`.

use iced::Element;
use iced::widget::column;
use nemo_core::catalog::SearchKind;

use super::super::super::app::{App, Message};
use super::super::super::assets::Icon;
use super::super::kit::{blank, sliding_chips, status};
use super::no_results;
use super::section::{self, Frame};
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

/// The labels of the chips, All first.
#[must_use]
pub(crate) fn labels() -> Vec<&'static str> {
    std::iter::once(None)
        .chain(SearchKind::FILTERS.into_iter().map(Some))
        .map(label)
        .collect()
}

/// Position of the chip of `picked` in [`labels`].
#[must_use]
pub(crate) fn index(picked: Option<SearchKind>) -> usize {
    picked.map_or(0, |k| {
        SearchKind::FILTERS
            .iter()
            .position(|f| *f == k)
            .map_or(0, |i| i + 1)
    })
}

/// The kind of the chip at `i` (All is 0).
fn kind_at(i: usize) -> Option<SearchKind> {
    i.checked_sub(1)
        .and_then(|j| SearchKind::FILTERS.get(j).copied())
}

/// All, then one chip per [`SearchKind::FILTERS`], with a pill that slides to the picked one.
pub(super) fn chips(app: &App, picked: Option<SearchKind>) -> Element<'_, Message> {
    sliding_chips(app, &labels(), index(picked), 2, |i| {
        Message::Ui(Event::SearchFilter(kind_at(i)))
    })
}

/// The answer of chip `kind`: its section in full, or the state of its search.
pub(super) fn filtered<'a>(app: &'a App, kind: SearchKind, frame: &Frame) -> Element<'a, Message> {
    let search = &app.ui.search;
    let Some(answer) = search.filtered_now() else {
        return blank();
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

    #[test]
    fn a_chip_index_round_trips_to_its_kind() {
        assert_eq!(index(None), 0);
        assert_eq!(kind_at(0), None);
        for (i, kind) in SearchKind::FILTERS.into_iter().enumerate() {
            assert_eq!(index(Some(kind)), i + 1);
            assert_eq!(kind_at(i + 1), Some(kind));
        }
        assert_eq!(kind_at(99), None);
        assert_eq!(labels().len(), SearchKind::FILTERS.len() + 1);
    }
}
