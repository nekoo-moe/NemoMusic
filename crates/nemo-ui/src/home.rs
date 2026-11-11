//! What Home shows when the catalog has no home feed: shelves built from a few calm seed
//! searches, local albums, and the recently played tracks. Pure.

use nemo_core::catalog::{AlbumSummary, SearchResults, Shelf, ShelfItem};
use nemo_core::track::Track;

/// Prefix of the ids of local albums in search results.
pub const LOCAL_ALBUM_PREFIX: &str = "local-album:";
/// Calm seed searches `(shelf title, query)`, run one at a time and cached for the session.
pub const SEED_SHELVES: [(&str, &str); 4] = [
    ("Lo-fi to settle in", "lofi"),
    ("Piano", "piano"),
    ("Acoustic mornings", "acoustic"),
    ("Jazz for the evening", "jazz"),
];
/// Most items on one shelf.
pub const SHELF_ITEMS: usize = 12;
/// Most recently played tracks kept.
pub const RECENT_MAX: usize = 12;

/// `true` for the id of a local album.
#[must_use]
pub fn is_local_album(album: &AlbumSummary) -> bool {
    album.id.0.starts_with(LOCAL_ALBUM_PREFIX)
}

/// Local albums in `results`.
#[must_use]
pub fn local_albums(results: &SearchResults) -> Vec<AlbumSummary> {
    results
        .albums
        .iter()
        .filter(|a| is_local_album(a))
        .cloned()
        .collect()
}

/// Add `new` albums to `known`, skipping ids that are already there.
pub fn merge_albums(known: &mut Vec<AlbumSummary>, new: Vec<AlbumSummary>) {
    for a in new {
        if !known.iter().any(|k| k.id == a.id) {
            known.push(a);
        }
    }
}

/// Add local tracks of `results` to `known` (by id).
pub fn merge_local_tracks(known: &mut Vec<Track>, results: &SearchResults) {
    for t in results
        .songs
        .iter()
        .filter(|t| t.origin == nemo_core::audio::Origin::Local)
    {
        if !known.iter().any(|k| k.id == t.id) {
            known.push(t.clone());
        }
    }
}

/// A shelf made of one search answer: albums first, then playlists, then songs, then artists;
/// at most [`SHELF_ITEMS`]. Local albums are left out (they have their own shelf). `None` when
/// nothing is left, so the shelf is hidden.
#[must_use]
pub fn shelf_from_search(title: &str, results: SearchResults) -> Option<Shelf> {
    let mut items: Vec<ShelfItem> = Vec::new();
    items.extend(
        results
            .albums
            .into_iter()
            .filter(|a| !is_local_album(a))
            .map(ShelfItem::Album),
    );
    items.extend(results.playlists.into_iter().map(ShelfItem::Playlist));
    items.extend(results.songs.into_iter().map(ShelfItem::Song));
    items.extend(results.artists.into_iter().map(ShelfItem::Artist));
    items.truncate(SHELF_ITEMS);
    (!items.is_empty()).then(|| Shelf {
        title: title.to_owned(),
        items,
    })
}

/// Put `track` first in `recent` (no duplicates, at most [`RECENT_MAX`]).
pub fn remember(recent: &mut Vec<Track>, track: &Track) {
    recent.retain(|t| t.id != track.id);
    recent.insert(0, track.clone());
    recent.truncate(RECENT_MAX);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock;
    use nemo_core::catalog::BrowseId;

    fn album(id: &str) -> AlbumSummary {
        AlbumSummary {
            id: BrowseId(id.to_owned()),
            title: id.to_owned(),
            artists: vec![],
            year: None,
            thumbnails: vec![],
        }
    }

    #[test]
    fn seeds_are_four_calm_searches() {
        assert_eq!(SEED_SHELVES.len(), 4);
        assert!(
            SEED_SHELVES
                .iter()
                .all(|(t, q)| !t.is_empty() && !q.is_empty())
        );
    }

    #[test]
    fn local_albums_are_found_by_prefix_and_deduplicated() {
        let r = SearchResults {
            albums: vec![
                album("local-album:1"),
                album("MPRE1"),
                album("local-album:2"),
            ],
            ..SearchResults::default()
        };
        let mut known = local_albums(&r);
        assert_eq!(known.len(), 2);
        merge_albums(
            &mut known,
            vec![album("local-album:2"), album("local-album:3")],
        );
        assert_eq!(known.len(), 3);
    }

    #[test]
    fn a_shelf_without_remote_items_is_hidden() {
        let r = SearchResults {
            albums: vec![album("local-album:1")],
            ..SearchResults::default()
        };
        assert_eq!(shelf_from_search("x", r), None);
        let r = SearchResults {
            albums: vec![album("MPRE1")],
            songs: mock::tracks(),
            ..SearchResults::default()
        };
        let shelf = shelf_from_search("x", r).expect("shelf");
        assert_eq!(shelf.items.len(), SHELF_ITEMS);
        assert!(matches!(shelf.items[0], ShelfItem::Album(_)));
    }

    #[test]
    fn recent_tracks_are_newest_first_without_duplicates() {
        let mut recent = Vec::new();
        for n in [0, 1, 2, 1] {
            remember(&mut recent, &mock::track(n));
        }
        let ids: Vec<_> = recent.iter().map(|t| t.id.0.clone()).collect();
        assert_eq!(ids, ["mock-track-1", "mock-track-2", "mock-track-0"]);
        for n in 0..30 {
            remember(&mut recent, &mock::track(n));
        }
        assert_eq!(recent.len(), RECENT_MAX);
    }

    #[test]
    fn local_tracks_merge_by_id() {
        let r = SearchResults {
            songs: mock::tracks(),
            ..SearchResults::default()
        };
        let mut known = Vec::new();
        merge_local_tracks(&mut known, &r);
        let n = known.len();
        assert!(n > 0 && known.iter().all(|t| t.local_path.is_some()));
        merge_local_tracks(&mut known, &r);
        assert_eq!(known.len(), n);
    }
}
