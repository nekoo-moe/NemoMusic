//! Anonymous YouTube Music catalog on top of `ytmapi-rs` 0.3.4.
//!
//! No sign-in: the handle is `YtMusic<NoAuthToken>`. It asks YouTube Music for a visitor id the
//! first time it is needed. Search, album, artist and playlist pages work this way; the home feed
//! stays unavailable.
//!
//! Mapping is split in two steps so it can be tested without the network: thin adapters copy
//! the fields we need out of `ytmapi-rs` types into the plain `Raw*` structs, and pure `map_*`
//! functions turn those into `nemo-core` types. A song without a valid [`VideoId`], without a
//! title, or that cannot be played (podcasts, episodes, uploads, greyed-out items) is dropped.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use nemo_core::BoxFuture;
use nemo_core::audio::Origin;
use nemo_core::catalog::{
    AlbumDetail, AlbumSummary, ArtistDetail, ArtistSummary, BrowseId, Catalog, CatalogError,
    Explore, FeedShelf, HomeFeed, PlaylistDetail, PlaylistSummary, PodcastDetail, SearchResults,
};
use nemo_core::track::{Artwork, Thumbnail, Track, TrackId, VideoId};
use tokio::runtime::Handle;
use tokio::sync::OnceCell;
use ytmapi_rs::YtMusic;
use ytmapi_rs::auth::noauth::NoAuthToken;
use ytmapi_rs::common::{AlbumID, ArtistChannelID, PlaylistID, YoutubeID};
use ytmapi_rs::error::ErrorKind;

use crate::pages;

/// Longest search text sent, in characters.
const MAX_QUERY_CHARS: usize = 200;
/// Longest browse or playlist id accepted, in bytes.
const MAX_ID_BYTES: usize = 128;

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

// ---------------------------------------------------------------------------------------------
// Pure mapping
// ---------------------------------------------------------------------------------------------

fn clean(text: &str) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn clean_all(texts: &[String]) -> Vec<String> {
    texts.iter().filter_map(|t| clean(t)).collect()
}

/// `m:ss`, `mm:ss` or `h:mm:ss` to a duration. Anything else (also `0:00`) gives `None`.
pub(crate) fn parse_duration(text: &str) -> Option<Duration> {
    let mut seconds: u64 = 0;
    let mut parts = 0_u32;
    for part in text.trim().split(':') {
        if part.is_empty() || part.len() > 2 || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        seconds = seconds * 60 + part.parse::<u64>().ok()?;
        parts += 1;
    }
    ((2..=3).contains(&parts) && seconds > 0).then(|| Duration::from_secs(seconds))
}

/// The first run of four digits in `text` as a year, for example `"2019"` or `"Album • 2019"`.
pub(crate) fn parse_year(text: &str) -> Option<u16> {
    let bytes = text.as_bytes();
    bytes
        .windows(4)
        .enumerate()
        .find(|(index, window)| {
            window.iter().all(u8::is_ascii_digit)
                && !bytes
                    .get(index.wrapping_sub(1))
                    .is_some_and(u8::is_ascii_digit)
                && !bytes.get(index + 4).is_some_and(u8::is_ascii_digit)
        })
        .and_then(|(index, _)| text[index..index + 4].parse().ok())
}

/// Keep thumbnails with a non-empty URL; sizes that do not fit `u32` become `0` (unknown).
pub(crate) fn map_thumbnails(raw: &[(String, u64, u64)]) -> Vec<Thumbnail> {
    raw.iter()
        .filter(|(url, _, _)| !url.trim().is_empty())
        .map(|(url, width, height)| Thumbnail {
            url: url.trim().to_owned(),
            width: u32::try_from(*width).unwrap_or(0),
            height: u32::try_from(*height).unwrap_or(0),
        })
        .collect()
}

fn artwork(thumbnails: &[Thumbnail]) -> Artwork {
    if thumbnails.is_empty() {
        Artwork::None
    } else {
        Artwork::Remote(thumbnails.to_vec())
    }
}

fn build_track(
    video_id: &str,
    title: &str,
    artists: Vec<String>,
    album: Option<String>,
    duration: Option<&str>,
    thumbnails: &[Thumbnail],
) -> Option<Track> {
    let video = VideoId::parse(video_id).ok()?;
    let title = clean(title)?;
    Some(Track {
        id: TrackId(video.as_str().to_owned()),
        title,
        artists,
        album,
        duration: duration.and_then(parse_duration),
        origin: Origin::YoutubeMusic,
        video_id: Some(video),
        local_path: None,
        artwork: artwork(thumbnails),
    })
}

