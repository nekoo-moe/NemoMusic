//! Better Lyrics API provider (open source, default on).
//!
//! `GET https://api.betterlyrics.org/getLyrics?s=&a=&al=&d=` returns `{ttml, score?}` with TTML
//! syllable or line timing. Facts read 2026-11-11 from <https://lyrics-api-docs.boidu.dev/>,
//! <https://docs.betterlyrics.org/reference/get-lyrics> and
//! <https://docs.betterlyrics.org/docs/authentication>:
//! - The API is free and may be used in other apps; it asks for a link to Better Lyrics
//!   (<https://better-lyrics.boidu.dev/>), shown as the credit "Lyrics via Better Lyrics".
//! - Cached answers need no key. A cache miss needs an `X-API-Key`, which is not issued now;
//!   NemoMusic never sends or asks for a key, so a 401 means "not cached" and is no lyrics.
//! - All four parameters must be sent: the cache key includes album and duration (whole
//!   seconds). Without a known duration the query would miss the cache, so it is not sent.
//! - 404 is a cached "no lyrics"; 429 and 503 (cache-only mode) carry `Retry-After`.
//!
//! The doc level comes from the TTML: syllable pieces give [`Granularity::Syllable`].

use nemo_core::BoxFuture;
use nemo_core::lyrics::{
    CreditedDoc, Granularity, LyricsCredit, LyricsDoc, LyricsError, LyricsProvider, LyricsQuery,
    ProviderId, ProviderInfo,
};
use serde::Deserialize;

use crate::http::{build_client, duration_secs, send_text, url_with};
use crate::ttml::parse_ttml;

/// Provider id shown in the UI.
pub const BETTER_LYRICS_ID: ProviderId = ProviderId::new("Better Lyrics");

/// Page the credit links to, as the API docs ask.
pub const CREDIT_URL: &str = "https://better-lyrics.boidu.dev/";

const URL: &str = "https://api.betterlyrics.org/getLyrics";
const HOSTS: &[&str] = &["api.betterlyrics.org"];

#[derive(Debug, Deserialize)]
struct Answer {
    #[serde(default)]
    ttml: String,
}

/// Parse a `/getLyrics` answer. `Ok(None)` when it holds no lyrics.
///
/// # Errors
/// [`LyricsError::Parse`] if the body is not an answer.
pub fn parse_response(body: &str) -> Result<Option<LyricsDoc>, LyricsError> {
    let a: Answer = serde_json::from_str(body).map_err(|e| LyricsError::Parse(e.to_string()))?;
    let doc = parse_ttml(&a.ttml, BETTER_LYRICS_ID);
    Ok((!doc.lines.is_empty()).then_some(doc))
}

