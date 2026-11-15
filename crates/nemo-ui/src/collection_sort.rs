//! The Sort control of album and playlist pages (collection-sort-meta spec): the orders, the
//! sorting itself, and the per-page memory kept in the settings. Pure, no GUI dependency.
//!
//! Every order is stable: songs that compare equal keep the page's own order. Missing values
//! (no album, no length) go last. "Newest first" and "Oldest first" of YouTube Music need the
//! date each song was added, which the catalog does not give, so they are not offered.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use nemo_core::track::Track;

/// Most pages whose order is remembered; past it the alphabetically first key is forgotten, so
/// the settings file stays small.
pub const MAX_REMEMBERED: usize = 300;

/// One order of the track list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CollectionSort {
    /// The page's own order.
    #[default]
    Default,
    /// Title, A to Z.
    Title,
    /// First artist, A to Z.
    Artist,
    /// Album, A to Z (playlists only).
    Album,
    /// Shortest first.
    Duration,
}

impl CollectionSort {
    /// Every order, in menu order.
    pub const ALL: [Self; 5] = [
        Self::Default,
        Self::Title,
        Self::Artist,
        Self::Album,
        Self::Duration,
    ];

    /// Menu label (also the stored name).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Default => "Default order",
            Self::Title => "Title",
            Self::Artist => "Artist",
            Self::Album => "Album",
            Self::Duration => "Duration",
        }
    }

    /// The order stored under `label`; unknown text gives `None`.
    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.label() == label)
    }

    /// The orders that make sense on a page: an album page has one album, so no Album order.
    #[must_use]
    pub fn offered(album_page: bool) -> Vec<Self> {
        Self::ALL
            .into_iter()
            .filter(|s| !(album_page && *s == Self::Album))
            .collect()
    }
}

/// The settings key of a page: `album:<id>` or `playlist:<id>`.
#[must_use]
pub fn page_key(album: bool, id: &str) -> String {
    if album {
        format!("album:{id}")
    } else {
        format!("playlist:{id}")
    }
}

fn text_key(text: &str) -> String {
    text.trim().to_lowercase()
}

/// Compare optional values with the missing ones last.
fn missing_last<T: Ord>(a: Option<T>, b: Option<T>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// Positions of `tracks` in the order `sort` shows them: `order[k]` is the page index of the
/// song shown at row `k`.
#[must_use]
pub fn order(tracks: &[Track], sort: CollectionSort) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..tracks.len()).collect();
    let key = |i: usize| -> &Track { &tracks[i] };
    match sort {
        CollectionSort::Default => {}
        CollectionSort::Title => idx.sort_by_cached_key(|&i| text_key(&key(i).title)),
        CollectionSort::Artist => idx.sort_by_cached_key(|&i| {
            key(i)
                .artists
                .first()
                .map_or_else(|| "\u{10FFFF}".to_owned(), |a| text_key(a))
        }),
        CollectionSort::Album => idx.sort_by(|&a, &b| {
            missing_last(
                key(a).album.as_deref().map(text_key),
                key(b).album.as_deref().map(text_key),
            )
        }),
        CollectionSort::Duration => {
            idx.sort_by(|&a, &b| missing_last(key(a).duration, key(b).duration));
        }
    }
    idx
}

/// `tracks` in the order `sort` shows them.
#[must_use]
pub fn sorted(tracks: &[Track], sort: CollectionSort) -> Vec<Track> {
    order(tracks, sort)
        .into_iter()
        .map(|i| tracks[i].clone())
        .collect()
}

/// Remember `sort` for the page `key`: the default order is forgotten (it needs no entry), and
/// at most [`MAX_REMEMBERED`] pages are kept.
pub fn remember(map: &mut BTreeMap<String, CollectionSort>, key: &str, sort: CollectionSort) {
    if sort == CollectionSort::Default {
        map.remove(key);
        return;
    }
    if !map.contains_key(key) && map.len() >= MAX_REMEMBERED {
        let first = map.keys().next().cloned();
        if let Some(first) = first {
            map.remove(&first);
        }
    }
    map.insert(key.to_owned(), sort);
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::mock;

    fn t(title: &str, artist: &str, album: Option<&str>, secs: Option<u64>) -> Track {
        let mut tr = mock::track(0);
        tr.id = nemo_core::track::TrackId(format!("{title}-{artist}"));
        tr.title = title.to_owned();
        tr.artists = if artist.is_empty() {
            Vec::new()
        } else {
            vec![artist.to_owned()]
        };
        tr.album = album.map(str::to_owned);
        tr.duration = secs.map(Duration::from_secs);
        tr
    }

    fn list() -> Vec<Track> {
        vec![
            t("bravo", "Zed", Some("Night"), Some(200)),
            t("Alpha", "anna", None, Some(90)),
            t("charlie", "", Some("dawn"), None),
            t("alpha", "Bea", Some("Dawn"), Some(90)),
        ]
    }

    fn titles(tracks: &[Track]) -> Vec<&str> {
        tracks.iter().map(|t| t.title.as_str()).collect()
    }

    #[test]
    fn every_order_is_stable_and_puts_missing_values_last() {
        let l = list();
        assert_eq!(order(&l, CollectionSort::Default), vec![0, 1, 2, 3]);
        // Case-insensitive; equal titles keep the page order.
        assert_eq!(order(&l, CollectionSort::Title), vec![1, 3, 0, 2]);
        // No artist goes last.
        assert_eq!(order(&l, CollectionSort::Artist), vec![1, 3, 0, 2]);
        // "dawn" and "Dawn" tie and keep their order; no album goes last.
        assert_eq!(order(&l, CollectionSort::Album), vec![2, 3, 0, 1]);
        // Equal lengths keep their order; no length goes last.
        assert_eq!(order(&l, CollectionSort::Duration), vec![1, 3, 0, 2]);
        assert_eq!(
            titles(&sorted(&l, CollectionSort::Title)),
            ["Alpha", "alpha", "bravo", "charlie"]
        );
        assert!(order(&[], CollectionSort::Title).is_empty());
    }

    #[test]
    fn labels_round_trip_and_album_pages_offer_no_album_order() {
        for s in CollectionSort::ALL {
            assert_eq!(CollectionSort::from_label(s.label()), Some(s));
        }
        assert_eq!(CollectionSort::from_label("Newest first"), None);
        assert!(!CollectionSort::offered(true).contains(&CollectionSort::Album));
        assert_eq!(
            CollectionSort::offered(false).len(),
            CollectionSort::ALL.len()
        );
    }

    #[test]
    fn the_memory_forgets_the_default_and_stays_bounded() {
        let mut m = BTreeMap::new();
        remember(&mut m, "playlist:a", CollectionSort::Title);
        assert_eq!(m.get("playlist:a"), Some(&CollectionSort::Title));
        remember(&mut m, "playlist:a", CollectionSort::Default);
        assert!(m.is_empty());
        for i in 0..MAX_REMEMBERED + 20 {
            remember(
                &mut m,
                &format!("playlist:{i:04}"),
                CollectionSort::Duration,
            );
        }
        assert_eq!(m.len(), MAX_REMEMBERED);
        remember(&mut m, "playlist:0319", CollectionSort::Album);
        assert_eq!(
            m.len(),
            MAX_REMEMBERED,
            "a known key is updated, nothing dropped"
        );
    }
}