/// A song, or `None` when the id or the title is unusable.
pub(crate) fn map_song(raw: &RawSong) -> Option<Track> {
    build_track(
        &raw.video_id,
        &raw.title,
        clean_all(&raw.artists),
        raw.album.as_deref().and_then(clean),
        raw.duration.as_deref(),
        &raw.thumbnails,
    )
}

fn map_songs(raw: &[RawSong]) -> Vec<Track> {
    raw.iter().filter_map(map_song).collect()
}

/// An album summary, or `None` without an id or a title.
pub(crate) fn map_album(raw: &RawAlbum) -> Option<AlbumSummary> {
    let id = clean(&raw.id)?;
    Some(AlbumSummary {
        id: BrowseId(id),
        title: clean(&raw.title)?,
        artists: clean_all(&raw.artists),
        year: raw.year.as_deref().and_then(parse_year),
        thumbnails: raw.thumbnails.clone(),
    })
}

/// An artist summary, or `None` without an id or a name.
pub(crate) fn map_artist(raw: &RawArtist) -> Option<ArtistSummary> {
    Some(ArtistSummary {
        id: BrowseId(clean(&raw.id)?),
        name: clean(&raw.name)?,
        thumbnails: raw.thumbnails.clone(),
    })
}

/// A playlist summary, or `None` without an id or a title.
pub(crate) fn map_playlist(raw: &RawPlaylist) -> Option<PlaylistSummary> {
    Some(PlaylistSummary {
        id: BrowseId(clean(&raw.id)?),
        title: clean(&raw.title)?,
        author: raw.author.as_deref().and_then(clean),
        thumbnails: raw.thumbnails.clone(),
    })
}

/// Album page: the tracks get the album's artists, title and cover.
pub(crate) fn map_album_page(raw: &RawAlbumPage, requested: &BrowseId) -> Option<AlbumDetail> {
    let mut summary = map_album(&raw.album)?;
    summary.id = requested.clone();
    let artists = summary.artists.clone();
    let tracks = raw
        .tracks
        .iter()
        .filter_map(|t| {
            build_track(
                &t.video_id,
                &t.title,
                artists.clone(),
                Some(summary.title.clone()),
                t.duration.as_deref(),
                &raw.album.thumbnails,
            )
        })
        .collect();
    Some(AlbumDetail { summary, tracks })
}

/// Artist page.
pub(crate) fn map_artist_page(raw: &RawArtistPage, requested: &BrowseId) -> Option<ArtistDetail> {
    let mut summary = map_artist(&raw.artist)?;
    summary.id = requested.clone();
    Some(ArtistDetail {
        summary,
        top_songs: map_songs(&raw.songs),
        albums: raw.albums.iter().filter_map(map_album).collect(),
        description: raw.description.as_deref().and_then(clean),
    })
}

/// Playlist page.
pub(crate) fn map_playlist_page(
    raw: &RawPlaylistPage,
    requested: &BrowseId,
) -> Option<PlaylistDetail> {
    let mut summary = map_playlist(&raw.playlist)?;
    summary.id = requested.clone();
    Some(PlaylistDetail {
        summary,
        tracks: map_songs(&raw.songs),
    })
}

/// Browse and playlist ids come from the network. Accept only short ids made of letters, digits,
/// `-` and `_`, so nothing odd reaches a request body.
pub(crate) fn valid_browse_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_BYTES
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

// ---------------------------------------------------------------------------------------------
// Adapters from ytmapi-rs types
// ---------------------------------------------------------------------------------------------

pub(crate) mod adapt {
    use ytmapi_rs::common::{Thumbnail as ApiThumbnail, YoutubeID};
    use ytmapi_rs::parse::{
        AlbumResult, ArtistSong, GetAlbum, GetArtist, GetPlaylistDetails, ParsedSongArtist,
        PlaylistItem, SearchResultAlbum, SearchResultArtist, SearchResultFeaturedPlaylist,
        SearchResultPlaylist, SearchResultSong,
    };

    use super::{
        RawAlbum, RawAlbumPage, RawAlbumTrack, RawArtist, RawArtistPage, RawPlaylist,
        RawPlaylistPage, RawSong, Thumbnail, map_thumbnails,
    };

    pub fn thumbs(raw: &[ApiThumbnail]) -> Vec<Thumbnail> {
        let list: Vec<(String, u64, u64)> = raw
            .iter()
            .map(|t| (t.url.clone(), t.width, t.height))
            .collect();
        map_thumbnails(&list)
    }

    fn names(artists: &[ParsedSongArtist]) -> Vec<String> {
        artists.iter().map(|a| a.name.clone()).collect()
    }

