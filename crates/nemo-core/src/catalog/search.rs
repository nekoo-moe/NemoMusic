//! The parts of a search answer that keep YouTube Music's own ranking: the top result and the
//! order of the result kinds (search-like-ytm).

use super::{AlbumSummary, ArtistSummary, PlaylistSummary, SearchResults, VideoSummary};
use crate::track::Track;

/// A kind of search result, one section of the search page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchKind {
    /// [`SearchResults::songs`].
    Songs,
    /// [`SearchResults::albums`].
    Albums,
    /// [`SearchResults::artists`].
    Artists,
    /// [`SearchResults::playlists`].
    Playlists,
    /// [`SearchResults::videos`].
    Videos,
    /// [`SearchResults::channels`].
    Channels,
}

impl SearchKind {
    /// The section order used for kinds the source did not rank.
    pub const DEFAULT_ORDER: [Self; 6] = [
        Self::Songs,
        Self::Albums,
        Self::Artists,
        Self::Playlists,
        Self::Videos,
        Self::Channels,
    ];
}

/// The single best match, shown above the sections (YouTube Music's "Top result").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopResult {
    /// A song.
    Song(Track),
    /// An album, single or EP.
    Album(AlbumSummary),
    /// An artist.
    Artist(ArtistSummary),
    /// A playlist.
    Playlist(PlaylistSummary),
    /// A music video.
    Video(VideoSummary),
}

impl SearchResults {
    /// How many results of `kind` there are.
    #[must_use]
    pub fn count(&self, kind: SearchKind) -> usize {
        match kind {
            SearchKind::Songs => self.songs.len(),
            SearchKind::Albums => self.albums.len(),
            SearchKind::Artists => self.artists.len(),
            SearchKind::Playlists => self.playlists.len(),
            SearchKind::Videos => self.videos.len(),
            SearchKind::Channels => self.channels.len(),
        }
    }

    /// `true` when there is no top result and every section is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.top.is_none()
            && SearchKind::DEFAULT_ORDER
                .iter()
                .all(|k| self.count(*k) == 0)
    }

    /// The non-empty sections in display order: [`SearchResults::order`] first (each kind
    /// once), then the remaining kinds in [`SearchKind::DEFAULT_ORDER`].
    #[must_use]
    pub fn sections(&self) -> Vec<SearchKind> {
        let mut out: Vec<SearchKind> = Vec::with_capacity(SearchKind::DEFAULT_ORDER.len());
        for kind in self.order.iter().chain(SearchKind::DEFAULT_ORDER.iter()) {
            if self.count(*kind) > 0 && !out.contains(kind) {
                out.push(*kind);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::BrowseId;

    fn artist(name: &str) -> ArtistSummary {
        ArtistSummary {
            id: BrowseId(format!("UC{name}")),
            name: name.to_owned(),
            thumbnails: Vec::new(),
        }
    }

    #[test]
    fn sections_follow_the_source_order_then_the_default() {
        let r = SearchResults {
            artists: vec![artist("a")],
            playlists: vec![PlaylistSummary {
                id: BrowseId("VLx".to_owned()),
                title: "p".to_owned(),
                author: None,
                thumbnails: Vec::new(),
            }],
            order: vec![
                SearchKind::Playlists,
                SearchKind::Songs,
                SearchKind::Playlists,
            ],
            ..SearchResults::default()
        };
        assert_eq!(
            r.sections(),
            vec![SearchKind::Playlists, SearchKind::Artists],
            "empty kinds are skipped, a repeated kind counts once"
        );
    }

    #[test]
    fn a_top_result_alone_is_not_empty() {
        assert!(SearchResults::default().is_empty());
        let r = SearchResults {
            top: Some(TopResult::Artist(artist("a"))),
            ..SearchResults::default()
        };
        assert!(!r.is_empty());
        assert!(r.sections().is_empty());
    }
}
