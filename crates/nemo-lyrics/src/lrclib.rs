//! LRCLIB provider (ADR 0002): `GET /api/get`, then `GET /api/search` on a 404.
//!
//! API facts come from <https://lrclib.net/docs.txt> (checked 2026-11-10): `track_name` and
//! `artist_name` are required, `album_name` and `duration` (seconds, 1 to 3600) improve the match;
//! `/api/get` matches only within +-2 s of duration; `/api/search` returns at most 20 records;
//! 429 and 503 carry `Retry-After`; clients must send an identifying `User-Agent`.
//!
//! Parsing is pure and tested on synthetic JSON. Word timing from the `lyricsfile` YAML is not
//! read: its field names are unverified (ADR 0002), so word-synced records fall back to their
//! line-synced `syncedLyrics`.

use std::time::Duration;

use nemo_core::BoxFuture;
use nemo_core::lyrics::{
    LyricsDoc, LyricsError, LyricsProvider, LyricsQuery, MatchedMeta, ProviderId,
};
use reqwest::Url;
use serde::Deserialize;

use crate::lrc::parse_lrc;
use crate::pick::{Candidate, pick_best};
use crate::plain::parse_plain;
use crate::score::score_match;

/// Provider id shown in the UI.
pub const LRCLIB_ID: ProviderId = ProviderId::new("LRCLIB");

/// `User-Agent` sent to LRCLIB (required by its API rules).
pub const USER_AGENT: &str = "NemoMusic/0.0.1 (+https://github.com/nekoo-moe/NemoMusic)";

const BASE_URL: &str = "https://lrclib.net";
const HOSTS: &[&str] = &["lrclib.net"];
/// Total time for one HTTP request.
const REQUEST_TIMEOUT: Duration = Duration::from_millis(3500);
/// LRCLIB asks for 200 to 500 ms between requests.
const REQUEST_GAP: Duration = Duration::from_millis(250);
/// Answers larger than this are not lyrics records.
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
/// LRCLIB accepts durations from 1 to 3600 seconds.
const MAX_DURATION_SECS: u64 = 3600;

/// One lyrics record as LRCLIB sends it (only the fields we use).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    track_name: Option<String>,
    name: Option<String>,
    artist_name: Option<String>,
    album_name: Option<String>,
    duration: Option<f64>,
    #[serde(default)]
    instrumental: bool,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

/// Seconds (as LRCLIB reports them) to milliseconds. Rejects negative, NaN and absurd values.
fn secs_to_ms(secs: f64) -> Option<u64> {
    if secs.is_finite() && (0.0..=86_400.0).contains(&secs) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        // Range checked above, so the cast cannot truncate or change sign.
        let ms = (secs * 1000.0).round() as u64;
        Some(ms)
    } else {
        None
    }
}

fn non_blank(text: Option<&str>) -> Option<&str> {
    text.filter(|t| !t.trim().is_empty())
}

/// Turn one record into a doc. `None` when the record has no usable lyrics and is not instrumental.
fn record_to_doc(rec: &Record) -> Option<LyricsDoc> {
    let title = rec
        .track_name
        .clone()
        .or_else(|| rec.name.clone())
        .unwrap_or_default();
    let matched = MatchedMeta {
        title,
        artists: rec.artist_name.iter().cloned().collect(),
        album: rec.album_name.clone().filter(|a| !a.trim().is_empty()),
        duration_ms: rec.duration.and_then(secs_to_ms),
    };

    let mut doc = if rec.instrumental {
        let mut d = parse_plain("", LRCLIB_ID);
        d.instrumental = true;
        d
    } else {
        let synced = non_blank(rec.synced_lyrics.as_deref()).map(|t| parse_lrc(t, LRCLIB_ID));
        match synced {
            Some(d) if !d.lines.is_empty() => d,
            _ => {
                let plain = non_blank(rec.plain_lyrics.as_deref())?;
                let d = parse_plain(plain, LRCLIB_ID);
                if d.lines.is_empty() {
                    return None;
                }
                d
            }
        }
    };
    doc.matched = Some(matched);
    Some(doc)
}

