//! The Library tabs while the account library loads: skeleton rows or cards, in the layout of
//! the tab. The header and the tabs above stay real.

use iced::Element;
use iced::widget::column;

use crate::explore::LibTab;
use crate::gui::app::Message;
use crate::gui::style::gap;
use crate::gui::views::kit::{Skeleton, vspace};
use crate::layout;

/// Rows of a list tab.
const ROWS: usize = 7;
/// Card rows of a grid tab.
const CARD_ROWS: usize = 2;

/// The skeleton of tab `tab`.
pub(super) fn view<'a>(
    sk: Skeleton,
    tab: LibTab,
    grid: layout::Grid,
    content_w: f32,
) -> Element<'a, Message> {
    let mut c = column![sk.section_header(content_w * 0.2)];
    match tab {
        LibTab::Overview | LibTab::Songs | LibTab::Local => {
            for i in 0..ROWS {
                c = c.push(sk.row(content_w, i));
            }
        }
        LibTab::Playlists | LibTab::Albums => c = c.push(sk.cards(grid, CARD_ROWS, false)),
        LibTab::Artists => c = c.push(sk.cards(grid, CARD_ROWS, true)),
    }
    c.push(vspace(gap::XXL)).into()
}
