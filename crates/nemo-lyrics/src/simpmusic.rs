//! `SimpMusic` Lyrics provider (opt-in, default off, line timing).
//!
//! `GET https://api-lyrics.simpmusic.org/v1/<videoId>` returns `{success, error?, data[{id,
//! videoId, songTitle, artistName, albumName, durationSeconds, plainLyric, syncedLyrics,
//! richSyncLyrics, vote, ...}]}`. The shape and the read request (unsigned; only writes carry an
//! HMAC) were checked on 2026-11-11 in `SimpMusic`'s own open-source client
//! (<https://github.com/maxrave-dev/core>, `service/lyricsService`: `SimpMusicLyrics.kt`,
//! `LyricsResponse.kt`, `LrcTextParser.kt`). There `richSyncLyrics` is LRC lines with inline
//! `<mm:ss.xx>` word tags, which [`parse_lrc`] could read.
//!
//! It is not read, and the provider stays at line timing and off: a probe on 2026-11-11 with
//! NemoMusic's own `User-Agent` got HTTP 403 "This API is only available to `SimpMusic`
//! clients". Getting past that would mean posing as the `SimpMusic` app, which NemoMusic does
//! not do. Until the API is opened to other apps, this provider finds nothing and reports
//! "unavailable". It needs a YouTube video id and returns nothing for local files.

use nemo_core::BoxFuture;
use nemo_core::lyrics::{
    LyricsDoc, LyricsError, LyricsProvider, LyricsQuery, MatchedMeta, ProviderId, ProviderInfo,
};
use serde::Deserialize;

use crate::http::{build_client, send_text};
use crate::lrc::parse_lrc;
use crate::plain::parse_plain;

/// Provider id shown in the UI.
pub const SIMPMUSIC_ID: ProviderId = ProviderId::new("SimpMusic");

const BASE_URL: &str = "https://api-lyrics.simpmusic.org/v1/";
const HOSTS: &[&str] = &["api-lyrics.simpmusic.org"];
const RISK: &str = "Community database of the SimpMusic app; it answers only SimpMusic's own app (HTTP 403 to others) and publishes no terms for other apps.";

#[derive(Debug, Deserialize)]
struct Answer {
    #[serde(default)]
    data: Option<Vec<Entry>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    song_title: Option<String>,
    artist_name: Option<String>,
    album_name: Option<String>,
    duration_seconds: Option<f64>,
    synced_lyrics: Option<String>,
    plain_lyric: Option<String>,
    vote: Option<i64>,
}

fn non_blank(s: Option<&str>) -> Option<&str> {
    s.filter(|t| !t.trim().is_empty())
}

fn entry_to_doc(e: &Entry) -> Option<LyricsDoc> {
    let synced = non_blank(e.synced_lyrics.as_deref()).map(|t| parse_lrc(t, SIMPMUSIC_ID));
    let mut doc = match synced {
        Some(d) if !d.lines.is_empty() => d,
        _ => {
            let d = parse_plain(non_blank(e.plain_lyric.as_deref())?, SIMPMUSIC_ID);
            if d.lines.is_empty() {
                return None;
            }
            d
        }
    };
    let duration_ms = e
        .duration_seconds
        .filter(|s| s.is_finite() && (0.0..=86_400.0).contains(s))
        .map(|s| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            // Range checked in the filter above.
            let ms = (s * 1000.0).round() as u64;
            ms
        });
    doc.matched = Some(MatchedMeta {
        title: e.song_title.clone().unwrap_or_default(),
        artists: e.artist_name.iter().cloned().collect(),
        album: e.album_name.clone().filter(|a| !a.trim().is_empty()),
        duration_ms,
    });
    Some(doc)
}

/// Parse an answer and take the best-voted entry that has lyrics, preferring synced ones.
///
/// # Errors
/// [`LyricsError::Parse`] if the body is not an answer.
pub fn parse_response(body: &str) -> Result<Option<LyricsDoc>, LyricsError> {
    let a: Answer = serde_json::from_str(body).map_err(|e| LyricsError::Parse(e.to_string()))?;
    let mut docs: Vec<(i64, LyricsDoc)> = a
        .data
        .iter()
        .flatten()
        .filter_map(|e| Some((e.vote.unwrap_or(0), entry_to_doc(e)?)))
        .collect();
    // Best granularity first, then votes. The sort is stable, so ties keep server order.
    docs.sort_by(|a, b| {
        b.1.granularity
            .cmp(&a.1.granularity)
            .then_with(|| b.0.cmp(&a.0))
    });
    Ok(docs.into_iter().next().map(|(_, d)| d))
}

/// Lyrics from the `SimpMusic` community database.
#[derive(Debug, Clone)]
pub struct SimpMusicProvider {
    client: reqwest::Client,
}

impl SimpMusicProvider {
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
        let Some(id) = &query.ytm_video_id else {
            return Ok(None);
        };
        // A video id has only URL-safe characters; the check keeps the path clean regardless.
        let id = id.as_str();
        if !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Ok(None);
        }
        let (status, body) = send_text(self.client.get(format!("{BASE_URL}{id}"))).await?;
        match status {
            200..=299 => parse_response(&body),
            404 => Ok(None),
            403 => Err(LyricsError::Unavailable(
                "SimpMusic refused the request (HTTP 403)".to_owned(),
            )),
            other => Err(LyricsError::Network(format!("HTTP status {other}"))),
        }
    }
}

impl LyricsProvider for SimpMusicProvider {
    fn id(&self) -> ProviderId {
        SIMPMUSIC_ID
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

    #[test]
    fn best_synced_entry_wins() {
        let body = serde_json::json!({"type":"success","data":[
            {"songTitle":"T","artistName":"A","durationSeconds":200,"plainLyric":"one\ntwo","vote":50},
            {"songTitle":"T","artistName":"A","durationSeconds":200,"syncedLyrics":"[00:01.00]one","vote":1},
            {"songTitle":"T","artistName":"A","durationSeconds":200,"syncedLyrics":"[00:01.00]one","vote":9}
        ]})
        .to_string();
        let doc = parse_response(&body).expect("parses").expect("lyrics");
        assert_eq!(doc.granularity, Granularity::Line);
        assert_eq!(
            doc.matched.as_ref().and_then(|m| m.duration_ms),
            Some(200_000)
        );
    }

    #[test]
    fn empty_and_broken_answers() {
        assert_eq!(parse_response(r#"{"type":"success","data":[]}"#), Ok(None));
        assert_eq!(
            parse_response(r#"{"success":false,"data":null,"error":{"code":403}}"#),
            Ok(None)
        );
        assert!(parse_response("[").is_err());
    }

    #[test]
    fn provider_is_opt_in_at_line_level() {
        let p = SimpMusicProvider::new().expect("client");
        assert!(!p.info().default_enabled && p.info().risk_note.is_some());
        assert_eq!(p.best_granularity(), Granularity::Line);
    }
}
