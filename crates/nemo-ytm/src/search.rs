//! YouTube Music's own search, as the website asks it (search-like-ytm).
//!
//! One `POST youtubei/v1/search` through the `ytmapi-rs` client the catalog already holds
//! (anonymous or signed in, so the session is used the same way as for every other signed-in
//! call). [`search`] sends only the query, like the search box: the answer keeps YouTube Music's
//! ranking, a top-result card (`musicCardShelfRenderer`) and then the results in YouTube's
//! order. [`search_kind`] adds the `params` of one filter chip (Songs, Videos, Albums, Artists,
//! Community playlists), as the chips of music.youtube.com send them; that answer is one shelf
//! of that kind. [`parse_search`] reads both.
//!
//! When an answer cannot be read (an unknown layout) or the request is refused, the filtered
//! searches of `ytmapi-rs` run instead, as before. A network error is returned as it is: the
//! same network would fail the fallback too, and no request is retried.

use std::borrow::Cow;

use nemo_core::catalog::{CatalogError, SearchKind, SearchResults};
use serde_json::{Map, Value, json};
use ytmapi_rs::YtMusic;
use ytmapi_rs::auth::AuthToken;
use ytmapi_rs::parse::{ParseFrom, ProcessedResult};
use ytmapi_rs::query::{PostMethod, PostQuery, Query};

use crate::catalog::{SearchParts, map_error, map_search};
use crate::raw::{Unparsed, catalog_error};

mod parse;

pub(crate) use parse::parse_search;

/// The `params` of YouTube Music's filter chips, copied from the `chipCloudRenderer` of a live
/// answer on 2026-11-15 (`tests/fixtures/music/`). `None`: YouTube Music has no chip for it.
#[must_use]
pub(crate) fn filter_params(kind: SearchKind) -> Option<&'static str> {
    match kind {
        SearchKind::Songs => Some("EgWKAQIIAWoQEAkQAxAFEAQQChAVEBAQEQ%3D%3D"),
        SearchKind::Videos => Some("EgWKAQIQAWoQEAkQAxAFEAQQChAVEBAQEQ%3D%3D"),
        SearchKind::Albums => Some("EgWKAQIYAWoQEAkQAxAFEAQQChAVEBAQEQ%3D%3D"),
        SearchKind::Artists => Some("EgWKAQIgAWoQEAkQAxAFEAQQChAVEBAQEQ%3D%3D"),
        // "Community playlists".
        SearchKind::Playlists => Some("EgeKAQQoAEABahAQCRADEAUQBBAKEBUQEBAR"),
        SearchKind::Channels => None,
    }
}

/// A `search` request: the query, and the `params` of a filter chip when one is picked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SearchQuery {
    query: String,
    params: Option<&'static str>,
}

impl SearchQuery {
    /// The unfiltered search for `query` (already trimmed and capped by the caller).
    pub(crate) fn new(query: &str) -> Self {
        Self {
            query: query.to_owned(),
            params: None,
        }
    }

    /// The search for `query` narrowed with a chip's `params` ([`filter_params`]).
    pub(crate) fn filtered(query: &str, params: &'static str) -> Self {
        Self {
            query: query.to_owned(),
            params: Some(params),
        }
    }
}

impl ParseFrom<SearchQuery> for Unparsed {
    fn parse_from(_: ProcessedResult<SearchQuery>) -> ytmapi_rs::Result<Self> {
        Ok(Self)
    }
}

impl<A: AuthToken> Query<A> for SearchQuery {
    type Output = Unparsed;
    type Method = PostMethod;
}

impl PostQuery for SearchQuery {
    fn header(&self) -> Map<String, Value> {
        let mut map = Map::new();
        map.insert("query".to_owned(), json!(self.query));
        if let Some(params) = self.params {
            map.insert("params".to_owned(), json!(params));
        }
        map
    }

    fn params(&self) -> Vec<(&str, Cow<'_, str>)> {
        Vec::new()
    }

    fn path(&self) -> &'static str {
        "search"
    }
}

