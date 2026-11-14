//! Searching and browsing the YouTube Music catalog.
//!
//! Implementations are runtime-agnostic: they do their work on their own runtime and return a
//! future that any executor can poll (the UI runs them on its own executor).

use crate::BoxFuture;
use crate::track::{Thumbnail, Track, VideoId};

mod playlist_edit;
mod video;

pub use playlist_edit::{PlaylistEditInfo, PlaylistEditor, Privacy, SetVideoId};
pub use video::{ChannelDetail, ChannelRef, ChannelSummary, SearchScope, VideoSummary};

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
    /// Matching videos ([`SearchScope::AllOfYoutube`] only).
    pub videos: Vec<VideoSummary>,
    /// Matching channels ([`SearchScope::AllOfYoutube`] only).
    pub channels: Vec<ChannelSummary>,
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
    /// Editing data, present only when the signed-in user owns the playlist.
    pub edit: Option<PlaylistEditInfo>,
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

/// A mood, genre or activity the user can open, such as a chip above the home feed.
///
/// `id` is opaque: pass it to [`Catalog::mood_playlists`] unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoodChip {
    /// Label as YouTube Music shows it.
    pub title: String,
    /// Id for [`Catalog::mood_playlists`].
    pub id: BrowseId,
}

/// A podcast show in a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodcastSummary {
    /// Id for [`Catalog::podcast`].
    pub id: BrowseId,
    /// Show title.
    pub title: String,
    /// Host or channel name, if shown.
    pub author: Option<String>,
    /// Cover sizes.
    pub thumbnails: Vec<Thumbnail>,
}

/// One podcast episode. It plays like a song: `track.video_id` goes to the stream resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeSummary {
    /// The playable item. `artists` holds the show name when known.
    pub track: Track,
    /// Short description, if shown.
    pub description: Option<String>,
    /// Publication date as YouTube Music writes it (for example `Jan 5, 2026`), if shown.
    pub published: Option<String>,
}

/// A podcast page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodcastDetail {
    /// Header data.
    pub summary: PodcastSummary,
    /// About the show, if any.
    pub description: Option<String>,
    /// Episodes, newest first as the source gives them.
    pub episodes: Vec<EpisodeSummary>,
}

/// One entry of a [`FeedShelf`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedItem {
    /// A song (audio track).
    Song(Track),
    /// A music video (official video or user upload). It plays through the same stream path as
    /// a song; the variant tells the UI to show a video-shaped card.
    Video(Track),
    /// An album, single or EP.
    Album(AlbumSummary),
    /// An artist.
    Artist(ArtistSummary),
    /// A playlist, mix or radio.
    Playlist(PlaylistSummary),
    /// A podcast show.
    Podcast(PodcastSummary),
    /// A podcast episode.
    Episode(EpisodeSummary),
    /// A mood or genre tile.
    Mood(MoodChip),
}

/// How the UI should lay out a [`FeedShelf`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShelfKind {
    /// A horizontal row of cover cards.
    #[default]
    Cards,
    /// A compact list of song rows (Quick picks, charts).
    QuickPicks,
    /// A wrap of text chips or tiles ([`FeedItem::Mood`]).
    Chips,
}

/// A titled row of a feed page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedShelf {
    /// Shelf title.
    pub title: String,
    /// Small line above the title, for example the account name on "Listen again".
    pub subtitle: Option<String>,
    /// Layout hint.
    pub kind: ShelfKind,
    /// Items in display order.
    pub items: Vec<FeedItem>,
}

/// The home page: mood chips and shelves.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HomeFeed {
    /// Mood chips shown above the shelves (relax, focus, ...). May be empty.
    pub chips: Vec<MoodChip>,
    /// Shelves in display order: Listen again, Quick picks, mixes, videos, podcasts, ...
    pub shelves: Vec<FeedShelf>,
}

/// The Explore page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Explore {
    /// New albums and singles.
    pub new_releases: Vec<AlbumSummary>,
    /// Chart shelves (top songs, trending, top artists), when YouTube Music answers them.
    pub charts: Vec<FeedShelf>,
    /// Every other shelf of the page, for example new music videos.
    pub shelves: Vec<FeedShelf>,
    /// The mood and genre tiles.
    pub moods: Vec<MoodChip>,
}

/// One day or period of listening history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistorySection {
    /// Period label as YouTube Music writes it ("Today", "Yesterday", ...).
    pub title: String,
    /// Tracks, most recent first.
    pub tracks: Vec<Track>,
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
    /// YouTube Music refused a change. The text is fixed; it never holds the response body.
    #[error("YouTube Music refused this change: {0}")]
    Rejected(String),
    /// YouTube Music asked to slow down (HTTP 429). Nothing is retried automatically.
    #[error("too many requests; try again later")]
    RateLimited,
}

