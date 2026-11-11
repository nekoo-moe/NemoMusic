//! A signed-in YouTube Music session on top of `ytmapi-rs` 0.3.4 browser auth.
//!
//! [`SessionBackend`] is the seam between the account logic and the network, so the account
//! tests can run with a fake. [`YtmSessionBackend`] is the real one. Errors carry fixed texts
//! only: `ytmapi-rs` errors can hold response bodies, and the cookie must never leave this module.

use std::future::Future;
use std::sync::Arc;

use nemo_core::BoxFuture;
use nemo_core::account::{AccountError, AccountInfo};
use nemo_core::catalog::{
    AlbumDetail, AlbumSummary, ArtistDetail, ArtistSummary, BrowseId, Catalog, CatalogError,
    Explore, FeedShelf, HistorySection, HomeFeed, PlaylistDetail, PlaylistSummary, PodcastDetail,
    SearchResults, Shelf, ShelfItem,
};
use nemo_core::secret::Secret;
use nemo_core::track::Track;
use tokio::runtime::Handle;
use tokio::sync::OnceCell;
use ytmapi_rs::YtMusic;
use ytmapi_rs::YtMusicBuilder;
use ytmapi_rs::auth::BrowserToken;
use ytmapi_rs::common::{AlbumID, ArtistChannelID, PlaylistID, YoutubeID};
use ytmapi_rs::error::ErrorKind;

use crate::catalog::{
    SearchParts, adapt, map_album_page, map_artist_page, map_error, map_playlist_page, map_search,
    valid_browse_id,
};
use crate::home_feed::{AccountMenuQuery, legacy_shelves, parse_account};
use crate::pages;

/// Longest search text sent, in characters.
const MAX_QUERY_CHARS: usize = 200;
/// Most liked songs shown on the home page.
const MAX_LIKED: usize = 50;

/// Turn what the user pasted, or what the sign-in window returned, into one cookie header.
///
/// Accepts a leading `Cookie:`, line breaks between cookies, and a missing trailing `;`
/// (`ytmapi-rs` needs one after `SAPISID`). Returns `None` when there is no `SAPISID` cookie.
#[must_use]
pub fn normalize_cookie(raw: &str) -> Option<String> {
    let text = raw.trim();
    let text = match text.get(..7) {
        Some(head) if head.eq_ignore_ascii_case("cookie:") => text[7..].trim(),
        _ => text,
    };
    let joined = text
        .lines()
        .map(|l| l.trim().trim_end_matches(';').trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("; ");
    let joined = joined.trim_end_matches(';').trim().to_owned();
    let has_session = joined.split(';').any(|c| {
        c.trim()
            .strip_prefix("SAPISID=")
            .is_some_and(|v| !v.is_empty())
    });
    (has_session && !joined.chars().any(char::is_control)).then(|| format!("{joined};"))
}

/// The network side of an account: check a cookie, and build the catalog that uses it.
pub trait SessionBackend: Send + Sync {
    /// Check that YouTube Music accepts the cookie and read the account header.
    fn probe(&self, cookie: Secret<String>) -> BoxFuture<'_, Result<AccountInfo, AccountError>>;

    /// A catalog that sends its requests with this session. Makes no request itself.
    fn catalog(&self, cookie: Secret<String>) -> Arc<dyn Catalog>;
}

/// The real backend.
pub struct YtmSessionBackend {
    handle: Handle,
}

impl YtmSessionBackend {
    /// Run requests on `handle`.
    #[must_use]
    pub fn new(handle: Handle) -> Self {
        Self { handle }
    }
}

fn account_error(error: ytmapi_rs::Error) -> AccountError {
    match error.into_kind() {
        ErrorKind::Web { .. } | ErrorKind::Io(_) => {
            AccountError::Network("request to YouTube Music failed".to_owned())
        }
        _ => AccountError::Rejected,
    }
}

async fn connect(cookie: String) -> ytmapi_rs::Result<YtMusic<BrowserToken>> {
    YtMusicBuilder::new_native_tls()
        .with_browser_token_cookie(cookie)
        .build()
        .await
}

async fn probe_session(cookie: String) -> Result<AccountInfo, AccountError> {
    let api = connect(cookie).await.map_err(account_error)?;
    let answer = api
        .raw_json_query::<AccountMenuQuery>(&AccountMenuQuery)
        .await
        .map_err(account_error)?;
    if let Some(info) = parse_account(&answer) {
        return Ok(info);
    }
    // The header layout may change; a library request still proves the session works.
    api.get_library_playlists().await.map_err(account_error)?;
    Ok(AccountInfo {
        name: "YouTube Music account".to_owned(),
        handle: None,
        avatar: Vec::new(),
    })
}

impl SessionBackend for YtmSessionBackend {
    fn probe(&self, cookie: Secret<String>) -> BoxFuture<'_, Result<AccountInfo, AccountError>> {
        let task = self.handle.spawn(probe_session(cookie.expose().clone()));
        Box::pin(async move {
            task.await
                .unwrap_or_else(|_| Err(AccountError::Network("request was cancelled".to_owned())))
        })
    }

    fn catalog(&self, cookie: Secret<String>) -> Arc<dyn Catalog> {
        Arc::new(SessionCatalog::new(self.handle.clone(), cookie))
    }
}

