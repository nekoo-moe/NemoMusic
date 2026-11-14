//! Search over all of YouTube (youtube-extras): the music search, then one web search.
//!
//! [`WebCatalogExt`] wraps the music catalog. For [`SearchScope::AllOfYoutube`] it runs the
//! music search first and, only after it answered, one `POST /youtubei/v1/search` to
//! `www.youtube.com` with the `WEB` client and no cookies (a host already on the README egress
//! list). The answer's `videoRenderer` items become videos (a `BADGE_STYLE_TYPE_LIVE_NOW` badge
//! marks a live stream), `officialCardViewModel` cards become channels, and playlist
//! `lockupViewModel` items join the playlists (shapes from the youtube-extras spike, task 7).
//! [`SearchScope::Music`] and every other call go to the music catalog unchanged.
//!
//! Human-paced: one web request per user search and no retry. A 429 stops all-YouTube search
//! for its `Retry-After` (default [`DEFAULT_BACKOFF`]); in that time it answers
//! [`CatalogError::RateLimited`] without a request. Access is unofficial and may breach
//! YouTube's Terms of Service (`coc.md`). No URL or query is logged.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use nemo_core::BoxFuture;
use nemo_core::catalog::{
    AlbumDetail, AlbumSummary, ArtistDetail, ArtistSummary, BrowseId, Catalog, CatalogError,
    ChannelDetail, Explore, FeedShelf, HistorySection, HomeFeed, LikeStatus, PlaylistDetail,
    PlaylistSummary, PodcastDetail, SearchResults, SearchScope, Shelf, VideoSummary,
};
use nemo_core::track::{Track, VideoId};
use serde_json::json;

use crate::http::{InitError, https_redirects, parse_retry_after};

mod parse;

pub use parse::{WebResults, parse_search};

/// Endpoint of the web search.
pub const SEARCH_URL: &str = "https://www.youtube.com/youtubei/v1/search?prettyPrint=false";
/// `clientVersion` of the `WEB` client, as captured in the spike fixtures.
pub const WEB_CLIENT_VERSION: &str = "2.20261112.10.00";
/// `X-YouTube-Client-Name` of the `WEB` client.
const WEB_CLIENT_ID: &str = "1";
/// `Origin` of the requests.
const ORIGIN: &str = "https://www.youtube.com";
/// `User-Agent` of a desktop browser, as the web client sends.
const WEB_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36";
/// Total time one web search may take.
pub const WEB_TIMEOUT: Duration = Duration::from_secs(8);
/// How long a 429 without a usable `Retry-After` stops all-YouTube search.
pub const DEFAULT_BACKOFF: Duration = Duration::from_mins(10);
/// Longest answer read, in bytes. Real answers are a few hundred KiB.
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
/// Longest query sent, in characters (the music catalog uses the same cap).
const MAX_QUERY_CHARS: usize = 200;

/// A finished web answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebAnswer {
    /// HTTP status code.
    pub status: u16,
    /// Body as text.
    pub body: String,
    /// `Retry-After` in seconds, if the server sent one.
    pub retry_after: Option<Duration>,
}

/// One innertube `POST`, so tests can inject a fake. [`ReqwestWeb`] is the real one.
pub trait WebPost: Send + Sync {
    /// POST the JSON `body` to `url`. The error text never contains the URL.
    fn post<'a>(&'a self, url: &'a str, body: String) -> BoxFuture<'a, Result<WebAnswer, String>>;
}

/// [`WebPost`] over `reqwest`, run on the app's tokio runtime: HTTPS only (also across
/// redirects), [`WEB_TIMEOUT`], no cookie store, the `WEB` client headers.
#[derive(Debug, Clone)]
pub struct ReqwestWeb {
    client: reqwest::Client,
    handle: tokio::runtime::Handle,
}

impl ReqwestWeb {
    /// Build the client. `handle` is the runtime that performs the requests.
    ///
    /// # Errors
    /// [`InitError::Client`] when the TLS backend cannot start.
    pub fn new(handle: tokio::runtime::Handle) -> Result<Self, InitError> {
        Self::build(handle, true)
    }

    fn build(handle: tokio::runtime::Handle, https_only: bool) -> Result<Self, InitError> {
        let client = reqwest::Client::builder()
            .user_agent(WEB_USER_AGENT)
            .timeout(WEB_TIMEOUT)
            .redirect(https_redirects())
            .https_only(https_only)
            .build()
            .map_err(|_| InitError::Client)?;
        Ok(Self { client, handle })
    }
}

async fn send(client: reqwest::Client, url: String, body: String) -> Result<WebAnswer, String> {
    let mut response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("X-YouTube-Client-Name", WEB_CLIENT_ID)
        .header("X-YouTube-Client-Version", WEB_CLIENT_VERSION)
        .header("Origin", ORIGIN)
        .body(body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "web search timed out".to_owned()
            } else {
                "web search failed".to_owned()
            }
        })?;
    let status = response.status().as_u16();
    let retry_after = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_retry_after);
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "the web search answer was cut off".to_owned())?
    {
        if bytes.len() + chunk.len() > MAX_BODY_BYTES {
            return Err("the web search answer is too large".to_owned());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(WebAnswer {
        status,
        body: String::from_utf8_lossy(&bytes).into_owned(),
        retry_after,
    })
}

impl WebPost for ReqwestWeb {
    fn post<'a>(&'a self, url: &'a str, body: String) -> BoxFuture<'a, Result<WebAnswer, String>> {
        let task = self
            .handle
            .spawn(send(self.client.clone(), url.to_owned(), body));
        Box::pin(async move {
            task.await
                .unwrap_or_else(|_| Err("web search was cancelled".to_owned()))
        })
    }
}

/// The JSON body of a web search for `query`: the `WEB` client context and no `params`.
#[must_use]
pub fn search_body(query: &str) -> String {
    json!({
        "context": { "client": {
            "clientName": "WEB",
            "clientVersion": WEB_CLIENT_VERSION,
            "hl": "en",
            "gl": "US",
        } },
        "query": query,
    })
    .to_string()
}

/// The music catalog with all-YouTube search added. Built only when that setting is on.
pub struct WebCatalogExt<P: WebPost = ReqwestWeb> {
    music: Arc<dyn Catalog>,
    web: P,
    /// Until when a 429 stops web searches.
    blocked_until: Mutex<Option<Instant>>,
}

impl<P: WebPost> std::fmt::Debug for WebCatalogExt<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebCatalogExt").finish_non_exhaustive()
    }
}

