//! Home: "Quick picks" as a two-column song list, then shelves of large cards laid out with
//! [`crate::layout::grid_in`]. The shelves come from the catalog's home feed when there is one,
//! otherwise from a few calm seed searches (see [`crate::home`]); a shelf that fails is absent.
//! The chips in the top bar filter the shelves by kind.

use iced::widget::{Space, column, row};
use iced::{Alignment, Element, Length};
use nemo_core::catalog::{Shelf, ShelfItem};
use nemo_core::track::{Artwork, Track};

use super::super::app::{App, ArtShape, HomeFilter, HomePhase, ListSource, Message};
use super::super::assets::Icon;
use super::super::style::{col, gap, label};
use super::kit::{
    Card, card_grid, eyebrow, page, quick_pick, section_title, shelf_header, status, text_button,
    vspace,
};
use crate::format;
use crate::layout;
use crate::route::Route;
use crate::state::Event;
use crate::theme::{color, type_scale};

/// Songs in Quick picks (four rows of two).
const QUICK_COUNT: usize = 8;

/// The Home screen. `w` is the width of the centre pane.
pub fn view(app: &App, w: u32) -> Element<'_, Message> {
    let grid = layout::grid_in(w);
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let content_w = grid.content as f32;
    let filter = app.home_filter;
    let mut c = column![].width(Length::Fill);
    let mut any = false;

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

/// Where Quick picks come from: the first shelf with songs, else the recent tracks, else the
/// local library. The list a row plays is that same source.
fn quick_picks_source(app: &App) -> Option<(ListSource, Vec<&Track>)> {
    for (si, shelf) in app.home.shelves.iter().enumerate() {
        let songs: Vec<&Track> = shelf
            .items
            .iter()
            .filter_map(|i| match i {
                ShelfItem::Song(t) => Some(t),
                _ => None,
            })
            .collect();
        if !songs.is_empty() {
            return Some((
                ListSource::Home(si),
                songs.into_iter().take(QUICK_COUNT).collect(),
            ));
        }
    }
    if !app.ui.recent.is_empty() {
        return Some((
            ListSource::Recent,
            app.ui.recent.iter().take(QUICK_COUNT).collect(),
        ));
    }
    if !app.local_tracks.is_empty() {
        return Some((
            ListSource::Library,
            app.local_tracks.iter().take(QUICK_COUNT).collect(),
        ));
    }
    None
}

fn quick_source_kind(app: &App) -> Option<ListSource> {
    quick_picks_source(app).map(|(s, _)| s)
}

fn quick_picks<'a>(
    app: &'a App,
    source: ListSource,
    tracks: &[&'a Track],
    content_w: f32,
) -> Element<'a, Message> {
    let col_w = ((content_w - gap::LG) / 2.0).floor();
    let mut rows = column![].spacing(gap::XS);
    for (r, pair) in tracks.chunks(2).enumerate() {
        let mut line = row![].spacing(gap::LG);
        for (k, t) in pair.iter().enumerate() {
            line = line.push(quick_pick(
                app,
                t,
                source.clone(),
                r * 2 + k,
                col_w,
                r * 2 + k,
            ));
        }
        rows = rows.push(line);
    }
    column![
        eyebrow("Start listening"),
        row![
            section_title("Quick picks"),
            Space::new().width(Length::Fill),
            text_button("Play all", Message::PlayAt { source, index: 0 }),
        ]
        .align_y(Alignment::Center),
        vspace(gap::SM),
        rows,
        vspace(gap::XXL),
    ]
    .into()
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
            },
            ShelfItem::Album(a) => Card {
                artwork: Artwork::Remote(a.thumbnails.clone()),
                title: a.title.clone(),
                subtitle: format::artists(&a.artists),
                shape: ArtShape::Tile,
                on_press: Message::Ui(Event::Go(Route::Album(a.id.clone()))),
            },
            ShelfItem::Artist(a) => Card {
                artwork: Artwork::Remote(a.thumbnails.clone()),
                title: a.name.clone(),
                subtitle: "Artist".to_owned(),
                shape: ArtShape::Circle,
                on_press: Message::Ui(Event::Go(Route::Artist(a.id.clone()))),
            },
            ShelfItem::Playlist(p) => Card {
                artwork: Artwork::Remote(p.thumbnails.clone()),
                title: p.title.clone(),
                subtitle: p.author.clone().unwrap_or_else(|| "Playlist".to_owned()),
                shape: ArtShape::Tile,
                on_press: Message::Ui(Event::Go(Route::Playlist(p.id.clone()))),
            },
        });
    }
    cards
}
