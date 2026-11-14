//! The plain structs the `ytmapi-rs` adapters fill.

use nemo_core::track::Thumbnail;

// ---------------------------------------------------------------------------------------------
// Plain intermediate types
// ---------------------------------------------------------------------------------------------

/// A song as copied from a `ytmapi-rs` answer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RawSong {
    pub video_id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    /// `m:ss` or `h:mm:ss`.
    pub duration: Option<String>,
    pub thumbnails: Vec<Thumbnail>,
}

/// An album in a list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RawAlbum {
    pub id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub year: Option<String>,
    pub thumbnails: Vec<Thumbnail>,
}

/// An artist in a list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RawArtist {
    pub id: String,
    pub name: String,
    pub thumbnails: Vec<Thumbnail>,
}

/// A playlist in a list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RawPlaylist {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub thumbnails: Vec<Thumbnail>,
}

/// One track row of an album page (the page itself carries artists and cover).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RawAlbumTrack {
    pub video_id: String,
    pub title: String,
    pub duration: Option<String>,
}

/// An album page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RawAlbumPage {
    pub album: RawAlbum,
    pub tracks: Vec<RawAlbumTrack>,
}

/// An artist page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RawArtistPage {
    pub artist: RawArtist,
    pub description: Option<String>,
    pub songs: Vec<RawSong>,
    pub albums: Vec<RawAlbum>,
}

/// A playlist page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RawPlaylistPage {
    pub playlist: RawPlaylist,
    pub songs: Vec<RawSong>,
}