    pub fn search_song(s: &SearchResultSong) -> RawSong {
        RawSong {
            video_id: s.video_id.get_raw().to_owned(),
            title: s.title.clone(),
            artists: vec![s.artist.clone()],
            album: s.album.as_ref().map(|a| a.name.clone()),
            duration: Some(s.duration.clone()),
            thumbnails: thumbs(&s.thumbnails),
        }
    }

    pub fn search_album(a: &SearchResultAlbum) -> RawAlbum {
        RawAlbum {
            id: a.album_id.get_raw().to_owned(),
            title: a.title.clone(),
            artists: vec![a.artist.clone()],
            year: Some(a.year.clone()),
            thumbnails: thumbs(&a.thumbnails),
        }
    }

    pub fn search_artist(a: &SearchResultArtist) -> RawArtist {
        RawArtist {
            id: a.browse_id.get_raw().to_owned(),
            name: a.artist.clone(),
            thumbnails: thumbs(&a.thumbnails),
        }
    }

    pub fn featured_playlist(p: &SearchResultFeaturedPlaylist) -> RawPlaylist {
        RawPlaylist {
            id: p.playlist_id.get_raw().to_owned(),
            title: p.title.clone(),
            author: Some(p.author.clone()),
            thumbnails: thumbs(&p.thumbnails),
        }
    }

    /// Featured and community playlists. Podcasts, which the playlist filter can return, are
    /// dropped.
    pub fn search_playlist(p: &SearchResultPlaylist) -> Option<RawPlaylist> {
        match p {
            SearchResultPlaylist::Featured(p) => Some(featured_playlist(p)),
            SearchResultPlaylist::Community(p) => Some(RawPlaylist {
                id: p.playlist_id.get_raw().to_owned(),
                title: p.title.clone(),
                author: Some(p.author.clone()),
                thumbnails: thumbs(&p.thumbnails),
            }),
            _ => None,
        }
    }

    fn album_result(a: &AlbumResult) -> RawAlbum {
        RawAlbum {
            id: a.album_id.get_raw().to_owned(),
            title: a.title.clone(),
            artists: Vec::new(),
            year: Some(a.year.clone()),
            thumbnails: thumbs(&a.thumbnails),
        }
    }

    fn artist_song(s: &ArtistSong) -> RawSong {
        RawSong {
            video_id: s.video_id.get_raw().to_owned(),
            title: s.title.clone(),
            artists: names(&s.artists),
            album: Some(s.album.name.clone()),
            duration: None,
            thumbnails: Vec::new(),
        }
    }

    pub fn album_page(a: &GetAlbum) -> RawAlbumPage {
        RawAlbumPage {
            album: RawAlbum {
                // The caller puts the requested id here; the page does not repeat it.
                id: "page".to_owned(),
                title: a.title.clone(),
                artists: names(&a.artists),
                year: Some(a.year.clone()),
                thumbnails: thumbs(&a.thumbnails),
            },
            tracks: a
                .tracks
                .iter()
                .map(|t| RawAlbumTrack {
                    video_id: t.video_id.get_raw().to_owned(),
                    title: t.title.clone(),
                    duration: Some(t.duration.clone()),
                })
                .collect(),
        }
    }

    pub fn artist_page(a: &GetArtist) -> RawArtistPage {
        let releases = &a.top_releases;
        let albums = releases
            .albums
            .iter()
            .chain(releases.singles.iter())
            .flat_map(|group| group.results.iter())
            .map(album_result)
            .collect();
        RawArtistPage {
            artist: RawArtist {
                id: "page".to_owned(),
                name: a.name.clone(),
                thumbnails: thumbs(&a.thumbnails),
            },
            description: a.description.clone(),
            songs: releases
                .songs
                .iter()
                .flat_map(|group| group.results.iter())
                .map(artist_song)
                .collect(),
            albums,
        }
    }

    /// Songs and music videos that are available. Episodes, uploads and greyed-out items are
    /// dropped.
    pub fn playlist_items(items: &[PlaylistItem]) -> Vec<RawSong> {
        items
            .iter()
            .filter_map(|item| match item {
                PlaylistItem::Song(s) if s.is_available => Some(RawSong {
                    video_id: s.video_id.get_raw().to_owned(),
                    title: s.title.clone(),
                    artists: names(&s.artists),
                    album: Some(s.album.name.clone()),
                    duration: Some(s.duration.clone()),
                    thumbnails: thumbs(&s.thumbnails),
                }),
                PlaylistItem::Video(v) if v.is_available => Some(RawSong {
                    video_id: v.video_id.get_raw().to_owned(),
                    title: v.title.clone(),
                    artists: vec![v.channel_name.clone()],
                    album: None,
                    duration: Some(v.duration.clone()),
                    thumbnails: thumbs(&v.thumbnails),
                }),
                _ => None,
            })
            .collect()
    }