/// Parse the body of `GET /api/get` (one record). `Ok(None)` when it holds no usable lyrics.
///
/// # Errors
/// [`LyricsError::Parse`] if the body is not a record.
pub fn parse_record(body: &str) -> Result<Option<LyricsDoc>, LyricsError> {
    let rec: Record = serde_json::from_str(body).map_err(|e| LyricsError::Parse(e.to_string()))?;
    Ok(record_to_doc(&rec))
}

/// Parse the body of `GET /api/search` (an array of records). Records without usable lyrics are
/// dropped.
///
/// # Errors
/// [`LyricsError::Parse`] if the body is not an array of records.
pub fn parse_search(body: &str) -> Result<Vec<LyricsDoc>, LyricsError> {
    let recs: Vec<Record> =
        serde_json::from_str(body).map_err(|e| LyricsError::Parse(e.to_string()))?;
    Ok(recs.iter().filter_map(record_to_doc).collect())
}

/// Choose the best acceptable doc from search results with the ADR 0002 rules (score at least
/// 70, title points, duration within 3 s when both are known), then granularity, score and order.
#[must_use]
pub fn select_from_search(query: &LyricsQuery, docs: Vec<LyricsDoc>) -> Option<LyricsDoc> {
    let candidates: Vec<Candidate> = docs
        .into_iter()
        .enumerate()
        .filter_map(|(chain_order, doc)| {
            let score = score_match(query, doc.matched.as_ref()?)?;
            score.is_acceptable().then_some(Candidate {
                doc,
                score: score.total,
                chain_order,
            })
        })
        .collect();
    pick_best(&candidates).map(|c| c.doc.clone())
}

/// What to do with an HTTP status.
#[derive(Debug, Clone, PartialEq, Eq)]
enum StatusAction {
    /// 2xx: read the body.
    Body,
    /// 404: no such record.
    NotFound,
    /// Anything else maps to an error.
    Fail(LyricsError),
}

/// Parse a `Retry-After` value given in seconds (the date form is not used by LRCLIB).
fn parse_retry_after(value: Option<&str>) -> Option<Duration> {
    value?.trim().parse::<u64>().ok().map(Duration::from_secs)
}

fn classify_status(status: u16, retry_after: Option<&str>) -> StatusAction {
    match status {
        200..=299 => StatusAction::Body,
        404 => StatusAction::NotFound,
        429 | 503 => StatusAction::Fail(LyricsError::RateLimited {
            retry_after: parse_retry_after(retry_after),
        }),
        other => StatusAction::Fail(LyricsError::Network(format!("HTTP status {other}"))),
    }
}

/// Query duration as whole seconds in LRCLIB's accepted range.
fn duration_secs(query: &LyricsQuery) -> Option<u64> {
    let secs = query.duration_ms?.saturating_add(500) / 1000;
    (1..=MAX_DURATION_SECS).contains(&secs).then_some(secs)
}

fn build_url(path: &str, params: &[(&str, &str)]) -> Result<Url, LyricsError> {
    Url::parse_with_params(&format!("{BASE_URL}{path}"), params)
        .map_err(|e| LyricsError::Unavailable(e.to_string()))
}

/// Lyrics from <https://lrclib.net>.
#[derive(Debug, Clone)]
pub struct LrclibProvider {
    client: reqwest::Client,
}

