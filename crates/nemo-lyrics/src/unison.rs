//! Unison provider: the Better Lyrics community lyrics API (default on).
//!
//! Facts from <https://github.com/better-lyrics/unison> (README read 2026-11-11): reading needs
//! no key or signature; `GET /lyrics?v=<videoId>` or `?song=&artist=&album=&duration=` returns
//! `{success, data{id, videoId, song, artist, album, lyrics, format, syncType, confidence,
//! submitter{keyId, displayName}}}`; `format` is `ttml`, `lrc` or `plain`; `syncType` is
//! `richsync` (word or syllable timing), `linesync` or `plain`; 404 means none;
//! 503 `LOOKUP_PENDING` carries `Retry-After`. The API is free for apps that credit Unison
//! and the submitter (deadline for apps: 7 February 2027), so every doc comes with a
//! [`LyricsCredit`]. We send an identifying `User-Agent` as the README asks and cache locally
//! instead of using `ETag` revalidation.

use nemo_core::BoxFuture;
use nemo_core::lyrics::{
    CreditedDoc, Granularity, LyricsCredit, LyricsDoc, LyricsError, LyricsProvider, LyricsQuery,
    MatchedMeta, ProviderId, ProviderInfo,
};
use serde::Deserialize;

use crate::http::{build_client, duration_secs, send_text, split_artists, url_with};
use crate::lrc::parse_lrc;
use crate::plain::parse_plain;
use crate::ttml::parse_ttml;

/// Provider id shown in the UI.
pub const UNISON_ID: ProviderId = ProviderId::new("Unison");

const BASE_URL: &str = "https://unison.betterlyrics.org";
const HOSTS: &[&str] = &["unison.betterlyrics.org"];

