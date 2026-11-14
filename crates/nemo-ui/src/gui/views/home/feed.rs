//! Home: the Videos and Podcasts pages of the content chips.

use iced::Element;
use iced::widget::column;
use nemo_core::track::Artwork;

use crate::format;
use crate::gui::app::{App, ArtShape, ListSource, Message};
use crate::gui::assets::Icon;
use crate::gui::style::{col, gap, label};
use crate::gui::views::kit::{Card, card_grid, page, section_title, status, vspace};
use crate::layout;
use crate::theme::{color, type_scale};

/// The Videos and Podcasts chips: real shelves when the home feed gave them.
pub(super) fn feed_kind_page(app: &App, grid: layout::Grid) -> Element<'_, Message> {
    use crate::explore::ContentFilter;
    if app.content_filter == ContentFilter::Videos && !app.daily.videos.is_empty() {
        let cards = app
            .daily
            .videos
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let play = Message::PlayAt {
                    source: ListSource::Videos,
                    index: i,
                };
                Card {
                    artwork: t.artwork.clone(),
                    title: t.title.clone(),
                    subtitle: format::artists(&t.artists),
                    shape: ArtShape::Tile,
                    on_press: play.clone(),
                    play: Some(play),
                }
            })
            .collect();
        return page(
            grid.content,
            column![
                section_title("Music videos"),
                label(
                    "Plays the audio of the video.",
                    type_scale::LABEL_MD,
                    false,
                    col(color::on_surface_variant())
                ),
                vspace(gap::MD),
                card_grid(app, grid, cards, 4, 0),
            ],
        );
    }
    if app.content_filter == ContentFilter::Podcasts && !app.daily.podcasts.is_empty() {
        let cards = app
            .daily
            .podcasts
            .iter()
            .map(|p| Card {
                artwork: Artwork::Remote(p.thumbnails.clone()),
                title: p.title.clone(),
                subtitle: p.author.clone().unwrap_or_else(|| "Podcast".to_owned()),
                shape: ArtShape::Tile,
                on_press: Message::Noop,
                play: None,
            })
            .collect();
        return page(
            grid.content,
            column![
                section_title("Podcasts"),
                label(
                    "Show pages are not built yet.",
                    type_scale::LABEL_MD,
                    false,
                    col(color::on_surface_variant())
                ),
                vspace(gap::MD),
                card_grid(app, grid, cards, 4, 0),
            ],
        );
    }
    let (title, detail, glyph) = if app.content_filter == ContentFilter::Videos {
        (
            "No music videos yet",
            "The home feed has none right now, or you are offline.",
            Icon::Video,
        )
    } else {
        (
            "No podcasts yet",
            "The home feed has none right now, or you are offline.",
            Icon::Podcasts,
        )
    };
    page(
        grid.content,
        status(
            glyph,
            title,
            detail,
            Some(("Back to All", Message::ContentFilter(ContentFilter::All))),
        ),
    )
}
