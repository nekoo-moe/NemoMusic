//! Better Lyrics API provider (opt-in, default off).
//!
//! `GET https://api.betterlyrics.org/getLyrics?s=&a=&al=&d=` returns `{ttml, score?}` with TTML
//! syllable timing (docs: <https://docs.betterlyrics.org/reference/get-lyrics>, read
//! 2026-11-11). Uncached queries need an `X-API-Key`; the README of
//! <https://github.com/better-lyrics/api> says that key is exclusive to the Better Lyrics
//! extension and must not be asked for. NemoMusic therefore sends no key and only gets what
//! the service already has cached (a 401 means "not cached" and is treated as no lyrics).
//! The data comes from lrc.red and an unnamed upstream provider (unverified), and no terms for
//! other apps are published, so the provider is a grey source and stays off until the user turns it on (ADR 0002).

use nemo_core::BoxFuture;
use nemo_core::lyrics::{
    CreditedDoc, LyricsCredit, LyricsDoc, LyricsError, LyricsProvider, LyricsQuery, ProviderId,
    ProviderInfo,
};
use serde::Deserialize;

use crate::http::{build_client, duration_secs, send_text, url_with};
use crate::ttml::parse_ttml;

/// Provider id shown in the UI.
pub const BETTER_LYRICS_ID: ProviderId = ProviderId::new("Better Lyrics");

const URL: &str = "https://api.betterlyrics.org/getLyrics";
const HOSTS: &[&str] = &["api.betterlyrics.org"];
const RISK: &str = "Third-party lyrics cache read without a key; its upstream source is unnamed and no terms for other apps are published, so it may stop at any time.";

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

/// Lyrics from the Better Lyrics API (cache only, no key).
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
        let Some(artist) = query.artists.first() else {
            return Ok(None);
        };
        let duration = duration_secs(query.duration_ms).map(|d| d.to_string());
        let mut params: Vec<(&str, &str)> = vec![("s", &query.title), ("a", artist)];
        if let Some(album) = &query.album {
            params.push(("al", album));
        }
        if let Some(d) = &duration {
            params.push(("d", d));
        }
        let (status, body) = send_text(self.client.get(url_with(URL, &params)?)).await?;
        match status {
            200..=299 => parse_response(&body),
            // 401: not cached and we hold no key. 404/422: nothing for this track.
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
        Box::pin(async move {
            Ok(self.lookup(query).await?.map(|doc| {
                let credit = LyricsCredit {
                    label: "Lyrics via Better Lyrics".to_owned(),
                    url: Some("https://betterlyrics.org".to_owned()),
                    by: None,
                    by_url: None,
                    note: Some(RISK.to_owned()),
                };
                (doc, Some(credit))
            }))
        })
    }

    fn info(&self) -> ProviderInfo {
        ProviderInfo::opt_in(RISK)
    }
}

#[cfg(test)]
mod tests {
    use nemo_core::lyrics::Granularity;

    use super::*;

    #[test]
    fn ttml_answer_parses() {
        let body = serde_json::json!({
            "ttml": r#"<tt><body><div><p begin="1.0" end="2.0"><span begin="1.0" end="2.0">word</span></p></div></body></tt>"#,
            "score": 0.97
        })
        .to_string();
        let doc = parse_response(&body).expect("parses").expect("lyrics");
        assert_eq!(doc.granularity, Granularity::Word);
        assert_eq!(doc.provider, BETTER_LYRICS_ID);
        assert_eq!(parse_response(r#"{"ttml":""}"#), Ok(None));
        assert!(parse_response("x").is_err());
    }

    #[test]
    fn provider_is_opt_in() {
        let p = BetterLyricsProvider::new().expect("client");
        assert!(!p.info().default_enabled && p.info().risk_note.is_some());
        assert_eq!(p.hosts(), ["api.betterlyrics.org"]);
    }
}