impl LrclibProvider {
    /// Build the provider with a short request timeout and the project `User-Agent`.
    ///
    /// # Errors
    /// [`LyricsError::Unavailable`] if the HTTP client cannot be built.
    pub fn new() -> Result<Self, LyricsError> {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| LyricsError::Unavailable(e.without_url().to_string()))?;
        Ok(Self { client })
    }

    /// GET a URL. `Ok(None)` on 404. Never logs the URL (it carries the track metadata).
    async fn request(&self, url: Url) -> Result<Option<String>, LyricsError> {
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| LyricsError::Network(e.without_url().to_string()))?;
        let retry_after = resp
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        match classify_status(resp.status().as_u16(), retry_after.as_deref()) {
            StatusAction::NotFound => return Ok(None),
            StatusAction::Fail(e) => return Err(e),
            StatusAction::Body => {}
        }
        if resp
            .content_length()
            .is_some_and(|n| n > MAX_BODY_BYTES as u64)
        {
            return Err(LyricsError::Parse("answer too large".to_owned()));
        }
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| LyricsError::Network(e.without_url().to_string()))?;
        if bytes.len() > MAX_BODY_BYTES {
            return Err(LyricsError::Parse("answer too large".to_owned()));
        }
        String::from_utf8(bytes.to_vec())
            .map(Some)
            .map_err(|_| LyricsError::Parse("answer is not UTF-8".to_owned()))
    }

    async fn lookup(&self, query: &LyricsQuery) -> Result<Option<LyricsDoc>, LyricsError> {
        let title = query.title.trim();
        let Some(artist) = query.artists.first().map(|a| a.trim()) else {
            return Ok(None);
        };
        if title.is_empty() || artist.is_empty() {
            return Ok(None);
        }

        let secs = duration_secs(query).map(|s| s.to_string());
        let mut params: Vec<(&str, &str)> = vec![("track_name", title), ("artist_name", artist)];
        if let Some(album) = query.album.as_deref().filter(|a| !a.trim().is_empty()) {
            params.push(("album_name", album));
        }
        if let Some(s) = secs.as_deref() {
            params.push(("duration", s));
        }
        if let Some(body) = self.request(build_url("/api/get", &params)?).await? {
            return parse_record(&body);
        }

        // 404: look for near matches. Album is left out so a differently titled album still hits.
        tokio::time::sleep(REQUEST_GAP).await;
        let params = [("track_name", title), ("artist_name", artist)];
        let Some(body) = self.request(build_url("/api/search", &params)?).await? else {
            return Ok(None);
        };
        Ok(select_from_search(query, parse_search(&body)?))
    }
}

impl LyricsProvider for LrclibProvider {
    fn id(&self) -> ProviderId {
        LRCLIB_ID
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use nemo_core::lyrics::Granularity;

    fn query(duration_ms: Option<u64>) -> LyricsQuery {
        LyricsQuery {
            title: "Test Song".into(),
            artists: vec!["Test Artist".into()],
            album: Some("Test Album".into()),
            duration_ms,
            ytm_video_id: None,
            local_path: None,
        }
    }

    const SYNCED: &str = r#"{
        "id": 1, "name": "Test Song", "trackName": "Test Song", "artistName": "Test Artist",
        "albumName": "Test Album", "duration": 200.4, "instrumental": false, "hasWordSync": false,
        "plainLyrics": "first line\nsecond line",
        "syncedLyrics": "[00:01.00] first line\n[00:05.50] second line\n[03:20.00] ",
        "lyricsfile": "version: '1.0'"
    }"#;

    #[test]
    fn synced_record_becomes_line_doc() {
        let doc = parse_record(SYNCED).expect("parses").expect("some");
        assert_eq!(doc.granularity, Granularity::Line);
        assert_eq!(doc.provider, LRCLIB_ID);
        assert!(!doc.instrumental);
        assert_eq!(doc.lines[0].start_ms, Some(1000));
        assert_eq!(doc.lines[1].start_ms, Some(5500));
        assert_eq!(doc.lines[1].text, "second line");
        let m = doc.matched.expect("matched");
        assert_eq!(m.title, "Test Song");
        assert_eq!(m.artists, ["Test Artist"]);
        assert_eq!(m.album.as_deref(), Some("Test Album"));
        assert_eq!(m.duration_ms, Some(200_400));
    }