    pub fn playlist_page(d: &GetPlaylistDetails, items: &[PlaylistItem]) -> RawPlaylistPage {
        RawPlaylistPage {
            playlist: RawPlaylist {
                id: "page".to_owned(),
                title: d.title.clone(),
                author: Some(d.author.clone()),
                thumbnails: thumbs(&d.thumbnails),
            },
            songs: playlist_items(items),
        }
    }
}

/// The four filtered search answers, ready to map.
pub(crate) struct SearchParts {
    pub(crate) songs: Vec<ytmapi_rs::parse::SearchResultSong>,
    pub(crate) albums: Vec<ytmapi_rs::parse::SearchResultAlbum>,
    pub(crate) artists: Vec<ytmapi_rs::parse::SearchResultArtist>,
    pub(crate) playlists: Vec<ytmapi_rs::parse::SearchResultPlaylist>,
}

/// Turn the search answers into core types.
pub(crate) fn map_search(parts: &SearchParts) -> SearchResults {
    SearchResults {
        songs: parts
            .songs
            .iter()
            .map(adapt::search_song)
            .filter_map(|s| map_song(&s))
            .collect(),
        albums: parts
            .albums
            .iter()
            .map(adapt::search_album)
            .filter_map(|a| map_album(&a))
            .collect(),
        artists: parts
            .artists
            .iter()
            .map(adapt::search_artist)
            .filter_map(|a| map_artist(&a))
            .collect(),
        playlists: parts
            .playlists
            .iter()
            .filter_map(adapt::search_playlist)
            .filter_map(|p| map_playlist(&p))
            .collect(),
    }
}
/// Fixed texts only: `ytmapi-rs` errors can carry response bodies.
pub(crate) fn map_error(error: ytmapi_rs::Error) -> CatalogError {
    match error.into_kind() {
        ErrorKind::Web { .. } | ErrorKind::Io(_) => {
            CatalogError::Network("request to YouTube Music failed".to_owned())
        }
        ErrorKind::NoVisitorData
        | ErrorKind::UnableToParseYtCfg { .. }
        | ErrorKind::InvalidUserAgent(_) => CatalogError::Unavailable(
            "YouTube Music changed or refused the anonymous request".to_owned(),
        ),
        _ => CatalogError::Parse("unexpected answer from YouTube Music".to_owned()),
    }
}

// ---------------------------------------------------------------------------------------------
// The catalog
// ---------------------------------------------------------------------------------------------

struct Inner {
    api: OnceCell<YtMusic<NoAuthToken>>,
}

impl Inner {
    async fn api(&self) -> Result<&YtMusic<NoAuthToken>, CatalogError> {
        self.api
            .get_or_try_init(|| async { YtMusic::new_unauthenticated().await })
            .await
            .map_err(map_error)
    }

    async fn search(&self, query: String) -> Result<SearchResults, CatalogError> {
        let api = self.api().await?;
        // The unfiltered search of ytmapi-rs is deprecated and returned nothing anonymously, so
        // each kind is asked for with its own filter. Songs are required; a failed album, artist
        // or playlist request only leaves that group empty.
        let (songs, albums, artists, playlists) = tokio::join!(
            api.search_songs(query.as_str()),
            api.search_albums(query.as_str()),
            api.search_artists(query.as_str()),
            api.search_playlists(query.as_str()),
        );
        Ok(map_search(&SearchParts {
            songs: songs.map_err(map_error)?,
            albums: albums.unwrap_or_default(),
            artists: artists.unwrap_or_default(),
            playlists: playlists.unwrap_or_default(),
        }))
    }

    async fn album(&self, id: BrowseId) -> Result<AlbumDetail, CatalogError> {
        let api = self.api().await?;
        if id.0.starts_with("MPRE")
            && let Ok(page) = api.get_album(AlbumID::from_raw(id.0.clone())).await
            && let Some(detail) = map_album_page(&adapt::album_page(&page), &id)
        {
            return Ok(detail);
        }
        // Playlist-style album ids (`OLAK5uy...`) and layouts the typed parser rejects.
        pages::album(api, &id).await
    }

