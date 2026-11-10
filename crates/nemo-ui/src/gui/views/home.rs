//! Home: "Jump back in" (recently played), local albums and shelves, laid out with
//! [`crate::layout::grid`]. The shelves come from the catalog's home feed when there is one,
//! otherwise from a few calm seed searches (see [`crate::home`]); a shelf that fails is absent.

use iced::widget::column;
use iced::{Element, Length};
use nemo_core::catalog::{Shelf, ShelfItem};

use super::super::app::{App, ArtShape, HomePhase, ListSource, Message};
use super::super::assets::Icon;
use super::super::style::{col, gap, label};
use super::kit::{Card, card_grid, page, page_title, shelf_header, status, vspace};
use crate::format;
use crate::layout;
use crate::route::Route;
use crate::state::Event;
use crate::theme::{color, type_scale};

/// The Home screen.
pub fn view(app: &App, w: u32) -> Element<'_, Message> {
    let grid = layout::grid(w, app.ui.queue_open);
    let mut c = column![page_title("Welcome back"), vspace(gap::XL)].width(Length::Fill);
    let mut any = false;

    if !app.ui.recent.is_empty() {
        any = true;
        let cards = app
            .ui
            .recent
            .iter()
            .enumerate()
            .map(|(i, t)| Card {
                artwork: t.artwork.clone(),
                title: t.title.clone(),
                subtitle: format::artists(&t.artists),
                shape: ArtShape::Card,
                on_press: Message::PlayAt {
                    source: ListSource::Recent,
                    index: i,
                },
            })
            .collect();
        c = c
            .push(shelf_header("Jump back in"))
            .push(card_grid(app, grid, cards, 1))
            .push(vspace(gap::XXL));
    }

    if !app.local_albums.is_empty() {
        any = true;
        let cards = app
            .local_albums
            .iter()
            .map(|a| Card {
                artwork: nemo_core::track::Artwork::Remote(a.thumbnails.clone()),
                title: a.title.clone(),
                subtitle: format::artists(&a.artists),
                shape: ArtShape::Card,
                on_press: Message::Ui(Event::Go(Route::Album(a.id.clone()))),
            })
            .collect();
        c = c
            .push(shelf_header("Your albums"))
            .push(card_grid(app, grid, cards, 1))
            .push(vspace(gap::XXL));
    }

    for (si, shelf) in app.home.shelves.iter().enumerate() {
        any = true;
        c = c
            .push(shelf_header(shelf.title.clone()))
            .push(card_grid(app, grid, shelf_cards(si, shelf), 1))
            .push(vspace(gap::XXL));
    }

    if !app.ui.youtube_enabled() {
        c = c.push(status(
            Icon::Info,
            "YouTube Music is waiting",
            "Read the short notice to browse and search YouTube Music. Your own files work without it.",
            Some(("Review notice", Message::Ui(Event::TosReview))),
        ));
    } else if app.home.phase != HomePhase::Done {
        c = c.push(label(
            "Finding something calm\u{2026}",
            type_scale::BODY_MD,
            false,
            col(color::ON_SURFACE_VARIANT),
        ));
    } else if !any {
        c = c.push(status(
            Icon::Note,
            "Nothing here yet",
            "Search for music, or add a music folder in Settings.",
            Some((
                "Search",
                Message::Ui(Event::Go(Route::Search(String::new()))),
            )),
        ));
    }
    page(grid.content, c)
}

fn shelf_cards(shelf_index: usize, shelf: &Shelf) -> Vec<Card> {
    let mut songs = 0_usize;
    shelf
        .items
        .iter()
        .map(|item| match item {
            ShelfItem::Song(t) => {
                let index = songs;
                songs += 1;
                Card {
                    artwork: t.artwork.clone(),
                    title: t.title.clone(),
                    subtitle: format::artists(&t.artists),
                    shape: ArtShape::Card,
                    on_press: Message::PlayAt {
                        source: ListSource::Home(shelf_index),
                        index,
                    },
                }
            }
            ShelfItem::Album(a) => Card {
                artwork: nemo_core::track::Artwork::Remote(a.thumbnails.clone()),
                title: a.title.clone(),
                subtitle: format::artists(&a.artists),
                shape: ArtShape::Card,
                on_press: Message::Ui(Event::Go(Route::Album(a.id.clone()))),
            },
            ShelfItem::Artist(a) => Card {
                artwork: nemo_core::track::Artwork::Remote(a.thumbnails.clone()),
                title: a.name.clone(),
                subtitle: "Artist".to_owned(),
                shape: ArtShape::Circle,
                on_press: Message::Ui(Event::Go(Route::Artist(a.id.clone()))),
            },
            ShelfItem::Playlist(p) => Card {
                artwork: nemo_core::track::Artwork::Remote(p.thumbnails.clone()),
                title: p.title.clone(),
                subtitle: p.author.clone().unwrap_or_else(|| "Playlist".to_owned()),
                shape: ArtShape::Card,
                on_press: Message::Ui(Event::Go(Route::Playlist(p.id.clone()))),
            },
        })
        .collect()
}
