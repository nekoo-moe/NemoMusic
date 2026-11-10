//! Searching and browsing the YouTube Music catalog.
//!
//! Implementations are runtime-agnostic: they do their work on their own runtime and return a
//! future that any executor can poll (the UI runs them on its own executor).

use crate::BoxFuture;
use crate::track::{Thumbnail, Track};

/// Opaque id of an album, artist or playlist page. Treat it as untrusted text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BrowseId(pub String);

/// An album in a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlbumSummary {
    /// Id for [`Catalog::album`].
    pub id: BrowseId,
    /// Album title.
    pub title: String,
    /// Artist names.
    pub artists: Vec<String>,
    /// Release year, if known.
    pub year: Option<u16>,
    /// Cover sizes.
    pub thumbnails: Vec<Thumbnail>,
}

/// An artist in a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtistSummary {
    /// Id for [`Catalog::artist`].
    pub id: BrowseId,
    /// Artist name.
    pub name: String,
    /// Picture sizes.
    pub thumbnails: Vec<Thumbnail>,
}

/// A playlist in a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistSummary {
    /// Id for [`Catalog::playlist`].
    pub id: BrowseId,
    /// Playlist title.
    pub title: String,
    /// Owner name, if shown.
    pub author: Option<String>,
    /// Cover sizes.
    pub thumbnails: Vec<Thumbnail>,
}

/// Search answer, grouped by kind.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchResults {
    /// Matching songs.
    pub songs: Vec<Track>,
    /// Matching albums.
    pub albums: Vec<AlbumSummary>,
    /// Matching artists.
    pub artists: Vec<ArtistSummary>,
    /// Matching playlists.
    pub playlists: Vec<PlaylistSummary>,
}

/// An album page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlbumDetail {
    /// Header data.
    pub summary: AlbumSummary,
    /// Tracks in album order.
    pub tracks: Vec<Track>,
}

/// An artist page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtistDetail {
    /// Header data.
    pub summary: ArtistSummary,
    /// Popular songs.
    pub top_songs: Vec<Track>,
    /// Albums and singles.
    pub albums: Vec<AlbumSummary>,
    /// Short biography, if any.
    pub description: Option<String>,
}

/// A playlist page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistDetail {
    /// Header data.
    pub summary: PlaylistSummary,
    /// Tracks in playlist order.
    pub tracks: Vec<Track>,
}

/// One entry of a home-feed shelf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShelfItem {
    /// A song.
    Song(Track),
    /// An album.
    Album(AlbumSummary),
    /// An artist.
    Artist(ArtistSummary),
    /// A playlist.
    Playlist(PlaylistSummary),
}

/// A titled row of the home feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shelf {
    /// Shelf title.
    pub title: String,
    /// Items in display order.
    pub items: Vec<ShelfItem>,
}

/// A catalog request failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CatalogError {
    /// The network request failed.
    #[error("network error: {0}")]
    Network(String),
    /// The answer could not be parsed.
    #[error("could not parse catalog answer: {0}")]
    Parse(String),
    /// The feature is not available (not supported, or YouTube changed).
    #[error("not available: {0}")]
    Unavailable(String),
    /// The page does not exist.
    #[error("not found")]
    NotFound,
}

/// The catalog. Human-paced use only: callers must not fire bulk requests.
pub trait Catalog: Send + Sync {
    /// Search for songs, albums, artists and playlists.
    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<SearchResults, CatalogError>>;

    /// Load an album page.
    fn album<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<AlbumDetail, CatalogError>>;

    /// Load an artist page.
    fn artist<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<ArtistDetail, CatalogError>>;

    /// Load a playlist page.
    fn playlist<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PlaylistDetail, CatalogError>>;

    /// Load the home feed. Optional: the default says it is not available.
    fn home(&self) -> BoxFuture<'_, Result<Vec<Shelf>, CatalogError>> {
        Box::pin(async { Err(CatalogError::Unavailable("home feed".to_owned())) })
    }
}
