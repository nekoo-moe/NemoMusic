//! Library, liked-songs and playlist-like lists.

use nemo_core::catalog::{AlbumSummary, ArtistSummary, FeedItem, PlaylistSummary};
use nemo_core::track::Track;
use serde_json::Value;

use super::item::{feed_item, song_row};
use super::json::find_key;
use super::shelf::{LIST_SCOPES, continuation_token, parse_root};

/// One part of a long list and the token of the next part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Page<T> {
    pub items: Vec<T>,
    pub next: Option<String>,
}

/// The entries of the list on this page: playlist shelf, plain shelf or grid, first page or
/// continuation.
fn list_entries(root: &Value) -> Vec<&Value> {
    let containers: [(&str, &str); 7] = [
        ("musicPlaylistShelfRenderer", "contents"),
        ("musicPlaylistShelfContinuation", "contents"),
        ("musicShelfContinuation", "contents"),
        ("gridContinuation", "items"),
        ("musicShelfRenderer", "contents"),
        ("gridRenderer", "items"),
        ("appendContinuationItemsAction", "continuationItems"),
    ];
    for (key, field) in containers {
        if let Some(entries) = find_key(root, key)
            .and_then(|c| c.get(field))
            .and_then(Value::as_array)
        {
            return entries.iter().collect();
        }
    }
    Vec::new()
}

fn list_page<T>(json: &str, pick: impl Fn(FeedItem) -> Option<T>) -> Page<T> {
    let Some(root) = parse_root(json) else {
        return Page {
            items: Vec::new(),
            next: None,
        };
    };
    Page {
        items: list_entries(&root)
            .into_iter()
            .filter_map(feed_item)
            .filter_map(pick)
            .collect(),
        next: continuation_token(&root, &LIST_SCOPES),
    }
}

/// The account's playlists.
pub(crate) fn parse_library_playlists(json: &str) -> Page<PlaylistSummary> {
    list_page(json, |i| match i {
        FeedItem::Playlist(p) => Some(p),
        _ => None,
    })
}

/// The account's saved albums.
pub(crate) fn parse_library_albums(json: &str) -> Page<AlbumSummary> {
    list_page(json, |i| match i {
        FeedItem::Album(a) => Some(a),
        _ => None,
    })
}

/// The account's saved artists.
pub(crate) fn parse_library_artists(json: &str) -> Page<ArtistSummary> {
    list_page(json, |i| match i {
        FeedItem::Artist(a) => Some(a),
        _ => None,
    })
}

/// Playable rows of a playlist-like page (liked songs, a playlist, an album-less list).
pub(crate) fn parse_song_rows(json: &str) -> Page<Track> {
    let Some(root) = parse_root(json) else {
        return Page {
            items: Vec::new(),
            next: None,
        };
    };
    Page {
        items: list_entries(&root)
            .into_iter()
            .filter_map(|e| e.get("musicResponsiveListItemRenderer"))
            .filter_map(song_row)
            .map(|(track, _)| track)
            .collect(),
        next: continuation_token(&root, &LIST_SCOPES),
    }
}