    async fn artist(&self, id: BrowseId) -> Result<ArtistDetail, CatalogError> {
        let api = self.api().await?;
        let page = api
            .get_artist(ArtistChannelID::from_raw(id.0.clone()))
            .await
            .map_err(map_error)?;
        map_artist_page(&adapt::artist_page(&page), &id).ok_or(CatalogError::NotFound)
    }

    async fn playlist(&self, id: BrowseId) -> Result<PlaylistDetail, CatalogError> {
        let api = self.api().await?;
        // Raw page first: the typed parser of ytmapi-rs 0.3.4 rejects the current playlist
        // layout for every id kind (checked 2026-11-11: PL, OLAK5uy, RDCLAK), which was the
        // "unexpected answer" error on playlists opened from Home. It stays as the fallback.
        let raw = pages::playlist(api, &id).await;
        if raw.is_ok() {
            return raw;
        }
        let bare = id.0.strip_prefix("VL").unwrap_or(&id.0).to_owned();
        let typed = async {
            let details = api
                .get_playlist_details(PlaylistID::from_raw(bare.clone()))
                .await
                .map_err(map_error)?;
            let items = api
                .get_playlist_tracks(PlaylistID::from_raw(bare))
                .await
                .map_err(map_error)?;
            map_playlist_page(&adapt::playlist_page(&details, &items), &id)
                .ok_or(CatalogError::NotFound)
        };
        // Report the raw path's error: it names the real cause (network, not found).
        typed.await.or(raw)
    }
}

/// The anonymous YouTube Music catalog.
///
/// Work runs on the tokio runtime given at construction. The futures it returns only await
/// join handles, so any executor can poll them.
#[derive(Clone)]
pub struct YtmCatalog {
    inner: Arc<Inner>,
    handle: Handle,
}

impl std::fmt::Debug for YtmCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("YtmCatalog").finish_non_exhaustive()
    }
}

impl YtmCatalog {
    /// Create a catalog that runs its requests on `handle`. No request is made until first use.
    #[must_use]
    pub fn new(handle: Handle) -> Self {
        Self {
            inner: Arc::new(Inner {
                api: OnceCell::new(),
            }),
            handle,
        }
    }

    /// Development aid: describe the raw browse page of `browse_id`; shapes and counts only.
    pub async fn diagnose_browse(&self, browse_id: String) -> String {
        let inner = Arc::clone(&self.inner);
        self.run(async move {
            let api = inner.api().await?;
            Ok(crate::diagnostics::browse_report(api, &browse_id).await)
        })
        .await
        .unwrap_or_else(|e| format!("could not connect ({e})"))
    }

    fn run<T, F>(&self, work: F) -> BoxFuture<'static, Result<T, CatalogError>>
    where
        T: Send + 'static,
        F: Future<Output = Result<T, CatalogError>> + Send + 'static,
    {
        let task = self.handle.spawn(work);
        Box::pin(async move {
            task.await
                .unwrap_or_else(|_| Err(CatalogError::Network("request was cancelled".to_owned())))
        })
    }
}

fn checked(id: &BrowseId) -> Result<BrowseId, CatalogError> {
    if valid_browse_id(&id.0) {
        Ok(id.clone())
    } else {
        Err(CatalogError::NotFound)
    }
}