/// The four query parameters (`s`, `a`, `al`, `d`), or `None` when title, artist or duration
/// is unknown. A missing album is sent empty.
#[must_use]
pub fn query_params(query: &LyricsQuery) -> Option<[(&'static str, String); 4]> {
    let title = query.title.trim();
    let artist = query.artists.first().map(|a| a.trim())?;
    let secs = duration_secs(query.duration_ms)?;
    if title.is_empty() || artist.is_empty() {
        return None;
    }
    Some([
        ("s", title.to_owned()),
        ("a", artist.to_owned()),
        ("al", query.album.as_deref().unwrap_or("").trim().to_owned()),
        ("d", secs.to_string()),
    ])
}

/// The credit Better Lyrics asks for.
#[must_use]
pub fn credit() -> LyricsCredit {
    LyricsCredit {
        label: "Lyrics via Better Lyrics".to_owned(),
        url: Some(CREDIT_URL.to_owned()),
        by: None,
        by_url: None,
        note: None,
    }
}

/// Lyrics from the Better Lyrics API (cached answers, no key).
#[derive(Debug, Clone)]
pub struct BetterLyricsProvider {
    client: reqwest::Client,
}

impl BetterLyricsProvider {
    /// Build the provider.
    ///
    /// # Errors
    /// [`LyricsError::Unavailable`] if the HTTP client cannot be built.
    pub fn new() -> Result<Self, LyricsError> {
        Ok(Self {
            client: build_client()?,
        })
    }

    async fn lookup(&self, query: &LyricsQuery) -> Result<Option<LyricsDoc>, LyricsError> {
        let Some(params) = query_params(query) else {
            return Ok(None);
        };
        let pairs: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let (status, body) = send_text(self.client.get(url_with(URL, &pairs)?)).await?;
        match status {
            200..=299 => parse_response(&body),
            // 401: not cached and we hold no key. 404: a cached "no lyrics". 422: no title.
            401 | 403 | 404 | 422 => Ok(None),
            other => Err(LyricsError::Network(format!("HTTP status {other}"))),
        }
    }
}

impl LyricsProvider for BetterLyricsProvider {
    fn id(&self) -> ProviderId {
        BETTER_LYRICS_ID
    }

    fn hosts(&self) -> &'static [&'static str] {
        HOSTS
    }

    fn fetch<'a>(
        &'a self,
        query: &'a LyricsQuery,
    ) -> BoxFuture<'a, Result<Option<LyricsDoc>, LyricsError>> {
        Box::pin(self.lookup(query))
    }

    fn fetch_credited<'a>(
        &'a self,
        query: &'a LyricsQuery,
    ) -> BoxFuture<'a, Result<Option<CreditedDoc>, LyricsError>> {
        Box::pin(async move { Ok(self.lookup(query).await?.map(|doc| (doc, Some(credit())))) })
    }

    fn info(&self) -> ProviderInfo {
        ProviderInfo::OPEN
    }

    /// The API serves TTML with syllable pieces.
    fn best_granularity(&self) -> Granularity {
        Granularity::Syllable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query() -> LyricsQuery {
        LyricsQuery {
            title: "Test Song".into(),
            artists: vec!["Test Artist".into(), "Other".into()],
            album: Some("Test Album".into()),
            duration_ms: Some(154_400),
            ytm_video_id: None,
            local_path: None,
        }
    }

    #[test]
    fn ttml_answers_parse_with_their_level() {
        let word = serde_json::json!({
            "ttml": r#"<tt><body><div><p begin="1.0" end="2.0"><span begin="1.0" end="2.0">word</span></p></div></body></tt>"#,
            "score": 0.97
        })
        .to_string();
        let doc = parse_response(&word).expect("parses").expect("lyrics");
        assert_eq!(doc.granularity, Granularity::Word);
        assert_eq!(doc.provider, BETTER_LYRICS_ID);
        let syllable = serde_json::json!({
            "ttml": r#"<tt><body><div><p begin="1.0" end="2.0"><span begin="1.0" end="1.5">wo</span><span begin="1.5" end="2.0">rd</span></p></div></body></tt>"#
        })
        .to_string();
        let doc = parse_response(&syllable).expect("parses").expect("lyrics");
        assert_eq!(doc.granularity, Granularity::Syllable);
        assert_eq!(parse_response(r#"{"ttml":""}"#), Ok(None));
        assert!(parse_response("x").is_err());
    }

    #[test]
    fn all_four_parameters_are_sent() {
        let p = query_params(&query()).expect("params");
        let got: Vec<(&str, &str)> = p.iter().map(|(k, v)| (*k, v.as_str())).collect();
        assert_eq!(
            got,
            [
                ("s", "Test Song"),
                ("a", "Test Artist"),
                ("al", "Test Album"),
                ("d", "154")
            ]
        );
        let mut no_album = query();
        no_album.album = None;
        assert_eq!(query_params(&no_album).expect("params")[2].1, "");
        let mut no_duration = query();
        no_duration.duration_ms = None;
        assert!(query_params(&no_duration).is_none());
        let mut no_artist = query();
        no_artist.artists.clear();
        assert!(query_params(&no_artist).is_none());
    }

    #[test]
    fn provider_is_open_syllable_and_credited() {
        let p = BetterLyricsProvider::new().expect("client");
        assert!(p.info().default_enabled && p.info().risk_note.is_none());
        assert_eq!(p.best_granularity(), Granularity::Syllable);
        assert_eq!(p.hosts(), ["api.betterlyrics.org"]);
        let c = credit();
        assert_eq!(c.label, "Lyrics via Better Lyrics");
        assert_eq!(c.url.as_deref(), Some("https://better-lyrics.boidu.dev/"));
    }
}