// ---------------------------------------------------------------------------------------------
// The signed-in catalog
// ---------------------------------------------------------------------------------------------

struct Inner {
    cookie: Secret<String>,
    api: OnceCell<YtMusic<BrowserToken>>,
}

impl Inner {
    async fn api(&self) -> Result<&YtMusic<BrowserToken>, CatalogError> {
        self.api
            .get_or_try_init(|| connect(self.cookie.expose().clone()))
            .await
            .map_err(map_error)
    }

    async fn search(&self, query: String) -> Result<SearchResults, CatalogError> {
        let api = self.api().await?;
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
        let page = api
            .get_album(AlbumID::from_raw(id.0.clone()))
            .await
            .map_err(map_error)?;
        map_album_page(&adapt::album_page(&page), &id).ok_or(CatalogError::NotFound)
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
        // Raw page first: it opens private playlists and the liked-music list. The typed
        // parser of ytmapi-rs stays as the fallback for layouts the raw reader does not know.
        if let Ok(detail) = pages::playlist(api, &id).await {
            return Ok(detail);
        }
        let bare = id.0.strip_prefix("VL").unwrap_or(&id.0).to_owned();
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
    }

    /// Like or un-like a song with this account.
    async fn rate_song(&self, video_id: String, liked: bool) -> Result<(), CatalogError> {
        let api = self.api().await?;
        let status = if liked {
            ytmapi_rs::common::LikeStatus::Liked
        } else {
            ytmapi_rs::common::LikeStatus::Indifferent
        };
        api.rate_song(ytmapi_rs::common::VideoID::from_raw(video_id), status)
            .await
            .map_err(map_error)
    }

    /// The old flat home contract: the home feed, then the account's playlists and liked songs.
    /// Only the feed or one library list must work; the rest is added when it loads.
    async fn home(&self) -> Result<Vec<Shelf>, CatalogError> {
        let api = self.api().await?;
        let feed = pages::home_feed(api).await;
        let mut shelves = feed.as_ref().map(legacy_shelves).unwrap_or_default();
        let playlists = pages::library_playlists(api, 1).await;
        let liked = pages::liked_songs(api, 1).await;
        let mine: Vec<ShelfItem> = playlists
            .as_ref()
            .map(|list| list.iter().cloned().map(ShelfItem::Playlist).collect())
            .unwrap_or_default();
        if !mine.is_empty() {
            shelves.push(Shelf {
                title: "Your playlists".to_owned(),
                items: mine,
            });
        }
        let songs: Vec<ShelfItem> = liked
            .as_ref()
            .map(|tracks| {
                tracks
                    .iter()
                    .take(MAX_LIKED)
                    .cloned()
                    .map(ShelfItem::Song)
                    .collect()
            })
            .unwrap_or_default();
        if !songs.is_empty() {
            shelves.push(Shelf {
                title: "Liked songs".to_owned(),
                items: songs,
            });
        }
        if shelves.is_empty() {
            // Nothing loaded: report why instead of an empty home.
            return feed
                .map(|_| Vec::new())
                .or_else(|e| playlists.map(|_| Vec::new()).map_err(|_| e));
        }
        Ok(shelves)
    }
}

