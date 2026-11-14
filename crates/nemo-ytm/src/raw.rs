//! Raw `browse` requests and error labels.
//!
//! `ytmapi-rs` 0.3.4 parses a fixed set of page layouts and fails on the whole page when one
//! part is unknown. The pages added for the account (library, history, explore, moods, podcasts)
//! are therefore requested as raw JSON through its public `Query` trait and read by the tolerant
//! parsers in `crate::feeds`. Request shapes follow `ytmusicapi` (`mixins/browsing.py`,
//! `mixins/library.py`, `mixins/explore.py`, `mixins/podcasts.py`): a `browseId` plus optional
//! `params`, or a continuation token in the query string.

use std::borrow::Cow;

use nemo_core::catalog::CatalogError;
use serde_json::{Map, Value, json};
use ytmapi_rs::auth::AuthToken;
use ytmapi_rs::error::ErrorKind;
use ytmapi_rs::parse::{ParseFrom, ProcessedResult};
use ytmapi_rs::query::{PostMethod, PostQuery, Query};

/// `ytmapi-rs` hands the answer back untouched; this crate reads the JSON itself.
#[derive(Debug)]
pub struct Unparsed;

impl ParseFrom<BrowseQuery> for Unparsed {
    fn parse_from(_: ProcessedResult<BrowseQuery>) -> ytmapi_rs::Result<Self> {
        Ok(Self)
    }
}

/// A `browse` request: a page id, optional `params`, or a continuation token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowseQuery {
    browse_id: Option<String>,
    params: Option<String>,
    continuation: Option<String>,
}

impl BrowseQuery {
    /// Page `browse_id`.
    #[must_use]
    pub fn page(browse_id: &str) -> Self {
        Self {
            browse_id: Some(browse_id.to_owned()),
            params: None,
            continuation: None,
        }
    }

    /// Page `browse_id` with the `params` blob a mood chip or sort order carries.
    #[must_use]
    pub fn with_params(browse_id: &str, params: &str) -> Self {
        Self {
            browse_id: Some(browse_id.to_owned()),
            params: Some(params.to_owned()),
            continuation: None,
        }
    }

    /// The next part of a long list.
    #[must_use]
    pub fn continued(token: &str) -> Self {
        Self {
            browse_id: None,
            params: None,
            continuation: Some(token.to_owned()),
        }
    }
}

impl<A: AuthToken> Query<A> for BrowseQuery {
    type Output = Unparsed;
    type Method = PostMethod;
}

impl PostQuery for BrowseQuery {
    fn header(&self) -> Map<String, Value> {
        let mut map = Map::new();
        if let Some(id) = &self.browse_id {
            map.insert("browseId".to_owned(), json!(id));
        }
        if let Some(params) = &self.params {
            map.insert("params".to_owned(), json!(params));
        }
        map
    }

    fn params(&self) -> Vec<(&str, Cow<'_, str>)> {
        // ytmusicapi sends continuations as `ctoken`/`continuation` query parameters with
        // `type=next`.
        match &self.continuation {
            Some(token) => vec![
                ("ctoken", Cow::Borrowed(token.as_str())),
                ("continuation", Cow::Borrowed(token.as_str())),
                ("type", Cow::Borrowed("next")),
            ],
            None => Vec::new(),
        }
    }

    fn path(&self) -> &'static str {
        "browse"
    }
}

/// A `next` request for a watch playlist: radios and mixes without a browse page. Body as in
/// `ytmusicapi` `mixins/watch.py` (`get_watch_playlist`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchQuery {
    playlist_id: String,
}

impl WatchQuery {
    /// The watch playlist `playlist_id` (no `VL` prefix).
    #[must_use]
    pub fn playlist(playlist_id: &str) -> Self {
        Self {
            playlist_id: playlist_id.to_owned(),
        }
    }
}

impl ParseFrom<WatchQuery> for Unparsed {
    fn parse_from(_: ProcessedResult<WatchQuery>) -> ytmapi_rs::Result<Self> {
        Ok(Self)
    }
}

impl<A: AuthToken> Query<A> for WatchQuery {
    type Output = Unparsed;
    type Method = PostMethod;
}

impl PostQuery for WatchQuery {
    fn header(&self) -> Map<String, Value> {
        let mut map = Map::new();
        map.insert("playlistId".to_owned(), json!(self.playlist_id));
        map.insert("enablePersistentPlaylistPanel".to_owned(), json!(true));
        map.insert("isAudioOnly".to_owned(), json!(true));
        map.insert(
            "tunerSettingValue".to_owned(),
            json!("AUTOMIX_SETTING_NORMAL"),
        );
        map
    }

    fn params(&self) -> Vec<(&str, Cow<'_, str>)> {
        Vec::new()
    }

    fn path(&self) -> &'static str {
        "next"
    }
}

/// A short, fixed label for a `ytmapi-rs` error. It never contains a response body, a URL or a
/// header, so it is safe to show and to log.
#[must_use]
pub fn error_label(error: ytmapi_rs::Error) -> String {
    match error.into_kind() {
        ErrorKind::Web { .. } => "web request failed".to_owned(),
        ErrorKind::Io(_) => "io error".to_owned(),
        ErrorKind::JsonParsing(_) => "library parser: layout not recognised".to_owned(),
        ErrorKind::InvalidResponse { .. } => "answer is not JSON".to_owned(),
        ErrorKind::Header => "sign-in header not recognised".to_owned(),
        ErrorKind::OtherErrorCodeInResponse { code, .. } => format!("server error code {code}"),
        ErrorKind::ApiStatusFailed => "status failed".to_owned(),
        ErrorKind::OAuthTokenExpired { .. } => "token expired".to_owned(),
        _ => "other error".to_owned(),
    }
}

/// Map an error to the fixed-text [`CatalogError`] the rest of the app shows.
pub(crate) fn catalog_error(error: ytmapi_rs::Error) -> CatalogError {
    match error.into_kind() {
        ErrorKind::Web { .. } | ErrorKind::Io(_) => {
            CatalogError::Network("request to YouTube Music failed".to_owned())
        }
        ErrorKind::OtherErrorCodeInResponse {
            code: 401 | 403, ..
        } => CatalogError::Unavailable("YouTube Music did not accept the sign-in".to_owned()),
        _ => CatalogError::Parse("unexpected answer from YouTube Music".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_and_params_are_in_the_body() {
        let q = BrowseQuery::with_params("FEx", "abc");
        let body = q.header();
        assert_eq!(body.get("browseId"), Some(&json!("FEx")));
        assert_eq!(body.get("params"), Some(&json!("abc")));
        assert_eq!(PostQuery::params(&q).len(), 0);
    }

    #[test]
    fn a_continuation_goes_in_the_query_string() {
        let q = BrowseQuery::continued("tok");
        assert!(q.header().is_empty());
        let params = PostQuery::params(&q);
        assert!(params.iter().any(|(k, v)| *k == "ctoken" && v == "tok"));
        assert!(params.iter().any(|(k, v)| *k == "type" && v == "next"));
    }
}
