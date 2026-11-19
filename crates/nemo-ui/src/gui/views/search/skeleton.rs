//! The Search answer while it is on its way: the Top result and the sections of All, or the
//! kind of the picked chip, as skeleton blocks. The chips above stay real.

use iced::Element;
use iced::widget::column;
use nemo_core::catalog::SearchKind;

use super::section::Frame;
use crate::gui::app::Message;
use crate::gui::style::gap;
use crate::gui::views::kit::{Skeleton, vspace};

/// Rows of the Songs preview.
const PREVIEW_ROWS: usize = 4;
/// Rows of one filtered list.
const FULL_ROWS: usize = 6;
/// Card rows of a preview and of a filtered kind.
const PREVIEW_CARD_ROWS: usize = 1;
const FULL_CARD_ROWS: usize = 2;

/// A titled block of `rows` track rows.
fn rows<'a>(sk: Skeleton, width: f32, rows: usize) -> Element<'a, Message> {
    let mut list = column![sk.line(width * 0.12, crate::theme::type_scale::TITLE_LG)];
    list = list.push(vspace(gap::SM));
    for i in 0..rows {
        list = list.push(sk.row(width, i));
    }
    list.push(vspace(gap::XXL)).into()
}

/// A titled block of `card_rows` rows of cards.
fn cards<'a>(sk: Skeleton, frame: &Frame, card_rows: usize, round: bool) -> Element<'a, Message> {
    column![
        sk.section_header(frame.width * 0.12),
        sk.cards(frame.grid, card_rows, round),
        vspace(gap::XXL),
    ]
    .into()
}

/// The skeleton of the answer for `kind` (`None` is All).
pub(super) fn view<'a>(
    sk: Skeleton,
    frame: &Frame,
    kind: Option<SearchKind>,
) -> Element<'a, Message> {
    let w = frame.width;
    match kind {
        None => column![
            sk.top_result(w),
            rows(sk, w, PREVIEW_ROWS),
            cards(sk, frame, PREVIEW_CARD_ROWS, false),
        ]
        .into(),
        Some(SearchKind::Songs | SearchKind::Videos) => rows(sk, w, FULL_ROWS),
        Some(SearchKind::Albums | SearchKind::Playlists) => cards(sk, frame, FULL_CARD_ROWS, false),
        Some(SearchKind::Artists) => cards(sk, frame, FULL_CARD_ROWS, true),
        Some(SearchKind::Channels) => column![].into(),
    }
}