    #[test]
    fn plain_only_record_is_unsynced() {
        let body = r#"{"trackName":"T","artistName":"A","duration":10,"instrumental":false,
            "plainLyrics":"la\nla la","syncedLyrics":null}"#;
        let doc = parse_record(body).expect("parses").expect("some");
        assert_eq!(doc.granularity, Granularity::Unsynced);
        assert!(doc.lines.iter().all(|l| l.start_ms.is_none()));
        assert_eq!(doc.lines.len(), 2);
    }

    #[test]
    fn blank_synced_falls_back_to_plain() {
        let body = r#"{"trackName":"T","artistName":"A","plainLyrics":"x","syncedLyrics":"  "}"#;
        let doc = parse_record(body).expect("parses").expect("some");
        assert_eq!(doc.granularity, Granularity::Unsynced);
    }

    #[test]
    fn instrumental_record() {
        let body = r#"{"trackName":"T","artistName":"A","duration":99,"instrumental":true,
            "plainLyrics":null,"syncedLyrics":null}"#;
        let doc = parse_record(body).expect("parses").expect("some");
        assert!(doc.instrumental);
        assert_eq!(doc.lines.len(), 0);
    }

    #[test]
    fn record_without_lyrics_is_none() {
        let body = r#"{"trackName":"T","artistName":"A","instrumental":false,
            "plainLyrics":null,"syncedLyrics":null}"#;
        assert_eq!(parse_record(body), Ok(None));
    }

    #[test]
    fn bad_json_is_parse_error() {
        assert!(matches!(parse_record("<html>"), Err(LyricsError::Parse(_))));
        assert!(matches!(parse_search("{}"), Err(LyricsError::Parse(_))));
    }

    #[test]
    fn absurd_duration_is_dropped() {
        for d in ["-5", "1e12"] {
            let body =
                format!(r#"{{"trackName":"T","artistName":"A","duration":{d},"plainLyrics":"x"}}"#);
            let doc = parse_record(&body).expect("parses").expect("some");
            assert_eq!(doc.matched.expect("matched").duration_ms, None);
        }
    }

    fn search_body() -> String {
        // Three records: wrong song, right song but plain only, right song synced.
        r#"[
            {"trackName":"Other Thing","artistName":"Someone Else","albumName":"X","duration":200,
             "plainLyrics":"nope","syncedLyrics":"[00:01.00] nope"},
            {"trackName":"Test Song","artistName":"Test Artist","albumName":"Test Album","duration":200,
             "plainLyrics":"plain only","syncedLyrics":null},
            {"trackName":"Test Song (Remastered 2011)","artistName":"Test Artist",
             "albumName":"Test Album","duration":201,
             "plainLyrics":"p","syncedLyrics":"[00:02.00] synced line"}
        ]"#
        .to_owned()
    }

    #[test]
    fn search_picks_synced_acceptable_record() {
        let docs = parse_search(&search_body()).expect("parses");
        assert_eq!(docs.len(), 3);
        let doc = select_from_search(&query(Some(200_000)), docs).expect("some");
        assert_eq!(doc.granularity, Granularity::Line);
        assert_eq!(doc.lines[0].text, "synced line");
    }

    #[test]
    fn search_rejects_duration_beyond_three_seconds() {
        let docs = parse_search(&search_body()).expect("parses");
        assert!(select_from_search(&query(Some(230_000)), docs).is_none());
    }

    #[test]
    fn search_rejects_wrong_songs() {
        let body = r#"[{"trackName":"Other Thing","artistName":"Someone Else",
            "duration":200,"plainLyrics":"nope"}]"#;
        let docs = parse_search(body).expect("parses");
        assert!(select_from_search(&query(Some(200_000)), docs).is_none());
        assert!(select_from_search(&query(None), Vec::new()).is_none());
    }

    #[test]
    fn status_mapping() {
        assert_eq!(classify_status(200, None), StatusAction::Body);
        assert_eq!(classify_status(404, None), StatusAction::NotFound);
        assert_eq!(
            classify_status(429, Some("7")),
            StatusAction::Fail(LyricsError::RateLimited {
                retry_after: Some(Duration::from_secs(7))
            })
        );
        assert_eq!(
            classify_status(503, Some("soon")),
            StatusAction::Fail(LyricsError::RateLimited { retry_after: None })
        );
        assert!(matches!(
            classify_status(500, None),
            StatusAction::Fail(LyricsError::Network(_))
        ));
    }

    #[test]
    fn duration_param_rounds_and_clamps() {
        assert_eq!(duration_secs(&query(Some(200_400))), Some(200));
        assert_eq!(duration_secs(&query(Some(200_500))), Some(201));
        assert_eq!(duration_secs(&query(Some(100))), None);
        assert_eq!(duration_secs(&query(Some(3_700_000))), None);
        assert_eq!(duration_secs(&query(None)), None);
    }

    #[test]
    fn urls_are_encoded() {
        let url =
            build_url("/api/get", &[("track_name", "A & B"), ("duration", "5")]).expect("url");
        assert_eq!(url.host_str(), Some("lrclib.net"));
        assert!(!url.as_str().contains(" & "));
        assert!(url.query().is_some_and(|q| q.contains("duration=5")));
    }

    #[test]
    fn provider_identity() {
        let p = LrclibProvider::new().expect("client");
        assert_eq!(p.id().as_str(), "LRCLIB");
        assert_eq!(p.hosts(), ["lrclib.net"]);
    }
}
