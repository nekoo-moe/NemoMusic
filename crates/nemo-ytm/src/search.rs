//! YouTube Music's own search, unfiltered, as the website asks it (search-like-ytm).
//!
//! One `POST youtubei/v1/search` with only the query, through the `ytmapi-rs` client the
//! catalog already holds (anonymous or signed in, so the session is used the same way as for
//! every other signed-in call). The answer keeps YouTube Music's ranking: a top-result card
//! (`musicCardShelfRenderer`), then the results in YouTube's order. [`parse_search`] reads it.
//!
//! When that answer cannot be read (an unknown layout) or the request is refused, the four
//! filtered searches of `ytmapi-rs` run instead, as before. A network error is returned as it
//! is: the same network would fail the fallback too, and no request is retried.

use std::borrow::Cow;

use nemo_core::catalog::{CatalogError, SearchResults};
use serde_json::{Map, Value, json};
use ytmapi_rs::YtMusic;
use ytmapi_rs::auth::AuthToken;
use ytmapi_rs::parse::{ParseFrom, ProcessedResult};
use ytmapi_rs::query::{PostMethod, PostQuery, Query};

use crate::catalog::{SearchParts, map_error, map_search};
use crate::raw::{Unparsed, catalog_error};

mod parse;

pub(crate) use parse::parse_search;

/// The unfiltered `search` request: the query and no `params`, like the search box of
/// music.youtube.com.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SearchQuery {
    query: String,
}

impl SearchQuery {
    /// Search for `query` (already trimmed and capped by the caller).
    pub(crate) fn new(query: &str) -> Self {
        Self {
            query: query.to_owned(),
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
        map
    }

    fn params(&self) -> Vec<(&str, Cow<'_, str>)> {
        Vec::new()
    }

    fn path(&self) -> &'static str {
        "search"
    }
}

/// Search like YouTube Music does; the filtered searches only when that answer is unusable.
pub(crate) async fn search<A: AuthToken>(
    api: &YtMusic<A>,
    query: &str,
) -> Result<SearchResults, CatalogError> {
    match api
        .raw_json_query::<SearchQuery>(&SearchQuery::new(query))
        .await
        .map_err(catalog_error)
    {
        Ok(json) => {
            if let Some(results) = parse_search(&json) {
                return Ok(results);
            }
            tracing::warn!("search answer layout not recognised; using the filtered searches");
        }
        Err(e @ CatalogError::Network(_)) => return Err(e),
        Err(_) => {
            tracing::warn!("unfiltered search refused; using the filtered searches");
        }
    }
    filtered(api, query).await
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
