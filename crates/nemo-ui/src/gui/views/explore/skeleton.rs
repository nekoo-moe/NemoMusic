//! Explore before its first answer: two sections of cards as skeleton blocks.

use iced::Element;
use iced::widget::column;

use crate::gui::app::Message;
use crate::gui::style::gap;
use crate::gui::views::kit::{Skeleton, vspace};
use crate::layout;

/// Sections of the skeleton.
const SECTIONS: usize = 2;

/// The skeleton of the body under the page intro.
pub(super) fn view<'a>(sk: Skeleton, grid: layout::Grid) -> Element<'a, Message> {
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let content_w = grid.content as f32;
    let mut c = column![];
    for _ in 0..SECTIONS {
        c = c
            .push(sk.section_header(content_w * 0.25))
            .push(sk.cards(grid, 1, false))
            .push(vspace(gap::XXL));
    }
    c.into()
}
