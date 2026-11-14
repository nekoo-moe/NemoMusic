//! Home: "Quick picks" as a two-column song list, then shelves of large cards laid out with
//! [`crate::layout::grid_in`]. The shelves come from the catalog's home feed when there is one,
//! otherwise from a few calm seed searches (see [`crate::home`]); a shelf that fails is absent.
//! The chips in the top bar filter the shelves by kind.

use iced::widget::{column, row};
use iced::{Element, Length};
use nemo_core::catalog::{Shelf, ShelfItem};
use nemo_core::track::Artwork;

use super::super::app::{
    App, ArtShape, DetailKey, DetailKind, HomeFilter, HomePhase, ListSource, Message,
};
use super::super::assets::Icon;
use super::super::style::{col, gap, label};
use super::kit::{Card, card_grid, page, shelf_header, status, vspace};
use crate::explore::ContentFilter;
use crate::format;
use crate::layout;
use crate::route::Route;
use crate::state::Event;
use crate::theme::{color, type_scale};

mod daily;
mod feed;
mod hero;
mod quick;

use daily::daily_section;
use feed::feed_kind_page;
use hero::greeting;
use quick::{quick_access, quick_access_grid, quick_picks, quick_picks_source, quick_source_kind};

/// The Home screen. `w` is the width of the centre pane.
pub fn view(app: &App, w: u32) -> Element<'_, Message> {
    let grid = layout::grid_in(w);
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let content_w = grid.content as f32;
    // The kind chips only apply under Music; Videos and Podcasts are hook points with no data yet.
    let filter = if app.content_filter == ContentFilter::Music {
        app.home_filter
    } else {
        HomeFilter::All
    };
    let mut c = column![].width(Length::Fill);
    let mut any = false;
    if matches!(
        app.content_filter,
        ContentFilter::Videos | ContentFilter::Podcasts
    ) {
        return feed_kind_page(app, grid);
    }
    if app.content_filter == ContentFilter::Music {
        c = c.push(kind_chips(app)).push(vspace(gap::MD));
    }
    if filter == HomeFilter::All {
        c = c.push(greeting(app, content_w));
    }
    if filter == HomeFilter::All {
        let qa = quick_access(app);
        if !qa.is_empty() {
            any = true;
            c = c
                .push(quick_access_grid(app, qa, content_w))
                .push(vspace(gap::XL));
        }
        for (i, sec) in app.daily_sections.iter().enumerate() {
            any = true;
            c = c.push(daily_section(app, i, sec, grid, content_w));
        }
    }

    let quick = quick_picks_source(app);
    let quick_shelf = match &quick {
        Some((ListSource::Home(si), _)) => Some(*si),
        _ => None,
    };
    if let Some((source, tracks)) = quick
        && matches!(filter, HomeFilter::All | HomeFilter::Songs)
    {
        any = true;
        c = c.push(quick_picks(app, source, &tracks, content_w));
    }

    if matches!(filter, HomeFilter::All | HomeFilter::Songs)
        && !app.ui.recent.is_empty()
        && !matches!(quick_source_kind(app), Some(ListSource::Recent))
    {
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
                shape: ArtShape::Tile,
                on_press: Message::PlayAt {
                    source: ListSource::Recent,
                    index: i,
                },
                play: Some(Message::PlayAt {
                    source: ListSource::Recent,
                    index: i,
                }),
            })
            .collect();
        c = c
            .push(shelf_header("Jump back in"))
            .push(card_grid(app, grid, cards, 1, 1))
            .push(vspace(gap::XXL));
    }

    if matches!(filter, HomeFilter::All | HomeFilter::Albums) && !app.local_albums.is_empty() {
        any = true;
        let cards = app
            .local_albums
            .iter()
            .map(|a| Card {
                artwork: Artwork::Remote(a.thumbnails.clone()),
                title: a.title.clone(),
                subtitle: format::artists(&a.artists),
                shape: ArtShape::Tile,
                on_press: Message::Ui(Event::Go(Route::Album(a.id.clone()))),
                play: Some(Message::PlayCollection(DetailKey {
                    kind: DetailKind::Album,
                    id: a.id.clone(),
                })),
            })
            .collect();
        c = c
            .push(shelf_header("Your albums"))
            .push(card_grid(app, grid, cards, 1, 2))
            .push(vspace(gap::XXL));
    }

    for (si, shelf) in app.home.shelves.iter().enumerate() {
        if quick_shelf == Some(si) {
            continue;
        }
        let cards = shelf_cards(si, shelf, filter);
        if cards.is_empty() {
            continue;
        }
        any = true;
        c = c
            .push(shelf_header(shelf.title.clone()))
            .push(card_grid(app, grid, cards, 1, 2 + si))
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
            col(color::on_surface_variant()),
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

fn shelf_cards(shelf_index: usize, shelf: &Shelf, filter: HomeFilter) -> Vec<Card> {
    let mut songs = 0_usize;
    let mut cards = Vec::new();
    for item in &shelf.items {
        // Song positions count every song of the shelf, filtered or not: that is the list a
        // press plays.
        let song_index = songs;
        if matches!(item, ShelfItem::Song(_)) {
            songs += 1;
        }
        if !filter.keeps(item) {
            continue;
        }
        cards.push(match item {
            ShelfItem::Song(t) => Card {
                artwork: t.artwork.clone(),
                title: t.title.clone(),
                subtitle: format::artists(&t.artists),
                shape: ArtShape::Tile,
                on_press: Message::PlayAt {
                    source: ListSource::Home(shelf_index),
                    index: song_index,
                },
                play: Some(Message::PlayAt {
                    source: ListSource::Home(shelf_index),
                    index: song_index,
                }),
            },
            ShelfItem::Album(a) => Card {
                artwork: Artwork::Remote(a.thumbnails.clone()),
                title: a.title.clone(),
                subtitle: format::artists(&a.artists),
                shape: ArtShape::Tile,
                on_press: Message::Ui(Event::Go(Route::Album(a.id.clone()))),
                play: Some(Message::PlayCollection(DetailKey {
                    kind: DetailKind::Album,
                    id: a.id.clone(),
                })),
            },
            ShelfItem::Artist(a) => Card {
                artwork: Artwork::Remote(a.thumbnails.clone()),
                title: a.name.clone(),
                subtitle: "Artist".to_owned(),
                shape: ArtShape::Circle,
                on_press: Message::Ui(Event::Go(Route::Artist(a.id.clone()))),
                play: None,
            },
            ShelfItem::Playlist(p) => Card {
                artwork: Artwork::Remote(p.thumbnails.clone()),
                title: p.title.clone(),
                subtitle: p.author.clone().unwrap_or_else(|| "Playlist".to_owned()),
                shape: ArtShape::Tile,
                on_press: Message::Ui(Event::Go(Route::Playlist(p.id.clone()))),
                play: Some(Message::PlayCollection(DetailKey {
                    kind: DetailKind::Playlist,
                    id: p.id.clone(),
                })),
            },
        });
    }
    cards
}

/// Kind chips under Music: All kinds, Songs, Albums, Artists, Playlists.
fn kind_chips(app: &App) -> Element<'_, Message> {
    let accent = app.accent();
    let mut r = row![].spacing(gap::SM);
    for f in HomeFilter::ALL {
        r = r.push(super::kit::chip(
            f.label(),
            app.home_filter == f,
            accent,
            Message::HomeFilter(f),
        ));
    }
    r.into()
}
