//! `NetEase` Cloud Music provider (opt-in, default off).
//!
//! `NetEase` has no public API for third-party apps. This provider uses the two long-standing
//! unauthenticated web endpoints that community tools use: `POST /api/search/get/` (form
//! `s`, `type=1`, `limit`, `offset`) and `GET /api/song/lyric?id=&lv=-1&kv=-1&tv=-1`. No key,
//! cookie, token or request signing is used, and none is faked. Only the plain line LRC is read,
//! so the provider reports line timing. The word-timed `yrc` format is not available without
//! signing: two probes on 2026-11-11 of the unsigned `GET /api/song/lyric/v1` with
//! `yv`/`ytv` set (`-1`, then `0`) answered `lrc`, `tlyric` and `romalrc` but no `yrc`;
//! community tools get `yrc` only through the encrypted `eapi` endpoints, which this provider
//! will not use. Whether this use fits `NetEase`'s terms is unverified, so it stays off until
//! the user turns it on (ADR 0002).

use std::time::Duration;

use nemo_core::BoxFuture;
use nemo_core::lyrics::{
    LyricsDoc, LyricsError, LyricsProvider, LyricsQuery, MatchedMeta, ProviderId, ProviderInfo,
};
use serde::Deserialize;

use crate::http::{build_client, send_text, url_with};
use crate::lrc::parse_lrc;
use crate::score::score_match;

/// Provider id shown in the UI.
pub const NETEASE_ID: ProviderId = ProviderId::new("NetEase");

const SEARCH_URL: &str = "https://music.163.com/api/search/get/";
const LYRIC_URL: &str = "https://music.163.com/api/song/lyric";
const HOSTS: &[&str] = &["music.163.com"];
/// Pause between the search and the lyric request.
const REQUEST_GAP: Duration = Duration::from_millis(300);
const RISK: &str = "Unofficial use of NetEase's web endpoints; terms unverified and they can change or block without notice.";

#[derive(Debug, Deserialize)]
struct SearchAnswer {
    result: Option<SearchResult>,
}

#[derive(Debug, Deserialize)]
struct SearchResult {
    #[serde(default)]
    songs: Vec<Song>,
}

/// A search hit.
#[derive(Debug, Deserialize)]
pub struct Song {
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    artists: Vec<Named>,
    album: Option<Named>,
    duration: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct Named {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct LyricAnswer {
    lrc: Option<LrcField>,
}

#[derive(Debug, Deserialize)]
struct LrcField {
    #[serde(default)]
    lyric: String,
}

impl Song {
    fn meta(&self) -> MatchedMeta {
        MatchedMeta {
            title: self.name.clone(),
            artists: self.artists.iter().map(|a| a.name.clone()).collect(),
            album: self
                .album
                .as_ref()
                .map(|a| a.name.clone())
                .filter(|a| !a.trim().is_empty()),
            duration_ms: self.duration,
        }
    }
}

/// Parse the search answer into hits.
///
/// # Errors
/// [`LyricsError::Parse`] if the body is not a search answer.
pub fn parse_search(body: &str) -> Result<Vec<Song>, LyricsError> {
    let a: SearchAnswer =
        serde_json::from_str(body).map_err(|e| LyricsError::Parse(e.to_string()))?;
    Ok(a.result.map(|r| r.songs).unwrap_or_default())
}

/// The id of the best acceptable hit (ADR 0002 scoring), with its metadata and score. On equal
/// scores the earlier hit wins.
#[must_use]
pub fn best_song(query: &LyricsQuery, songs: &[Song]) -> Option<(u64, MatchedMeta, u8)> {
    songs
        .iter()
        .enumerate()
        .filter_map(|(order, s)| {
            let meta = s.meta();
            let score = score_match(query, &meta)?;
            score
                .is_acceptable()
                .then_some((order, s.id, meta, score.total))
        })
        .max_by(|a, b| a.3.cmp(&b.3).then(b.0.cmp(&a.0)))
        .map(|(_, id, meta, score)| (id, meta, score))
}
/// Parse the lyric answer into a line-synced doc, with `matched` set.
///
/// # Errors
/// [`LyricsError::Parse`] if the body is not a lyric answer.
pub fn parse_lyric(body: &str, meta: MatchedMeta) -> Result<Option<LyricsDoc>, LyricsError> {
    let a: LyricAnswer =
        serde_json::from_str(body).map_err(|e| LyricsError::Parse(e.to_string()))?;
    let Some(lrc) = a.lrc.filter(|l| !l.lyric.trim().is_empty()) else {
        return Ok(None);
    };
    let mut doc = parse_lrc(&lrc.lyric, NETEASE_ID);
    if doc.lines.is_empty() {
        return Ok(None);
    }
    doc.matched = Some(meta);
    Ok(Some(doc))
}

/// Lyrics from `NetEase` Cloud Music.
#[derive(Debug, Clone)]
pub struct NeteaseProvider {
    client: reqwest::Client,
}

impl NeteaseProvider {
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
        let mut term = query.title.clone();
        if let Some(artist) = query.artists.first() {
            term.push(' ');
            term.push_str(artist);
        }
        // The form body is built with the URL encoder so no extra reqwest feature is needed.
        let form = url_with(
            "https://x.invalid/",
            &[("s", &term), ("type", "1"), ("limit", "5"), ("offset", "0")],
        )?;
        let form_body = form.query().unwrap_or_default().to_owned();
        let req = self
            .client
            .post(SEARCH_URL)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(form_body);
        let (status, body) = send_text(req).await?;
        if !(200..300).contains(&status) {
            return if matches!(status, 400 | 404) {
                Ok(None)
            } else {
                Err(LyricsError::Network(format!("HTTP status {status}")))
            };
        }
        let songs = parse_search(&body)?;
        let Some((id, meta, _)) = best_song(query, &songs) else {
            return Ok(None);
        };
        tokio::time::sleep(REQUEST_GAP).await;
        let id = id.to_string();
        let url = url_with(
            LYRIC_URL,
            &[("id", &id), ("lv", "-1"), ("kv", "-1"), ("tv", "-1")],
        )?;
        let (status, body) = send_text(self.client.get(url)).await?;
        match status {
            200..=299 => parse_lyric(&body, meta),
            400 | 404 => Ok(None),
            other => Err(LyricsError::Network(format!("HTTP status {other}"))),
        }
    }
}

impl LyricsProvider for NeteaseProvider {
    fn id(&self) -> ProviderId {
        NETEASE_ID
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

    fn info(&self) -> ProviderInfo {
        ProviderInfo::opt_in(RISK)
    }
}

#[cfg(test)]
mod tests {
    use nemo_core::lyrics::Granularity;

