//! Library data: collections of the account and of local files, sorted and de-duplicated.

use nemo_core::catalog::{AlbumSummary, ArtistSummary, PlaylistSummary, ShelfItem};
use nemo_core::track::{Artwork, Track};

use crate::explore::{LibSort, sort_by_key};
use crate::format;
use crate::gui::app::{App, DetailKey, DetailKind, Message};
use crate::route::Route;

/// A playlist, album or artist of the library.
#[derive(Clone)]
pub(super) struct Coll {
    pub(super) title: String,
    pub(super) subtitle: String,
    pub(super) artwork: Artwork,
    pub(super) route: Route,
    pub(super) circle: bool,
    pub(super) play: Option<Message>,
}

/// What the account has: from the parent's hook, else from the account's shelves on Home.
#[derive(Default)]
pub(super) struct AccountData {
    pub(super) playlists: Vec<Coll>,
    pub(super) albums: Vec<Coll>,
    pub(super) artists: Vec<Coll>,
    pub(super) songs: Vec<Track>,
}

fn playlist_coll(p: &PlaylistSummary) -> Coll {
    Coll {
        title: p.title.clone(),
        subtitle: format!(
            "Playlist \u{b7} {}",
            p.author
                .clone()
                .unwrap_or_else(|| "YouTube Music".to_owned())
        ),
        artwork: Artwork::Remote(p.thumbnails.clone()),
        route: Route::Playlist(p.id.clone()),
        circle: false,
        play: Some(Message::PlayCollection(DetailKey {
            kind: DetailKind::Playlist,
            id: p.id.clone(),
        })),
    }
}

pub(super) fn album_coll(a: &AlbumSummary) -> Coll {
    Coll {
        title: a.title.clone(),
        subtitle: format!("Album \u{b7} {}", format::artists(&a.artists)),
        artwork: Artwork::Remote(a.thumbnails.clone()),
        route: Route::Album(a.id.clone()),
        circle: false,
        play: Some(Message::PlayCollection(DetailKey {
            kind: DetailKind::Album,
            id: a.id.clone(),
        })),
    }
}

fn artist_coll(a: &ArtistSummary) -> Coll {
    Coll {
        title: a.name.clone(),
        subtitle: "Artist".to_owned(),
        artwork: Artwork::Remote(a.thumbnails.clone()),
        route: Route::Artist(a.id.clone()),
        circle: true,
        play: None,
    }
}

pub(super) fn account_data(app: &App) -> AccountData {
    let mut d = AccountData::default();
    if let Some(lib) = &app.account_library {
        d.playlists = lib.playlists.iter().map(playlist_coll).collect();
        d.albums = lib.albums.iter().map(album_coll).collect();
        d.artists = lib.artists.iter().map(artist_coll).collect();
        d.songs.clone_from(&lib.songs);
        return d;
    }
    if !app.signed_in() {
        return d;
    }
    for shelf in &app.home.shelves {
        for item in &shelf.items {
            match item {
                ShelfItem::Playlist(p) => d.playlists.push(playlist_coll(p)),
                ShelfItem::Album(a) => d.albums.push(album_coll(a)),
                ShelfItem::Artist(a) => d.artists.push(artist_coll(a)),
                ShelfItem::Song(t) if shelf.title == "Liked songs" => d.songs.push(t.clone()),
                ShelfItem::Song(_) => {}
            }
        }
    }
    dedupe(&mut d.playlists);
    dedupe(&mut d.albums);
    dedupe(&mut d.artists);
    d
}

fn dedupe(v: &mut Vec<Coll>) {
    let mut seen: Vec<Route> = Vec::new();
    v.retain(|c| {
        if seen.contains(&c.route) {
            false
        } else {
            seen.push(c.route.clone());
            true
        }
    });
}

pub(super) fn sorted(mut v: Vec<Coll>, sort: LibSort) -> Vec<Coll> {
    sort_by_key(&mut v, sort, |c| c.title.clone(), |c| c.subtitle.clone());
    v
}

pub(super) fn local_albums(app: &App) -> Vec<Coll> {
    sorted(
        app.local_albums.iter().map(album_coll).collect(),
        app.lib_sort,
    )
}

pub(super) fn local_tracks(app: &App) -> Vec<Track> {
    let mut v = app.local_tracks.clone();
    sort_by_key(
        &mut v,
        app.lib_sort,
        |t| t.title.clone(),
        |t| format::artists(&t.artists),
    );
    v
}
