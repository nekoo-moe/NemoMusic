//! Explore: mood and genre tiles (each runs a catalog search for its query), then shelves of
//! albums and playlists. Hook point: the parent can send real explore shelves with
//! `Message::ExploreLoaded`; until then the page shows what Home has.

use iced::widget::{Space, button, column, container, row};
use iced::{Alignment, Border, Element, Length, Padding};
use nemo_core::catalog::{Shelf, ShelfItem};
use nemo_core::track::Artwork;

use super::super::app::{App, ArtShape, DetailKey, DetailKind, Message};
use super::super::assets::Icon;
use super::super::fx::fx;
use super::super::style::{col, gap, icon, label};
use super::kit::{Card, card_grid, page, page_title, section_title, status, vspace};
use crate::explore::{MOODS, Mood};
use crate::format;
use crate::layout;
use crate::palette::{Oklch, oklch_to_rgb};
use crate::route::Route;
use crate::state::Event;
use crate::theme::{Rgba, color, shape, type_scale};

/// Tile colour of a mood on the active surfaces: deep on dark, pale on light.
pub(super) fn tile_color(mood: &Mood) -> (Rgba, Rgba) {
    let light = color::surface().relative_luminance() > 0.5;
    let (l, c) = if light { (0.86, 0.07) } else { (0.36, 0.09) };
    let fill = oklch_to_rgb(Oklch { l, c, h: mood.hue });
    let ink = if light {
        Rgba::from_rgb_hex(0x1A_1C20)
    } else {
        Rgba::from_rgb_hex(0xF2_F3F5)
    };
    (fill, ink)
}

/// The Explore screen. `w` is the width of the centre pane.
pub fn view(app: &App, w: u32) -> Element<'_, Message> {
    let grid = layout::grid_in(w);
    let mut c = column![
        page_title("Explore"),
        vspace(gap::XS),
        label(
            "Pick a mood or a genre. Each one searches YouTube Music.",
            type_scale::BODY_MD,
            false,
            col(color::on_surface_variant())
        ),
        vspace(gap::LG),
        section_title("Moods and genres"),
        vspace(gap::MD),
    ]
    .width(Length::Fill);

    // Mood tiles.
    let per_row = grid.columns as usize;
    #[allow(clippy::cast_precision_loss)]
    let (tile_w, gap_px) = (grid.card as f32, grid.gap as f32);
    let mut rows = column![].spacing(gap_px.min(gap::LG));
    for (chunk_index, chunk) in MOODS.chunks(per_row).enumerate() {
        let mut line = row![].spacing(gap_px);
        for (k, mood) in chunk.iter().enumerate() {
            let index = chunk_index * per_row + k;
            let (fill, ink) = tile_color(mood);
            let body = container(
                row![
                    label(mood.label, type_scale::TITLE_SM, true, col(ink)),
                    Space::new().width(Length::Fill),
                    icon(Icon::ArrowForward, 18.0, col(ink.with_alpha(0.8))),
                ]
                .align_y(Alignment::Center),
            )
            .padding(Padding::from([0.0, gap::LG]))
            .width(Length::Fixed(tile_w))
            .height(Length::Fixed(72.0))
            .align_y(Alignment::Center);
            let b = button(body).padding(0).on_press(Message::PickMood(index));
            let radius = shape::lg();
            line = line.push(
                fx(b.style(move |_, _| button::Style {
                    background: Some(col(fill).into()),
                    text_color: col(ink),
                    border: Border {
                        radius: radius.into(),
                        ..Border::default()
                    },
                    shadow: iced::Shadow::default(),
                    snap: false,
                }))
                .layer(col(ink))
                .radius(radius)
                .lift(3.0),
            );
        }
        rows = rows.push(line);
    }
    c = c.push(rows).push(vspace(gap::XXL));

    // Shelves: from the parent's explore hook, else the album, artist and playlist shelves of Home.
    let hooked = !app.explore_shelves.is_empty();
    let shelves: Vec<&Shelf> = if hooked {
        app.explore_shelves.iter().collect()
    } else {
        app.home.shelves.iter().collect()
    };
    let mut any = false;
    for (si, shelf) in shelves.iter().enumerate() {
        let cards = collection_cards(shelf);
        if cards.is_empty() {
            continue;
        }
        any = true;
        c = c
            .push(section_title(shelf.title.clone()))
            .push(vspace(gap::MD))
            .push(card_grid(app, grid, cards, 1, 4 + si))
            .push(vspace(gap::XXL));
    }
    if !any {
        c = c.push(status(
            Icon::Explore,
            "New releases and charts will show up here",
            "They come from the catalog once it offers them. Moods and genres above already work.",
            None,
        ));
    }
    page(grid.content, c)
}

/// Cards for the albums, playlists and artists of a shelf (songs are skipped: a song list needs
/// a source to play from).
fn collection_cards(shelf: &Shelf) -> Vec<Card> {
    let mut out = Vec::new();
    for item in &shelf.items {
        match item {
            ShelfItem::Song(_) => {}
            ShelfItem::Album(a) => out.push(Card {
                artwork: Artwork::Remote(a.thumbnails.clone()),
                title: a.title.clone(),
                subtitle: format::artists(&a.artists),
                shape: ArtShape::Tile,
                on_press: Message::Ui(Event::Go(Route::Album(a.id.clone()))),
                play: Some(Message::PlayCollection(DetailKey {
                    kind: DetailKind::Album,
                    id: a.id.clone(),
                })),
            }),
            ShelfItem::Playlist(p) => out.push(Card {
                artwork: Artwork::Remote(p.thumbnails.clone()),
                title: p.title.clone(),
                subtitle: p.author.clone().unwrap_or_else(|| "Playlist".to_owned()),
                shape: ArtShape::Tile,
                on_press: Message::Ui(Event::Go(Route::Playlist(p.id.clone()))),
                play: Some(Message::PlayCollection(DetailKey {
                    kind: DetailKind::Playlist,
                    id: p.id.clone(),
                })),
            }),
            ShelfItem::Artist(a) => out.push(Card {
                artwork: Artwork::Remote(a.thumbnails.clone()),
                title: a.name.clone(),
                subtitle: "Artist".to_owned(),
                shape: ArtShape::Circle,
                on_press: Message::Ui(Event::Go(Route::Artist(a.id.clone()))),
                play: None,
            }),
        }
    }
    out
}
