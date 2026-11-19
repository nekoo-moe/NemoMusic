//! Explore (real-explore spec): YouTube Music's own Explore page. New albums and singles, the
//! moods and genres (each opens its mood page), the charts, and the other shelves of the page
//! such as new music videos. When Explore never loaded, one short line says so and the seed
//! shelves of Home stand in, with mood tiles that run a search; when a reload fails, the last
//! answer stays on screen under one short line.

use iced::widget::{Space, button, column, container, row};
use iced::{Alignment, Border, Element, Length, Padding};
use nemo_core::catalog::{Explore, Shelf};

use super::super::app::{App, ExploreMsg, GATE_MESSAGE, ListSource, Message};
use super::super::assets::Icon;
use super::super::fx::fx;
use super::super::style::{col, fit, gap, icon, label};
use super::kit::{
    blank, card_grid, page, page_title, section_title, status, swap, text_button, vspace,
};
use crate::explore::{MOODS, Mood};
use crate::layout;
use crate::palette::{Oklch, oklch_to_rgb};
use crate::route::Route;
use crate::skeleton::Region;
use crate::state::Event;
use crate::theme::{Rgba, color, shape, type_scale};

mod feed;
pub mod mood;
mod skeleton;

use feed::{album_cards, collection_cards, feed_cards};

/// Rows of new releases shown (one, as YouTube Music shows them, so the moods stay in view).
const RELEASE_ROWS: usize = 1;
/// Height of a mood tile.
const TILE_H: f32 = 72.0;
/// Size of the arrow on a mood tile.
const ARROW: f32 = 18.0;

/// Tile colour of a mood on the active surfaces: deep on dark, pale on light.
pub(super) fn tile_color(mood: &Mood) -> (Rgba, Rgba) {
    hue_color(mood.hue)
}

fn hue_color(hue: f64) -> (Rgba, Rgba) {
    let light = color::surface().relative_luminance() > 0.5;
    let (l, c) = if light { (0.86, 0.07) } else { (0.36, 0.09) };
    let fill = oklch_to_rgb(Oklch { l, c, h: hue });
    let ink = if light {
        Rgba::from_rgb_hex(0x1A_1C20)
    } else {
        Rgba::from_rgb_hex(0xF2_F3F5)
    };
    (fill, ink)
}

/// The hue of a mood chip from the catalog: the hue of the built-in mood with the same label,
/// else one spread around the circle by its position.
fn chip_hue(title: &str, index: usize) -> f64 {
    MOODS
        .iter()
        .find(|m| m.label.eq_ignore_ascii_case(title))
        .map_or_else(
            || {
                // Positions are small.
                #[allow(clippy::cast_precision_loss)]
                let i = index as f64;
                (i * 137.5) % 360.0
            },
            |m| m.hue,
        )
}

/// One mood tile: label, hue and what a press does.
struct Tile {
    label: String,
    hue: f64,
    on_press: Message,
}

/// The Explore screen. `w` is the width of the centre pane.
pub fn view(app: &App, w: u32) -> Element<'_, Message> {
    let grid = layout::grid_in(w);
    let mut c = column![
        page_title("Explore"),
        vspace(gap::XS),
        label(
            "New releases, charts, moods and genres from YouTube Music.",
            type_scale::BODY_MD,
            false,
            col(color::on_surface_variant())
        ),
    ]
    .width(Length::Fill);
    if let Some(line) = app.explore.notice() {
        c = c.push(vspace(gap::SM)).push(notice(line));
    }
    c = c.push(vspace(gap::LG));
    let body = swap(
        app,
        Region::Explore,
        || match app.explore.data() {
            Some(e) => real(app, grid, e),
            None if app.explore.shows_fallback() => fallback(app, grid),
            None if !app.ui.youtube_enabled() => {
                status(Icon::Explore, "Explore is waiting", GATE_MESSAGE, None)
            }
            // Loading: the skeleton comes after its 150 ms wait.
            None => blank(),
        },
        |sk| skeleton::view(sk, grid),
    );
    page(grid.content, c.push(body))
}