impl Catalog for YtmCatalog {
    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
        let text: String = query.trim().chars().take(MAX_QUERY_CHARS).collect();
        if text.is_empty() {
            return Box::pin(async { Ok(SearchResults::default()) });
        }
        let inner = Arc::clone(&self.inner);
        self.run(async move { inner.search(text).await })
    }

    fn album<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<AlbumDetail, CatalogError>> {
        match checked(id) {
            Ok(id) => {
                let inner = Arc::clone(&self.inner);
                self.run(async move { inner.album(id).await })
            }
            Err(e) => Box::pin(async move { Err(e) }),
        }
    }

    fn artist<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<ArtistDetail, CatalogError>> {
        match checked(id) {
            Ok(id) => {
                let inner = Arc::clone(&self.inner);
                self.run(async move { inner.artist(id).await })
            }
            Err(e) => Box::pin(async move { Err(e) }),
        }
    }

    fn playlist<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PlaylistDetail, CatalogError>> {
        match checked(id) {
            Ok(id) => {
                let inner = Arc::clone(&self.inner);
                self.run(async move { inner.playlist(id).await })
            }
            Err(e) => Box::pin(async move { Err(e) }),
        }
    }

    fn home_feed(&self) -> BoxFuture<'_, Result<HomeFeed, CatalogError>> {
        let inner = Arc::clone(&self.inner);
        self.run(async move { pages::home_feed(inner.api().await?).await })
    }

    fn explore(&self) -> BoxFuture<'_, Result<Explore, CatalogError>> {
        let inner = Arc::clone(&self.inner);
        self.run(async move { pages::explore(inner.api().await?).await })
    }

    fn mood_playlists<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<Vec<FeedShelf>, CatalogError>> {
        let inner = Arc::clone(&self.inner);
        let id = id.clone();
        self.run(async move { pages::mood_playlists(inner.api().await?, &id).await })
    }

    fn podcast<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PodcastDetail, CatalogError>> {
        let inner = Arc::clone(&self.inner);
        let id = id.clone();
        self.run(async move { pages::podcast(inner.api().await?, &id).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "abcdefghijk";

    fn thumb(url: &str, width: u32) -> Thumbnail {
        Thumbnail {
            url: url.to_owned(),
            width,
            height: width,
        }
    }

    fn song(id: &str, title: &str) -> RawSong {
        RawSong {
            video_id: id.to_owned(),
            title: title.to_owned(),
            artists: vec!["Artist One".to_owned(), "  ".to_owned()],
            album: Some(" Album X ".to_owned()),
            duration: Some("3:45".to_owned()),
            thumbnails: vec![thumb("https://img.invalid/a", 120)],
        }
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("3:45"), Some(Duration::from_secs(225)));
        assert_eq!(parse_duration("0:05"), Some(Duration::from_secs(5)));
        assert_eq!(parse_duration("1:02:03"), Some(Duration::from_secs(3723)));
        assert_eq!(parse_duration(" 10:00 "), Some(Duration::from_secs(600)));
        for bad in [
            "", "0:00", "45", "1:2:3:4", "a:bc", "3:", ":30", "1:234", "-1:20", "3:45 min",
        ] {
            assert_eq!(parse_duration(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn years() {
        assert_eq!(parse_year("2019"), Some(2019));
        assert_eq!(parse_year("Album • 2019"), Some(2019));
        assert_eq!(parse_year("1999-2003"), Some(1999));
        assert_eq!(parse_year("12345"), None);
        assert_eq!(parse_year("abc"), None);
        assert_eq!(parse_year(""), None);
    }

    #[test]
    fn thumbnails_drop_empty_urls_and_oversized_numbers() {
        let raw = vec![
            ("https://img.invalid/a".to_owned(), 60, 60),
            ("   ".to_owned(), 10, 10),
            (" https://img.invalid/b ".to_owned(), u64::MAX, 5),
        ];
        let mapped = map_thumbnails(&raw);
        assert_eq!(mapped.len(), 2);
        assert_eq!(mapped[0], thumb("https://img.invalid/a", 60));
        assert_eq!(mapped[1].url, "https://img.invalid/b");
        assert_eq!((mapped[1].width, mapped[1].height), (0, 5));
    }

    #[test]
    fn a_song_maps_to_a_youtube_music_track() {
        let track = map_song(&song(ID, "  A Title ")).expect("valid song");
        assert_eq!(track.id, TrackId(ID.to_owned()));
        assert_eq!(track.title, "A Title");
        assert_eq!(track.artists, vec!["Artist One".to_owned()]);
        assert_eq!(track.album.as_deref(), Some("Album X"));
        assert_eq!(track.duration, Some(Duration::from_secs(225)));
        assert_eq!(track.origin, Origin::YoutubeMusic);
        assert_eq!(track.video_id.as_ref().map(VideoId::as_str), Some(ID));
        assert!(track.local_path.is_none());
        assert_eq!(
            track.artwork,
            Artwork::Remote(vec![thumb("https://img.invalid/a", 120)])
        );
    }

    #[test]
    fn songs_without_a_usable_id_or_title_are_dropped() {
        for bad_id in [
            "",
            "short",
            "abcdefghijkl",
            "abc def ghi",
            "abcdefghij;",
            "abc\ndefghij",
        ] {
            assert_eq!(map_song(&song(bad_id, "T")), None, "{bad_id:?}");
        }
        assert_eq!(map_song(&song(ID, "   ")), None);
    }

    #[test]
    fn missing_optional_parts_stay_missing() {
        let raw = RawSong {
            video_id: ID.to_owned(),
            title: "T".to_owned(),
            ..RawSong::default()
        };
        let track = map_song(&raw).expect("valid");
        assert_eq!(track.artists.len(), 0);
        assert_eq!(track.album, None);
        assert_eq!(track.duration, None);
        assert_eq!(track.artwork, Artwork::None);
    }

    #[test]
    fn albums_artists_and_playlists() {
        let album = RawAlbum {
            id: "MPREb_x".to_owned(),
            title: "Album".to_owned(),
            artists: vec!["A".to_owned()],
            year: Some("Album • 2020".to_owned()),
            thumbnails: vec![thumb("https://img.invalid/c", 226)],
        };
        let mapped = map_album(&album).expect("valid");
        assert_eq!(mapped.id, BrowseId("MPREb_x".to_owned()));
        assert_eq!(mapped.year, Some(2020));
        assert_eq!(
            map_album(&RawAlbum {
                id: " ".to_owned(),
                ..album.clone()
            }),
            None
        );
        assert_eq!(
            map_album(&RawAlbum {
                title: String::new(),
                ..album
            }),
            None
        );

        let artist = RawArtist {
            id: "UCx".to_owned(),
            name: "Name".to_owned(),
            thumbnails: Vec::new(),
        };
        assert_eq!(map_artist(&artist).expect("valid").name, "Name");
        assert_eq!(
            map_artist(&RawArtist {
                name: String::new(),
                ..artist
            }),
            None
        );

        let playlist = RawPlaylist {
            id: "PLx".to_owned(),
            title: "List".to_owned(),
            author: Some("  ".to_owned()),
            thumbnails: Vec::new(),
        };
        let mapped = map_playlist(&playlist).expect("valid");
        assert_eq!(mapped.author, None);
        assert_eq!(
            map_playlist(&RawPlaylist {
                id: String::new(),
                ..playlist
            }),
            None
        );
    }

    #[test]
    fn album_page_tracks_inherit_artists_title_and_cover() {
        let page = RawAlbumPage {
            album: RawAlbum {
                id: "page".to_owned(),
                title: "The Album".to_owned(),
                artists: vec!["Band".to_owned()],
                year: Some("2001".to_owned()),
                thumbnails: vec![thumb("https://img.invalid/d", 544)],
            },
            tracks: vec![
                RawAlbumTrack {
                    video_id: ID.to_owned(),
                    title: "One".to_owned(),
                    duration: Some("2:00".to_owned()),
                },
                RawAlbumTrack {
                    video_id: "bad id".to_owned(),
                    title: "Two".to_owned(),
                    duration: None,
                },
                RawAlbumTrack {
                    video_id: "zzzzzzzzzzz".to_owned(),
                    title: "Three".to_owned(),
                    duration: Some("x".to_owned()),
                },
            ],
        };
        let requested = BrowseId("MPREb_real".to_owned());
        let detail = map_album_page(&page, &requested).expect("valid");
        assert_eq!(detail.summary.id, requested);
        assert_eq!(detail.summary.year, Some(2001));
        assert_eq!(detail.tracks.len(), 2);
        assert_eq!(detail.tracks[0].artists, vec!["Band".to_owned()]);
        assert_eq!(detail.tracks[0].album.as_deref(), Some("The Album"));
        assert_eq!(detail.tracks[0].duration, Some(Duration::from_secs(120)));
        assert_eq!(detail.tracks[1].duration, None);
        assert!(matches!(detail.tracks[1].artwork, Artwork::Remote(_)));
    }

    #[test]
    fn artist_and_playlist_pages_keep_the_requested_id() {
        let requested = BrowseId("UCreal".to_owned());
        let page = RawArtistPage {
            artist: RawArtist {
                id: "page".to_owned(),
                name: "Name".to_owned(),
                thumbnails: Vec::new(),
            },
            description: Some("  ".to_owned()),
            songs: vec![song(ID, "S"), song("bad", "S2")],
            albums: vec![RawAlbum {
                id: "MPREb_a".to_owned(),
                title: "A".to_owned(),
                ..RawAlbum::default()
            }],
        };
        let detail = map_artist_page(&page, &requested).expect("valid");
        assert_eq!(detail.summary.id, requested);
        assert_eq!(detail.description, None);
        assert_eq!(detail.top_songs.len(), 1);
        assert_eq!(detail.albums.len(), 1);

        let requested = BrowseId("VLPLreal".to_owned());
        let page = RawPlaylistPage {
            playlist: RawPlaylist {
                id: "page".to_owned(),
                title: "P".to_owned(),
                ..RawPlaylist::default()
            },
            songs: vec![song(ID, "S")],
        };
        let detail = map_playlist_page(&page, &requested).expect("valid");
        assert_eq!(detail.summary.id, requested);
        assert_eq!(detail.tracks.len(), 1);
        // A page without a title is "not found".
        assert!(map_playlist_page(&RawPlaylistPage::default(), &requested).is_none());
    }

    #[test]
    fn browse_ids_are_checked() {
        for ok in ["MPREb_AbC-123", "UCabc", "VLPLxyz", "RDCLAK5uy_k"] {
            assert!(valid_browse_id(ok), "{ok}");
        }
        for bad in [
            "",
            "a b",
            "a/b",
            "a\"b",
            "x\ny",
            "é",
            &"a".repeat(MAX_ID_BYTES + 1),
        ] {
            assert!(!valid_browse_id(bad), "{bad:?}");
        }
    }

    /// Search answers are built from JSON, because `ytmapi-rs` result types cannot be built by
    /// hand. The shapes below are synthetic.
    #[test]
    fn search_answer_is_mapped_and_podcasts_are_dropped() {
        let songs = r#"[
            {"title":"Song A","artist":"Band","album":{"name":"Rec","id":"MPREb_1"},
             "duration":"4:01","plays":"1M","explicit":"NotExplicit","video_id":"abcdefghijk",
             "thumbnails":[{"height":60,"width":60,"url":"https://img.invalid/s"}]},
            {"title":"Bad id","artist":"Band","album":null,"duration":"1:00","plays":"1",
             "explicit":"NotExplicit","video_id":"no way","thumbnails":[]}
        ]"#;
        let albums = r#"[
            {"title":"Rec","artist":"Band","year":"2018","explicit":"NotExplicit",
             "album_id":"MPREb_1","album_type":"Album","thumbnails":[]}
        ]"#;
        let artists = r#"[
            {"artist":"Band","subscribers":null,"browse_id":"UCband","thumbnails":[]}
        ]"#;
        let playlists = r#"[
            {"Featured":{"title":"Mix","author":"YouTube Music","songs":"50 songs",
             "playlist_id":"RDCLAK1","thumbnails":[]}},
            {"Community":{"title":"Fan list","author":"Someone","views":"10 views",
             "playlist_id":"PLfan","thumbnails":[]}},
            {"Podcast":{"title":"A show","publisher":"Host","podcast_id":"MPSPPLshow",
             "thumbnails":[]}}
        ]"#;
        let parts = SearchParts {
            songs: serde_json::from_str(songs).expect("synthetic songs parse"),
            albums: serde_json::from_str(albums).expect("synthetic albums parse"),
            artists: serde_json::from_str(artists).expect("synthetic artists parse"),
            playlists: serde_json::from_str(playlists).expect("synthetic playlists parse"),
        };
        let mapped = map_search(&parts);
        assert_eq!(mapped.songs.len(), 1);
        assert_eq!(mapped.songs[0].title, "Song A");
        assert_eq!(mapped.songs[0].album.as_deref(), Some("Rec"));
        assert_eq!(mapped.songs[0].duration, Some(Duration::from_secs(241)));
        assert_eq!(mapped.albums.len(), 1);
        assert_eq!(mapped.albums[0].year, Some(2018));
        assert_eq!(mapped.artists.len(), 1);
        assert_eq!(mapped.artists[0].id, BrowseId("UCband".to_owned()));
        let titles: Vec<&str> = mapped.playlists.iter().map(|p| p.title.as_str()).collect();
        assert_eq!(titles, vec!["Mix", "Fan list"]);
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("runtime builds")
    }

    #[test]
    fn empty_queries_and_bad_ids_make_no_request() {
        // Nothing here can reach the network: all four calls answer before any request.
        let rt = runtime();
        let catalog = YtmCatalog::new(rt.handle().clone());
        let empty = rt.block_on(catalog.search("   ")).expect("empty is fine");
        assert_eq!(empty, SearchResults::default());
        let bad = BrowseId("a b/c".to_owned());
        assert_eq!(
            rt.block_on(catalog.album(&bad)).unwrap_err(),
            CatalogError::NotFound
        );
        assert_eq!(
            rt.block_on(catalog.artist(&bad)).unwrap_err(),
            CatalogError::NotFound
        );
        assert_eq!(
            rt.block_on(catalog.playlist(&bad)).unwrap_err(),
            CatalogError::NotFound
        );
        assert!(matches!(
            rt.block_on(catalog.home()),
            Err(CatalogError::Unavailable(_))
        ));
    }

    /// Anonymous `ytmapi-rs` search against the real service. Run by hand, at most twice.
    #[test]
    #[ignore = "needs network access to music.youtube.com"]
    fn real_anonymous_search_returns_songs() {
        let rt = runtime();
        let catalog = YtmCatalog::new(rt.handle().clone());
        let results = rt
            .block_on(catalog.search("piano"))
            .expect("anonymous search works");
        assert_ne!(results.songs.len(), 0);
        assert!(results.songs.iter().all(|t| t.video_id.is_some()));
    }
}
