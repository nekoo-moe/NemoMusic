//! Library lists: collections as cards or rows, and songs as rows or cards.

use iced::widget::{Space, button, column, container, row};
use iced::{Alignment, Color, Element, Length, Padding};
use nemo_core::track::Track;

use super::collection::Coll;
use super::note;
use crate::explore::LibLayout;
use crate::format;
use crate::gui::app::{App, ArtShape, ListSource, Message};
use crate::gui::style::{animated, col, fit, gap, label};
use crate::gui::views::kit::{Card, RowCtx, art, card_grid, track_list};
use crate::layout;
use crate::state::Event;
use crate::theme::{color, shape, type_scale};

/// A whole tab of collections.
pub(super) fn collections<'a>(
    app: &'a App,
    items: Vec<Coll>,
    grid: layout::Grid,
    content_w: f32,
    empty: &'a str,
) -> Element<'a, Message> {
    if items.is_empty() {
        return note(app, empty, None);
    }
    collection_view(app, items, grid, content_w, 12, 0, false)
}

/// Collections as a card grid or as rows, by the layout toggle.
pub(super) fn collection_view(
    app: &App,
    items: Vec<Coll>,
    grid: layout::Grid,
    content_w: f32,
    max_rows: usize,
    base: usize,
    force_grid: bool,
) -> Element<'_, Message> {
    if force_grid || app.lib_layout == LibLayout::Grid {
        let cards: Vec<Card> = items
            .into_iter()
            .map(|c| Card {
                artwork: c.artwork,
                title: c.title,
                subtitle: c.subtitle,
                shape: if c.circle {
                    ArtShape::Circle
                } else {
                    ArtShape::Tile
                },
                on_press: Message::Ui(Event::Go(c.route)),
                play: c.play,
            })
            .collect();
        return card_grid(app, grid, cards, max_rows, base);
    }
    let text_w = (content_w - 48.0 - gap::MD - 2.0 * gap::SM - 24.0).max(120.0);
    let mut list = column![].spacing(gap::XS);
    for c in items.into_iter().take(max_rows.saturating_mul(6).max(6)) {
        let shape_ = if c.circle {
            ArtShape::Circle
        } else {
            ArtShape::Thumb
        };
        let line = row![
            art(app, &c.artwork, 48, shape_),
            Space::new().width(gap::MD),
            column![
                label(
                    fit(&c.title, text_w, type_scale::BODY_MD.size),
                    type_scale::BODY_MD,
                    true,
                    col(color::on_surface())
                ),
                label(
                    fit(&c.subtitle, text_w, type_scale::BODY_SM.size),
                    type_scale::BODY_SM,
                    false,
                    col(color::on_surface_variant())
                ),
            ]
            .width(Length::Fixed(text_w)),
        ]
        .align_y(Alignment::Center);
        let b = button(
            container(line)
                .width(Length::Fill)
                .height(Length::Fixed(60.0))
                .padding(Padding::from([0.0, gap::SM]))
                .align_y(Alignment::Center),
        )
        .padding(0)
        .width(Length::Fill)
        .on_press(Message::Ui(Event::Go(c.route)));
        list = list.push(animated(b, Color::TRANSPARENT, shape::lg()));
    }
    list.into()
}

/// Songs as rows or, in grid mode, as cards that play.
#[allow(clippy::needless_pass_by_value)] // callers build the list per frame and hand it over
pub(super) fn song_block(
    app: &App,
    tracks: Vec<Track>,
    source: ListSource,
    content_w: f32,
    grid: layout::Grid,
) -> Element<'_, Message> {
    if app.lib_layout == LibLayout::Grid {
        let cards: Vec<Card> = tracks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let play = Message::PlayAt {
                    source: source.clone(),
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
        return card_grid(app, grid, cards, 8, 0);
    }
    let ctx = RowCtx {
        width: content_w,
        source,
        numbered: false,
        show_album: true,
    };
    track_list(app, &tracks, app.rows_limit, &ctx)
}