/// Ask `query`; `Ok(None)` when the answer is unusable and a fallback may run.
async fn ask<A: AuthToken>(
    api: &YtMusic<A>,
    query: &SearchQuery,
) -> Result<Option<SearchResults>, CatalogError> {
    match api
        .raw_json_query::<SearchQuery>(query)
        .await
        .map_err(catalog_error)
    {
        Ok(json) => {
            let parsed = parse_search(&json);
            if parsed.is_none() {
                tracing::warn!("search answer layout not recognised; using the filtered searches");
            }
            Ok(parsed)
        }
        Err(e @ CatalogError::Network(_)) => Err(e),
        Err(_) => {
            tracing::warn!("search refused; using the filtered searches");
            Ok(None)
        }
    }
}

/// Search like YouTube Music does; the filtered searches only when that answer is unusable.
pub(crate) async fn search<A: AuthToken>(
    api: &YtMusic<A>,
    query: &str,
) -> Result<SearchResults, CatalogError> {
    match ask(api, &SearchQuery::new(query)).await? {
        Some(results) => Ok(results),
        None => filtered(api, query).await,
    }
}

/// One kind only, as YouTube Music's filter chip of that kind shows it. The answer keeps only
/// that kind's list, with no top result.
pub(crate) async fn search_kind<A: AuthToken>(
    api: &YtMusic<A>,
    query: &str,
    kind: SearchKind,
) -> Result<SearchResults, CatalogError> {
    let Some(params) = filter_params(kind) else {
        return Err(CatalogError::Unavailable("search filter".to_owned()));
    };
    let results = match ask(api, &SearchQuery::filtered(query, params)).await? {
        Some(results) => results,
        None => typed_kind(api, query, kind).await?,
    };
    Ok(only(results, kind))
}

/// Keep the list of `kind` only.
fn only(results: SearchResults, kind: SearchKind) -> SearchResults {
    let mut out = SearchResults {
        order: vec![kind],
        ..SearchResults::default()
    };
    match kind {
        SearchKind::Songs => out.songs = results.songs,
        SearchKind::Videos => out.videos = results.videos,
        SearchKind::Albums => out.albums = results.albums,
        SearchKind::Artists => out.artists = results.artists,
        SearchKind::Playlists => out.playlists = results.playlists,
        SearchKind::Channels => out.channels = results.channels,
    }
    out
}

/// The typed filtered search of `ytmapi-rs` for one kind (the fallback). Videos have none.
async fn typed_kind<A: AuthToken>(
    api: &YtMusic<A>,
    query: &str,
    kind: SearchKind,
) -> Result<SearchResults, CatalogError> {
    let mut parts = SearchParts {
        songs: Vec::new(),
        albums: Vec::new(),
        artists: Vec::new(),
        playlists: Vec::new(),
    };
    match kind {
        SearchKind::Songs => parts.songs = api.search_songs(query).await.map_err(map_error)?,
        SearchKind::Albums => parts.albums = api.search_albums(query).await.map_err(map_error)?,
        SearchKind::Artists => {
            parts.artists = api.search_artists(query).await.map_err(map_error)?;
        }
        SearchKind::Playlists => {
            parts.playlists = api.search_playlists(query).await.map_err(map_error)?;
        }
        SearchKind::Videos | SearchKind::Channels => {
            return Err(CatalogError::Parse(
                "unexpected answer from YouTube Music".to_owned(),
            ));
        }
    }
    Ok(map_search(&parts))
}

/// The four filtered searches, joined: songs are required, a failed album, artist or playlist
/// request only leaves that group empty.
async fn filtered<A: AuthToken>(
    api: &YtMusic<A>,
    query: &str,
) -> Result<SearchResults, CatalogError> {
    let (songs, albums, artists, playlists) = tokio::join!(
        api.search_songs(query),
        api.search_albums(query),
        api.search_artists(query),
        api.search_playlists(query),
    );
    Ok(map_search(&SearchParts {
        songs: songs.map_err(map_error)?,
        albums: albums.unwrap_or_default(),
        artists: artists.unwrap_or_default(),
        playlists: playlists.unwrap_or_default(),
    }))
}

#[cfg(test)]
mod tests;