/// The one short line of a failed load, with Retry.
fn notice(line: &'static str) -> Element<'static, Message> {
    row![
        label(
            line,
            type_scale::BODY_MD,
            false,
            col(color::on_surface_variant())
        ),
        Space::new().width(gap::SM),
        text_button("Retry", Message::Explore(ExploreMsg::Retry)),
    ]
    .align_y(Alignment::Center)
    .into()
}

/// The catalog's Explore page.
fn real<'a>(app: &'a App, grid: layout::Grid, e: &'a Explore) -> Element<'a, Message> {
    let mut c = column![].width(Length::Fill);
    let releases = album_cards(&e.new_releases);
    if !releases.is_empty() {
        c = c
            .push(section_title("New albums and singles"))
            .push(vspace(gap::MD))
            .push(card_grid(app, grid, releases, RELEASE_ROWS, 0))
            .push(vspace(gap::XXL));
    }
    let tiles_in: Vec<Tile> = if e.moods.is_empty() {
        search_tiles()
    } else {
        e.moods
            .iter()
            .enumerate()
            .map(|(i, m)| Tile {
                label: m.title.clone(),
                hue: chip_hue(&m.title, i),
                on_press: Message::Ui(Event::Go(Route::Mood {
                    id: m.id.clone(),
                    title: m.title.clone(),
                })),
            })
            .collect()
    };
    c = c
        .push(section_title("Moods and genres"))
        .push(vspace(gap::MD))
        .push(tiles(grid, tiles_in))
        .push(vspace(gap::XXL));
    for (n, shelf) in app.explore.shelves().enumerate() {
        let cards = feed_cards(shelf, &ListSource::Explore(n));
        if cards.is_empty() {
            continue;
        }
        c = c
            .push(section_title(shelf.title.clone()))
            .push(vspace(gap::MD))
            .push(card_grid(app, grid, cards, 1, 4 + n))
            .push(vspace(gap::XXL));
    }
    c.into()
}

/// Explore never loaded: mood tiles that search, then the parent's shelves or Home's seed
/// shelves.
fn fallback(app: &App, grid: layout::Grid) -> Element<'_, Message> {
    let mut c = column![
        section_title("Moods and genres"),
        vspace(gap::MD),
        tiles(grid, search_tiles()),
        vspace(gap::XXL),
    ]
    .width(Length::Fill);
    let shelves: Vec<&Shelf> = if app.explore_shelves.is_empty() {
        app.home.shelves.iter().collect()
    } else {
        app.explore_shelves.iter().collect()
    };
    for (si, shelf) in shelves.iter().enumerate() {
        let cards = collection_cards(shelf);
        if cards.is_empty() {
            continue;
        }
        c = c
            .push(section_title(shelf.title.clone()))
            .push(vspace(gap::MD))
            .push(card_grid(app, grid, cards, 1, 4 + si))
            .push(vspace(gap::XXL));
    }
    c.into()
}

/// The built-in moods; each runs a catalog search.
fn search_tiles() -> Vec<Tile> {
    MOODS
        .iter()
        .enumerate()
        .map(|(i, m)| Tile {
            label: m.label.to_owned(),
            hue: m.hue,
            on_press: Message::PickMood(i),
        })
        .collect()
}

/// Mood tiles in rows of the grid's columns.
fn tiles<'a>(grid: layout::Grid, items: Vec<Tile>) -> Element<'a, Message> {
    let per_row = (grid.columns as usize).max(1);
    #[allow(clippy::cast_precision_loss)]
    let (tile_w, gap_px) = (grid.card as f32, grid.gap as f32);
    let mut rows = column![].spacing(gap_px.min(gap::LG));
    let mut it = items.into_iter().peekable();
    while it.peek().is_some() {
        let mut line = row![].spacing(gap_px);
        for tile in it.by_ref().take(per_row) {
            line = line.push(tile_view(tile, tile_w));
        }
        rows = rows.push(line);
    }
    rows.into()
}

fn tile_view<'a>(tile: Tile, tile_w: f32) -> Element<'a, Message> {
    let (fill, ink) = hue_color(tile.hue);
    let text_w = tile_w - 2.0 * gap::LG - ARROW - gap::SM;
    let body = container(
        row![
            label(
                fit(&tile.label, text_w, type_scale::TITLE_SM.size),
                type_scale::TITLE_SM,
                true,
                col(ink)
            ),
            Space::new().width(Length::Fill),
            icon(Icon::ArrowForward, ARROW, col(ink.with_alpha(0.8))),
        ]
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([0.0, gap::LG]))
    .width(Length::Fixed(tile_w))
    .height(Length::Fixed(TILE_H))
    .align_y(Alignment::Center);
    let b = button(body).padding(0).on_press(tile.on_press);
    let radius = shape::lg();
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
    .lift(3.0)
    .into()
}
