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

/// The kinds of the chips in order, `None` (All) first. SoundCloud alone has no videos, so that
/// chip is left out then (`with_videos` is `false`).
fn kinds(with_videos: bool) -> Vec<Option<SearchKind>> {
    std::iter::once(None)
        .chain(
            SearchKind::FILTERS
                .into_iter()
                .filter(|k| with_videos || *k != SearchKind::Videos)
                .map(Some),
        )
        .collect()
}

/// The labels of the chips, All first.
#[must_use]
pub(crate) fn labels(with_videos: bool) -> Vec<&'static str> {
    kinds(with_videos).into_iter().map(label).collect()
}

/// Position of the chip of `picked` in [`labels`]. A chip that is not there (Videos for
/// SoundCloud alone) counts as All.
#[must_use]
pub(crate) fn index(with_videos: bool, picked: Option<SearchKind>) -> usize {
    kinds(with_videos)
        .iter()
        .position(|k| *k == picked)
        .unwrap_or(0)
}

/// The kind of the chip at `i` (All is 0).
fn kind_at(with_videos: bool, i: usize) -> Option<SearchKind> {
    kinds(with_videos).get(i).copied().flatten()
}

/// The message of the chip at `i` with the Videos chip present.
fn pick_with_videos(i: usize) -> Message {
    Message::Ui(Event::SearchFilter(kind_at(true, i)))
}

/// The message of the chip at `i` without the Videos chip.
fn pick_without_videos(i: usize) -> Message {
    Message::Ui(Event::SearchFilter(kind_at(false, i)))
}

/// All, then one chip per [`SearchKind::FILTERS`] (no Videos for SoundCloud alone), with a pill
/// that slides to the picked one.
pub(super) fn chips(app: &App, picked: Option<SearchKind>) -> Element<'_, Message> {
    let with_videos = !app.search_is_soundcloud();
    let pick = if with_videos {
        pick_with_videos
    } else {
        pick_without_videos
    };
    sliding_chips(
        app,
        &labels(with_videos),
        index(with_videos, picked),
        2,
        pick,
    )
}

/// The answer of chip `kind`: its section in full, or the state of its search. With `All` in
/// the source switch, the SoundCloud group of the same chip follows.
pub(super) fn filtered<'a>(app: &'a App, kind: SearchKind, frame: &Frame) -> Element<'a, Message> {
    let search = &app.ui.search;
    let Some(answer) = search.filtered_now() else {
        return blank();
    };
    let own: Option<Element<'a, Message>> = match &answer.result {
        Ok(r) if r.count(kind) == 0 => {
            (!super::sc_group::has_results(app)).then(|| no_results(search.query()))
        }
        Ok(r) => section::section(app, r, kind, frame, 0),
        Err(e) => Some(status(
            Icon::Info,
            "Search did not work",
            format!("{e} Check your connection and try again."),
            Some(("Try again", Message::Ui(Event::SearchFilter(Some(kind))))),
        )),
    };
    let mut c = column![];
    if let Some(own) = own {
        c = c.push(own);
    }
    if let Some(group) = super::sc_group::view(app, frame) {
        c = c.push(group);
    }
    c.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_filter_has_its_own_label_and_all_comes_first() {
        assert_eq!(
            labels(true),
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
    fn soundcloud_alone_has_no_videos_chip() {
        assert_eq!(
            labels(false),
            ["All", "Songs", "Albums", "Artists", "Community playlists"]
        );
        // The chips after the missing one keep working.
        assert_eq!(kind_at(false, 2), Some(SearchKind::Albums));
        assert_eq!(index(false, Some(SearchKind::Albums)), 2);
        // A picked Videos chip counts as All there (the switch also forgets it).
        assert_eq!(index(false, Some(SearchKind::Videos)), 0);
    }

    #[test]
    fn a_chip_index_round_trips_to_its_kind() {
        for with_videos in [true, false] {
            assert_eq!(index(with_videos, None), 0);
            assert_eq!(kind_at(with_videos, 0), None);
            for (i, kind) in kinds(with_videos).into_iter().enumerate().skip(1) {
                assert_eq!(index(with_videos, kind), i);
                assert_eq!(kind_at(with_videos, i), kind);
            }
            assert_eq!(kind_at(with_videos, 99), None);
            assert_eq!(labels(with_videos).len(), kinds(with_videos).len());
        }
    }
}
