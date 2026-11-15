//! A mood or genre page opened from Explore: the playlist shelves YouTube Music gives for it.

use iced::Element;
use iced::widget::column;
use nemo_core::catalog::BrowseId;

use super::feed::feed_cards;
use crate::explore_feed::MoodPage;
use crate::gui::app::{App, ExploreMsg, GATE_MESSAGE, ListSource, Message};
use crate::gui::assets::Icon;
use crate::gui::style::gap;
use crate::gui::views::kit::{card_grid, page, page_title, section_title, status, vspace};
use crate::layout;
use crate::route::Route;

/// Rows of cards per shelf.
const SHELF_ROWS: usize = 1;

/// The mood page of the route. `w` is the width of the centre pane.
pub fn view(app: &App, w: u32) -> Element<'_, Message> {
    let grid = layout::grid_in(w);
    let Route::Mood { id, title } = app.ui.route() else {
        return page(grid.content, column![]);
    };
    let mut c = column![page_title(title.clone()), vspace(gap::LG)];
    let body: Element<'_, Message> = match app.moods.get(id) {
        Some(MoodPage::Ready(shelves)) => {
            let mut s = column![];
            for (n, shelf) in shelves.iter().enumerate() {
                let cards = feed_cards(shelf, &ListSource::Mood(id.clone(), n));
                if cards.is_empty() {
                    continue;
                }
                s = s
                    .push(section_title(shelf.title.clone()))
                    .push(vspace(gap::MD))
                    .push(card_grid(app, grid, cards, SHELF_ROWS, n))
                    .push(vspace(gap::XXL));
            }
            s.into()
        }
        Some(MoodPage::Failed(_)) => failed(id),
        None if !app.ui.youtube_enabled() => {
            status(Icon::Explore, "This page is waiting", GATE_MESSAGE, None)
        }
        Some(MoodPage::Loading) | None => status(
            Icon::Explore,
            "Loading",
            "Asking YouTube Music for the playlists of this mood.",
            None,
        ),
    };
    c = c.push(body);
    page(grid.content, c)
}

fn failed(id: &BrowseId) -> Element<'_, Message> {
    status(
        Icon::Explore,
        "This page did not load",
        "YouTube Music did not answer. Try again in a moment.",
        Some(("Retry", Message::Explore(ExploreMsg::RetryMood(id.clone())))),
    )
}