#[derive(Debug, Deserialize)]
struct Envelope {
    #[serde(default)]
    success: bool,
    data: Option<Data>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Data {
    video_id: Option<String>,
    song: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    #[serde(default)]
    lyrics: String,
    #[serde(default)]
    format: String,
    sync_type: Option<String>,
    confidence: Option<String>,
    submitter: Option<Submitter>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Submitter {
    key_id: Option<String>,
    display_name: Option<String>,
}

/// Ids go into URL paths, so only plain id characters are allowed.
fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// The level of a parsed answer, given its `syncType`. The timing in the lyrics decides:
/// `richsync` (word or syllable timing, per the README) and `linesync` keep what the parser
/// found, and a label never adds timing the text does not have. The one upgrade is a
/// word-timed doc labelled `syllable`, which is treated as syllable synced.
fn level_for(parsed: Granularity, sync_type: Option<&str>) -> Granularity {
    match sync_type.map(str::trim) {
        Some(s) if s.eq_ignore_ascii_case("syllable") && parsed == Granularity::Word => {
            Granularity::Syllable
        }
        _ => parsed,
    }
}

/// Parse a `/lyrics` answer. `Ok(None)` when it holds no usable lyrics.
///
/// # Errors
/// [`LyricsError::Parse`] if the body is not a Unison answer.
pub fn parse_response(body: &str) -> Result<Option<CreditedDoc>, LyricsError> {
    let env: Envelope =
        serde_json::from_str(body).map_err(|e| LyricsError::Parse(e.to_string()))?;
    let Some(data) = env.data.filter(|_| env.success) else {
        return Ok(None);
    };
    let mut doc = match data.format.as_str() {
        "ttml" => parse_ttml(&data.lyrics, UNISON_ID),
        "lrc" => parse_lrc(&data.lyrics, UNISON_ID),
        "plain" => parse_plain(&data.lyrics, UNISON_ID),
        other => {
            return Err(LyricsError::Parse(format!(
                "unknown lyric format {other:?}"
            )));
        }
    };
    if doc.lines.is_empty() {
        return Ok(None);
    }
    doc.granularity = level_for(doc.granularity, data.sync_type.as_deref());
    doc.matched = Some(MatchedMeta {
        title: data.song.unwrap_or_default(),
        artists: data
            .artist
            .as_deref()
            .map(split_artists)
            .unwrap_or_default(),
        album: data.album.filter(|a| !a.trim().is_empty()),
        duration_ms: None,
    });

    let url = data
        .video_id
        .as_deref()
        .filter(|v| safe_id(v))
        .map_or_else(|| BASE_URL.to_owned(), |v| format!("{BASE_URL}/song/{v}"));
    let (by, by_url) = match data.submitter {
        Some(s) => (
            s.display_name.filter(|n| !n.trim().is_empty()),
            s.key_id
                .filter(|k| safe_id(k))
                .map(|k| format!("{BASE_URL}/curator/{k}")),
        ),
        None => (None, None),
    };
    let mut note = Vec::new();
    if let Some(c) = data.confidence.filter(|c| !c.is_empty()) {
        note.push(format!("community confidence: {c}"));
    }
    if let Some(s) = data.sync_type.filter(|s| !s.is_empty()) {
        note.push(format!("sync: {s}"));
    }
    let credit = LyricsCredit {
        label: "Lyrics from Unison".to_owned(),
        url: Some(url),
        by,
        by_url,
        note: (!note.is_empty()).then(|| note.join(", ")),
    };
    Ok(Some((doc, Some(credit))))
}

/// Lyrics from <https://unison.betterlyrics.org>.
#[derive(Debug, Clone)]
pub struct UnisonProvider {
    client: reqwest::Client,
}

impl UnisonProvider {
    /// Build the provider.
    ///
    /// # Errors
    /// [`LyricsError::Unavailable`] if the HTTP client cannot be built.
    pub fn new() -> Result<Self, LyricsError> {
        Ok(Self {
            client: build_client()?,
        })
    }

    async fn lookup(&self, query: &LyricsQuery) -> Result<Option<CreditedDoc>, LyricsError> {
        let duration = duration_secs(query.duration_ms).map(|d| d.to_string());
        let mut params: Vec<(&str, &str)> = Vec::new();
        if let Some(id) = &query.ytm_video_id {
            params.push(("v", id.as_str()));
        }
        params.push(("song", &query.title));
        if let Some(artist) = query.artists.first() {
            params.push(("artist", artist));
        }
        if let Some(album) = &query.album {
            params.push(("album", album));
        }
        if let Some(d) = &duration {
            params.push(("duration", d));
        }
        let url = url_with(&format!("{BASE_URL}/lyrics"), &params)?;
        let (status, body) = send_text(self.client.get(url)).await?;
        match status {
            200..=299 => parse_response(&body),
            // Not found, or an id Unison cannot read: nothing for this track.
            400 | 404 => Ok(None),
            other => Err(LyricsError::Network(format!("HTTP status {other}"))),
        }
    }
}

impl LyricsProvider for UnisonProvider {
    fn id(&self) -> ProviderId {
        UNISON_ID
    }

    fn hosts(&self) -> &'static [&'static str] {
        HOSTS
    }

    fn fetch<'a>(
        &'a self,
        query: &'a LyricsQuery,
    ) -> BoxFuture<'a, Result<Option<LyricsDoc>, LyricsError>> {
        Box::pin(async move { Ok(self.lookup(query).await?.map(|(doc, _)| doc)) })
    }

    fn fetch_credited<'a>(
        &'a self,
        query: &'a LyricsQuery,
    ) -> BoxFuture<'a, Result<Option<CreditedDoc>, LyricsError>> {
        Box::pin(self.lookup(query))
    }

    fn info(&self) -> ProviderInfo {
        ProviderInfo::OPEN
    }

    /// Unison serves TTML `richsync` lyrics with syllable pieces.
    fn best_granularity(&self) -> Granularity {
        Granularity::Syllable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Synthetic bodies in the documented shape. Not real lyrics.
    fn body(format: &str, lyrics: &str) -> String {
        serde_json::json!({
            "success": true,
            "data": {
                "id": 1, "videoId": "abcdefghijk", "song": "Test Song",
                "artist": "Test Artist, Other Artist", "album": "Test Album",
                "lyrics": lyrics, "format": format, "syncType": "richsync",
                "confidence": "low",
                "submitter": {"keyId": "ab12cd", "displayName": "tester"}
            }
        })
        .to_string()
    }

    #[test]
    fn ttml_answer_is_word_synced_and_credited() {
        let ttml = r#"<tt><body><div><p begin="1.0" end="2.0"><span begin="1.0" end="1.5">Hi </span><span begin="1.5" end="2.0">there</span></p></div></body></tt>"#;
        let (doc, credit) = parse_response(&body("ttml", ttml))
            .expect("parses")
            .expect("has lyrics");
        assert_eq!(doc.granularity, Granularity::Word);
        assert_eq!(doc.provider, UNISON_ID);
        let m = doc.matched.expect("matched");
        assert_eq!(m.title, "Test Song");
        assert_eq!(m.artists, ["Test Artist", "Other Artist"]);
        let credit = credit.expect("credit");
        assert_eq!(credit.label, "Lyrics from Unison");
        assert_eq!(
            credit.url.as_deref(),
            Some("https://unison.betterlyrics.org/song/abcdefghijk")
        );
        assert_eq!(credit.by.as_deref(), Some("tester"));
        assert_eq!(
            credit.by_url.as_deref(),
            Some("https://unison.betterlyrics.org/curator/ab12cd")
        );
        assert_eq!(
            credit.note.as_deref(),
            Some("community confidence: low, sync: richsync")
        );
    }

    #[test]
    fn syllable_ttml_and_sync_labels() {
        let ttml = r#"<tt><body><div><p begin="1.0" end="2.0"><span begin="1.0" end="1.4">Hel</span><span begin="1.4" end="2.0">lo</span></p></div></body></tt>"#;
        let (doc, _) = parse_response(&body("ttml", ttml))
            .expect("parses")
            .expect("has lyrics");
        assert_eq!(doc.granularity, Granularity::Syllable);
        assert_eq!(
            level_for(Granularity::Word, Some("syllable")),
            Granularity::Syllable
        );
        assert_eq!(
            level_for(Granularity::Word, Some("richsync")),
            Granularity::Word
        );
        // A label never invents timing.
        assert_eq!(
            level_for(Granularity::Line, Some("syllable")),
            Granularity::Line
        );
        assert_eq!(
            level_for(Granularity::Line, Some("linesync")),
            Granularity::Line
        );
        assert_eq!(
            level_for(Granularity::Unsynced, None),
            Granularity::Unsynced
        );
        let p = UnisonProvider::new().expect("client");
        assert_eq!(p.best_granularity(), Granularity::Syllable);
        assert!(p.info().default_enabled);
    }

    #[test]
    fn lrc_and_plain_answers() {
        let (lrc, _) = parse_response(&body("lrc", "[00:01.00]one\n[00:02.00]two"))
            .expect("parses")
            .expect("has lyrics");
        assert_eq!(lrc.granularity, Granularity::Line);
        let (plain, _) = parse_response(&body("plain", "one\ntwo"))
            .expect("parses")
            .expect("has lyrics");
        assert_eq!(plain.granularity, Granularity::Unsynced);
    }

    #[test]
    fn failures() {
        assert_eq!(
            parse_response(r#"{"success":false,"error":"x","code":"NOT_FOUND"}"#),
            Ok(None)
        );
        assert!(parse_response("nope").is_err());
        assert!(parse_response(&body("docx", "x")).is_err());
        assert_eq!(parse_response(&body("lrc", "")), Ok(None));
    }

    #[test]
    fn unsafe_ids_do_not_reach_urls() {
        let json = body("plain", "one").replace("abcdefghijk", "a/../b");
        let (_, credit) = parse_response(&json).expect("parses").expect("lyrics");
        assert_eq!(
            credit.expect("credit").url.as_deref(),
            Some("https://unison.betterlyrics.org")
        );
    }
}
