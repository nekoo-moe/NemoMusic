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

#[cfg(test)]
mod tests {
    use iced::Length;
    use iced::widget::container;
    use nemo_core::catalog::TopResult;

    use super::super::top::top_card;
    use crate::gui::headless::{Scenario, build_app};
    use crate::gui::views::kit::{Skeleton, size_of};
    use crate::mock::{mock_backend, track};

    #[test]
    #[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
    fn the_skeleton_top_result_is_as_big_as_the_top_result_card() {
        let app = build_app(&mock_backend(), &Scenario::default());
        let top = TopResult::Song(track(0));
        for width in [560.0_f32, 677.0, 1000.0] {
            let real = size_of(
                container(top_card(&app, &top, width))
                    .width(Length::Fixed(width))
                    .into(),
            );
            let sk = size_of(Skeleton::still().top_result(width));
            assert!(
                (sk.width - real.width).abs() < 0.51 && (sk.height - real.height).abs() < 0.51,
                "{width}: skeleton {sk:?} against real {real:?}"
            );
        }
    }
}