/// The catalog of a signed-in session. Requests start on first use; the YouTube Music client is
/// built then and kept.
pub struct SessionCatalog {
    inner: Arc<Inner>,
    handle: Handle,
}

impl SessionCatalog {
    /// Create a catalog that signs its requests with `cookie` and runs them on `handle`.
    #[must_use]
    pub fn new(handle: Handle, cookie: Secret<String>) -> Self {
        Self {
            inner: Arc::new(Inner {
                cookie,
                api: OnceCell::new(),
            }),
            handle,
        }
    }

    /// Development aid for `tests/diagnose_account.rs`: asks the pages through the catalog layer
    /// and returns counts and error labels only (never titles, ids or URLs).
    pub async fn diagnose(&self) -> crate::diagnostics::Diagnosis {
        let inner = Arc::clone(&self.inner);
        self.run(async move {
            let api = inner.api().await?;
            Ok(crate::diagnostics::account_report(api).await)
        })
        .await
        .unwrap_or_else(|_| crate::diagnostics::Diagnosis::failed("could not connect"))
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

impl Catalog for SessionCatalog {
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

    fn home(&self) -> BoxFuture<'_, Result<Vec<Shelf>, CatalogError>> {
        let inner = Arc::clone(&self.inner);
        self.run(async move { inner.home().await })
    }

    fn home_feed(&self) -> BoxFuture<'_, Result<HomeFeed, CatalogError>> {
        let inner = Arc::clone(&self.inner);
        self.run(async move { pages::home_feed(inner.api().await?).await })
    }

    fn library_playlists(&self) -> BoxFuture<'_, Result<Vec<PlaylistSummary>, CatalogError>> {
        let inner = Arc::clone(&self.inner);
        self.run(
            async move { pages::library_playlists(inner.api().await?, pages::LIST_PAGES).await },
        )
    }

    fn liked_songs(&self) -> BoxFuture<'_, Result<Vec<Track>, CatalogError>> {
        let inner = Arc::clone(&self.inner);
        self.run(async move { pages::liked_songs(inner.api().await?, pages::PLAYLIST_PAGES).await })
    }

    fn library_albums(&self) -> BoxFuture<'_, Result<Vec<AlbumSummary>, CatalogError>> {
        let inner = Arc::clone(&self.inner);
        self.run(async move { pages::library_albums(inner.api().await?).await })
    }

    fn library_artists(&self) -> BoxFuture<'_, Result<Vec<ArtistSummary>, CatalogError>> {
        let inner = Arc::clone(&self.inner);
        self.run(async move { pages::library_artists(inner.api().await?).await })
    }

    fn history(&self) -> BoxFuture<'_, Result<Vec<HistorySection>, CatalogError>> {
        let inner = Arc::clone(&self.inner);
        self.run(async move { pages::history(inner.api().await?).await })
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

    fn rate_song<'a>(
        &'a self,
        video_id: &'a nemo_core::track::VideoId,
        liked: bool,
    ) -> BoxFuture<'a, Result<(), CatalogError>> {
        let inner = Arc::clone(&self.inner);
        let id = video_id.as_str().to_owned();
        self.run(async move { inner.rate_song(id, liked).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookie_gets_a_trailing_semicolon() {
        assert_eq!(
            normalize_cookie("a=1; SAPISID=x").as_deref(),
            Some("a=1; SAPISID=x;")
        );
        assert_eq!(
            normalize_cookie("a=1; SAPISID=x;;").as_deref(),
            Some("a=1; SAPISID=x;")
        );
    }

    #[test]
    fn cookie_prefix_and_line_breaks_are_accepted() {
        assert_eq!(
            normalize_cookie("Cookie: a=1;\r\nSAPISID=x\r\n").as_deref(),
            Some("a=1; SAPISID=x;")
        );
    }

    #[test]
    fn cookie_without_a_session_is_refused() {
        for bad in ["", "   ", "a=1; b=2", "SAPISID=", "x\u{0}=1; SAPISID=y"] {
            assert_eq!(normalize_cookie(bad), None, "{bad:?}");
        }
        // `__Secure-3PAPISID` is not the session cookie.
        assert_eq!(normalize_cookie("__Secure-3PAPISID=x"), None);
    }
}
