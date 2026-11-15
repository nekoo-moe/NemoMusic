//! One section of the search results: songs and videos as rows, albums, artists and playlists
//! as cards.

use iced::Element;
use iced::widget::column;
use nemo_core::catalog::{SearchKind, SearchResults};
use nemo_core::track::{Artwork, Track};

use super::super::super::app::{App, ArtShape, DetailKey, DetailKind, ListSource, Message};
use super::super::kit::{Card, RowCtx, card_grid, section_title, track_list, vspace};
use crate::format;
use crate::gui::style::gap;
use crate::layout::Grid;
use crate::route::Route;
use crate::search::SONGS_SHOWN;
use crate::state::Event;

/// Where the sections are drawn.
pub(super) struct Frame {
    /// The card grid of the page.
    pub(super) grid: Grid,
    /// Content width in px.
    pub(super) width: f32,
    /// One filtered kind: every result, with "Show more" on long lists, instead of a preview.
    pub(super) full: bool,
}

/// The section of `kind`, titled, or `None` when it shows nothing. `stagger` is the entrance
/// index of its first card.
pub(super) fn section<'a>(
    app: &'a App,
    r: &SearchResults,
    kind: SearchKind,
    frame: &Frame,
    stagger: usize,
) -> Option<Element<'a, Message>> {
    let (title, body) = match kind {
        SearchKind::Songs => ("Songs", rows(app, &r.songs, ListSource::Search, frame)),
        SearchKind::Videos => {
            let tracks: Vec<Track> = r.videos.iter().map(|v| v.track.clone()).collect();
            (
                "Videos",
                rows(app, &tracks, ListSource::SearchVideos, frame),
            )
        }
        SearchKind::Albums => ("Albums", cards(app, album_cards(r), frame, stagger)),
        SearchKind::Artists => ("Artists", cards(app, artist_cards(r), frame, stagger)),
        SearchKind::Playlists => ("Playlists", cards(app, playlist_cards(r), frame, stagger)),
        // All-of-YouTube channels have no section on this screen yet.
        SearchKind::Channels => return None,
    };
    let space = if matches!(kind, SearchKind::Songs | SearchKind::Videos) {
        gap::SM
    } else {
        gap::MD
    };
    Some(
        column![section_title(title), vspace(space), body]
            .push(vspace(gap::XXL))
            .into(),
    )
}

fn rows<'a>(
    app: &'a App,
    tracks: &[Track],
    source: ListSource,
    frame: &Frame,
) -> Element<'a, Message> {
    let limit = if frame.full {
        app.rows_limit
    } else {
        SONGS_SHOWN
    };
    let shown = tracks.len().min(limit.max(1));
    let ctx = RowCtx {
        width: frame.width,
        source,
        numbered: false,
        show_album: true,
    };
    track_list(app, tracks, shown, &ctx)
}

fn cards<'a>(app: &App, cards: Vec<Card>, frame: &Frame, stagger: usize) -> Element<'a, Message> {
    // A preview is one row; a filtered kind shows every card (at most one per row needed).
    let rows = if frame.full { cards.len().max(1) } else { 1 };
    card_grid(app, frame.grid, cards, rows, stagger)
}

fn album_cards(r: &SearchResults) -> Vec<Card> {
    r.albums
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
        .collect()
}

fn artist_cards(r: &SearchResults) -> Vec<Card> {
    r.artists
        .iter()
        .map(|a| Card {
            artwork: Artwork::Remote(a.thumbnails.clone()),
            title: a.name.clone(),
            subtitle: "Artist".to_owned(),
            shape: ArtShape::Circle,
            on_press: Message::Ui(Event::Go(Route::Artist(a.id.clone()))),
            play: None,
        })
        .collect()
}

fn playlist_cards(r: &SearchResults) -> Vec<Card> {
    r.playlists
        .iter()
        .map(|p| Card {
            artwork: Artwork::Remote(p.thumbnails.clone()),
            title: p.title.clone(),
            subtitle: p.author.clone().unwrap_or_else(|| "Playlist".to_owned()),
            shape: ArtShape::Tile,
            on_press: Message::Ui(Event::Go(Route::Playlist(p.id.clone()))),
            play: Some(Message::PlayCollection(DetailKey {
                kind: DetailKind::Playlist,
                id: p.id.clone(),
            })),
        })
        .collect()
}