    use super::*;

    fn query() -> LyricsQuery {
        LyricsQuery {
            title: "Test Song".into(),
            artists: vec!["Test Artist".into()],
            album: None,
            duration_ms: Some(200_000),
            ytm_video_id: None,
            local_path: None,
        }
    }

    // Synthetic bodies in the shape the endpoints use.
    const SEARCH: &str = r#"{"code":200,"result":{"songCount":2,"songs":[
        {"id":11,"name":"Other Thing","artists":[{"name":"Nobody"}],"album":{"name":"X"},"duration":90000},
        {"id":22,"name":"Test Song","artists":[{"name":"Test Artist"}],"album":{"name":"Y"},"duration":200500}]}}"#;

    #[test]
    fn picks_the_matching_hit() {
        let songs = parse_search(SEARCH).expect("parses");
        let (id, meta, score) = best_song(&query(), &songs).expect("a match");
        assert_eq!(id, 22);
        assert_eq!(meta.title, "Test Song");
        assert!(score >= 70);
    }

    #[test]
    fn no_acceptable_hit_is_none() {
        let mut q = query();
        q.title = "Completely Unrelated Words".into();
        q.artists = vec!["Someone Else".into()];
        let songs = parse_search(SEARCH).expect("parses");
        assert!(best_song(&q, &songs).is_none());
        assert!(parse_search(r#"{"code":200}"#).expect("parses").is_empty());
    }

    #[test]
    fn lyric_answer_is_line_synced() {
        let meta = parse_search(SEARCH).expect("parses")[1].meta();
        let doc = parse_lyric(
            r#"{"code":200,"lrc":{"version":3,"lyric":"[00:01.00]one\n[00:02.50]two"}}"#,
            meta,
        )
        .expect("parses")
        .expect("lyrics");
        assert_eq!(doc.granularity, Granularity::Line);
        assert_eq!(doc.provider, NETEASE_ID);
        assert_eq!(doc.lines.len(), 2);
        assert!(doc.matched.is_some());
        let none = parse_lyric(
            r#"{"nolyric":true}"#,
            MatchedMeta {
                title: String::new(),
                artists: vec![],
                album: None,
                duration_ms: None,
            },
        );
        assert_eq!(none, Ok(None));
    }

    #[test]
    fn provider_is_opt_in_with_a_risk_note() {
        let p = NeteaseProvider::new().expect("client");
        assert!(!p.info().default_enabled);
        assert!(p.info().risk_note.is_some());
        assert_eq!(p.best_granularity(), Granularity::Line);
        assert_eq!(p.hosts(), ["music.163.com"]);
    }
}