impl<P: WebPost> WebCatalogExt<P> {
    /// Wrap `music`; web searches go through `web`.
    #[must_use]
    pub fn new(music: Arc<dyn Catalog>, web: P) -> Self {
        Self {
            music,
            web,
            blocked_until: Mutex::new(None),
        }
    }

    fn blocked(&self) -> bool {
        let until = self
            .blocked_until
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        until.is_some_and(|at| Instant::now() < at)
    }

    fn back_off(&self, wait: Option<Duration>) {
        let wait = wait.unwrap_or(DEFAULT_BACKOFF);
        *self
            .blocked_until
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Instant::now().checked_add(wait);
        tracing::warn!(
            wait_secs = wait.as_secs(),
            "YouTube asked to slow down; all-YouTube search paused"
        );
    }

    /// One web search, after the music search answered.
    async fn web_search(&self, query: &str) -> Result<WebResults, CatalogError> {
        let answer = self
            .web
            .post(SEARCH_URL, search_body(query))
            .await
            .map_err(CatalogError::Network)?;
        match answer.status {
            200 => parse_search(&answer.body),
            429 => {
                self.back_off(answer.retry_after);
                Err(CatalogError::RateLimited)
            }
            other => Err(CatalogError::Network(format!("web search: HTTP {other}"))),
        }
    }

    async fn all_of_youtube(&self, query: &str) -> Result<SearchResults, CatalogError> {
        let text: String = query.trim().chars().take(MAX_QUERY_CHARS).collect();
        if text.is_empty() {
            return Ok(SearchResults::default());
        }
        if self.blocked() {
            return Err(CatalogError::RateLimited);
        }
        let mut results = self.music.search(&text).await?;
        let web = self.web_search(&text).await?;
        results.videos = web.videos;
        results.channels = web.channels;
        for playlist in web.playlists {
            if !results.playlists.iter().any(|p| p.id == playlist.id) {
                results.playlists.push(playlist);
            }
        }
        Ok(results)
    }
}

impl<P: WebPost> Catalog for WebCatalogExt<P> {
    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
        self.music.search(query)
    }

    fn search_scoped<'a>(
        &'a self,
        query: &'a str,
        scope: SearchScope,
    ) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
        match scope {
            SearchScope::Music => self.music.search(query),
            SearchScope::AllOfYoutube => Box::pin(self.all_of_youtube(query)),
        }
    }

    fn album<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<AlbumDetail, CatalogError>> {
        self.music.album(id)
    }

    fn artist<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<ArtistDetail, CatalogError>> {
        self.music.artist(id)
    }

    fn playlist<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PlaylistDetail, CatalogError>> {
        self.music.playlist(id)
    }

    fn home(&self) -> BoxFuture<'_, Result<Vec<Shelf>, CatalogError>> {
        self.music.home()
    }

    fn home_feed(&self) -> BoxFuture<'_, Result<HomeFeed, CatalogError>> {
        self.music.home_feed()
    }

    fn library_playlists(&self) -> BoxFuture<'_, Result<Vec<PlaylistSummary>, CatalogError>> {
        self.music.library_playlists()
    }

    fn liked_songs(&self) -> BoxFuture<'_, Result<Vec<Track>, CatalogError>> {
        self.music.liked_songs()
    }

    fn library_albums(&self) -> BoxFuture<'_, Result<Vec<AlbumSummary>, CatalogError>> {
        self.music.library_albums()
    }

    fn library_artists(&self) -> BoxFuture<'_, Result<Vec<ArtistSummary>, CatalogError>> {
        self.music.library_artists()
    }

    fn history(&self) -> BoxFuture<'_, Result<Vec<HistorySection>, CatalogError>> {
        self.music.history()
    }

    fn explore(&self) -> BoxFuture<'_, Result<Explore, CatalogError>> {
        self.music.explore()
    }

    fn mood_playlists<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<Vec<FeedShelf>, CatalogError>> {
        self.music.mood_playlists(id)
    }

    fn podcast<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PodcastDetail, CatalogError>> {
        self.music.podcast(id)
    }

    fn set_rating<'a>(
        &'a self,
        video_id: &'a VideoId,
        status: LikeStatus,
    ) -> BoxFuture<'a, Result<(), CatalogError>> {
        self.music.set_rating(video_id, status)
    }

    fn rate_song<'a>(
        &'a self,
        video_id: &'a VideoId,
        liked: bool,
    ) -> BoxFuture<'a, Result<(), CatalogError>> {
        self.music.rate_song(video_id, liked)
    }

    fn channel<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<ChannelDetail, CatalogError>> {
        self.music.channel(id)
    }

    fn live_streams(&self) -> BoxFuture<'_, Result<Vec<VideoSummary>, CatalogError>> {
        self.music.live_streams()
    }
}

#[cfg(test)]
mod tests;