/// The user's rating of a song.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LikeStatus {
    /// Liked. Replaces a dislike.
    Liked,
    /// Disliked. Replaces a like.
    Disliked,
    /// Neither liked nor disliked.
    Indifferent,
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

    /// The richer home page: mood chips, video and podcast shelves, shelf subtitles. Optional:
    /// the default says it is not available.
    fn home_feed(&self) -> BoxFuture<'_, Result<HomeFeed, CatalogError>> {
        Box::pin(async { Err(CatalogError::Unavailable("home feed".to_owned())) })
    }

    /// The account's playlists (the "Liked music" auto playlist first when present). Needs a
    /// signed-in session; the default says it is not available.
    fn library_playlists(&self) -> BoxFuture<'_, Result<Vec<PlaylistSummary>, CatalogError>> {
        Box::pin(async { Err(CatalogError::Unavailable("library playlists".to_owned())) })
    }

    /// The songs the account liked, most recent first. Needs a signed-in session.
    fn liked_songs(&self) -> BoxFuture<'_, Result<Vec<Track>, CatalogError>> {
        Box::pin(async { Err(CatalogError::Unavailable("liked songs".to_owned())) })
    }

    /// Albums saved in the account's library. Needs a signed-in session.
    fn library_albums(&self) -> BoxFuture<'_, Result<Vec<AlbumSummary>, CatalogError>> {
        Box::pin(async { Err(CatalogError::Unavailable("library albums".to_owned())) })
    }

    /// Artists saved in the account's library. Needs a signed-in session.
    fn library_artists(&self) -> BoxFuture<'_, Result<Vec<ArtistSummary>, CatalogError>> {
        Box::pin(async { Err(CatalogError::Unavailable("library artists".to_owned())) })
    }

    /// Listening history of the account, newest period first. Needs a signed-in session.
    fn history(&self) -> BoxFuture<'_, Result<Vec<HistorySection>, CatalogError>> {
        Box::pin(async { Err(CatalogError::Unavailable("history".to_owned())) })
    }

    /// New releases, charts and moods. Works without an account.
    fn explore(&self) -> BoxFuture<'_, Result<Explore, CatalogError>> {
        Box::pin(async { Err(CatalogError::Unavailable("explore".to_owned())) })
    }

    /// The playlist shelves of a mood or genre (`id` from a [`MoodChip`]). Works without an
    /// account.
    fn mood_playlists<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<Vec<FeedShelf>, CatalogError>> {
        let _ = id;
        Box::pin(async { Err(CatalogError::Unavailable("mood playlists".to_owned())) })
    }

    /// Load a podcast page with its episodes.
    fn podcast<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PodcastDetail, CatalogError>> {
        let _ = id;
        Box::pin(async { Err(CatalogError::Unavailable("podcasts".to_owned())) })
    }

    /// Set the rating of a song. Needs a signed-in session: the default, and every anonymous
    /// catalog, says it is not available. It never calls [`Catalog::rate_song`], so a catalog
    /// that implements neither method gets an error, not a loop.
    fn set_rating<'a>(
        &'a self,
        video_id: &'a VideoId,
        status: LikeStatus,
    ) -> BoxFuture<'a, Result<(), CatalogError>> {
        let _ = (video_id, status);
        Box::pin(async {
            Err(CatalogError::Unavailable(
                "likes need a signed-in account".to_owned(),
            ))
        })
    }

    /// Like (`liked = true`) or remove the like of a song. The default goes through
    /// [`Catalog::set_rating`] with [`LikeStatus::Liked`] or [`LikeStatus::Indifferent`].
    fn rate_song<'a>(
        &'a self,
        video_id: &'a VideoId,
        liked: bool,
    ) -> BoxFuture<'a, Result<(), CatalogError>> {
        let status = if liked {
            LikeStatus::Liked
        } else {
            LikeStatus::Indifferent
        };
        self.set_rating(video_id, status)
    }

    /// Search in a scope. Optional: the default says it is not available.
    fn search_scoped<'a>(
        &'a self,
        query: &'a str,
        scope: SearchScope,
    ) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
        let _ = (query, scope);
        Box::pin(async { Err(CatalogError::Unavailable("search scope".to_owned())) })
    }

    /// Load a YouTube channel page. Optional: the default says it is not available.
    fn channel<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<ChannelDetail, CatalogError>> {
        let _ = id;
        Box::pin(async { Err(CatalogError::Unavailable("channels".to_owned())) })
    }

    /// The YouTube live streams to show. Optional: the default says it is not available.
    fn live_streams(&self) -> BoxFuture<'_, Result<Vec<VideoSummary>, CatalogError>> {
        Box::pin(async { Err(CatalogError::Unavailable("live streams".to_owned())) })
    }
}

#[cfg(test)]
mod tests;
