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

use nemo_core::BoxFuture;
use nemo_core::catalog::{
    AlbumDetail, ArtistDetail, BrowseId, Catalog, CatalogError, Explore, FeedShelf, HomeFeed,
    PlaylistDetail, PodcastDetail, SearchResults,
};
use nemo_core::track::Thumbnail;
#[cfg(doc)]
use nemo_core::track::VideoId;
use tokio::runtime::Handle;
use tokio::sync::OnceCell;
use ytmapi_rs::YtMusic;
use ytmapi_rs::auth::noauth::NoAuthToken;
use ytmapi_rs::common::{AlbumID, ArtistChannelID, PlaylistID, YoutubeID};
use ytmapi_rs::error::ErrorKind;

use crate::pages;

pub(crate) mod adapt;
mod map;
mod raw;

pub(crate) use map::{
    SearchParts, map_album, map_album_page, map_artist, map_artist_page, map_playlist,
    map_playlist_page, map_search, map_song, map_thumbnails, parse_duration, parse_year,
    valid_browse_id,
};
pub(crate) use raw::{
    RawAlbum, RawAlbumPage, RawAlbumTrack, RawArtist, RawArtistPage, RawPlaylist, RawPlaylistPage,
    RawSong,
};

/// Longest search text sent, in characters.
const MAX_QUERY_CHARS: usize = 200;
/// Longest browse or playlist id accepted, in bytes.
const MAX_ID_BYTES: usize = 128;

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
mod tests;
