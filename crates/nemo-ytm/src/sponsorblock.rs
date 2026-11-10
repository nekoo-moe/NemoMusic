//! `SponsorBlock` lookups with the privacy hash prefix (ADR 0003, Part B).
//!
//! Query format checked on <https://wiki.sponsor.ajay.app/w/API_Docs> on 2026-11-10:
//! `GET /api/skipSegments/:sha256HashPrefix` with repeatable `category` and `actionType`.
//! Only the first 4 hex characters of the SHA-256 of the video id are sent. The answer lists
//! every video sharing the prefix, so it is filtered locally by exact id. Segment data is
//! CC BY-NC-SA 4.0: it is never stored in the repo, and tests use synthetic fixtures only.

use std::fmt::Write as _;
use std::time::Duration;

use nemo_core::BoxFuture;
use nemo_core::segment::{Segment, SegmentCategory, SegmentError, SegmentSource};
use nemo_core::track::VideoId;
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Base URL of the public API.
pub const BASE_URL: &str = "https://sponsor.ajay.app";
/// Segments whose submitted video duration differs from the real one by more than this are
/// treated as outdated.
pub const MAX_DURATION_DRIFT: Duration = Duration::from_secs(2);

/// Categories requested when the caller does not choose: the ADR 0003 defaults.
pub const DEFAULT_CATEGORIES: [SegmentCategory; 4] = [
    SegmentCategory::MusicOffTopic,
    SegmentCategory::Sponsor,
    SegmentCategory::SelfPromo,
    SegmentCategory::Interaction,
];

fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut out = String::with_capacity(64);
    for byte in &digest {
        // Writing to a String cannot fail.
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// First 4 lowercase hex characters of the SHA-256 of the video id.
#[must_use]
pub fn hash_prefix(video: &VideoId) -> String {
    let mut hex = sha256_hex(video.as_str().as_bytes());
    hex.truncate(4);
    hex
}

/// The wire name of a category.
#[must_use]
pub const fn category_name(category: SegmentCategory) -> &'static str {
    match category {
        SegmentCategory::Sponsor => "sponsor",
        SegmentCategory::SelfPromo => "selfpromo",
        SegmentCategory::Interaction => "interaction",
        SegmentCategory::Intro => "intro",
        SegmentCategory::Outro => "outro",
        SegmentCategory::Preview => "preview",
        SegmentCategory::Hook => "hook",
        SegmentCategory::Filler => "filler",
        SegmentCategory::MusicOffTopic => "music_offtopic",
    }
}

fn category_from_name(name: &str) -> Option<SegmentCategory> {
    [
        SegmentCategory::Sponsor,
        SegmentCategory::SelfPromo,
        SegmentCategory::Interaction,
        SegmentCategory::Intro,
        SegmentCategory::Outro,
        SegmentCategory::Preview,
        SegmentCategory::Hook,
        SegmentCategory::Filler,
        SegmentCategory::MusicOffTopic,
    ]
    .into_iter()
    .find(|c| category_name(*c) == name)
}

/// Percent-encode everything except the RFC 3986 unreserved characters.
fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            // Writing to a String cannot fail.
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

/// URL of the lookup for one video. Contains the 4-character hash prefix, never the video id.
#[must_use]
pub fn request_url(video: &VideoId, categories: &[SegmentCategory]) -> String {
    let mut url = format!(
        "{BASE_URL}/api/skipSegments/{}?actionType={}",
        hash_prefix(video),
        percent_encode("skip")
    );
    for category in categories {
        url.push_str("&category=");
        url.push_str(&percent_encode(category_name(*category)));
    }
    url
}

#[derive(Deserialize)]
struct Entry {
    #[serde(rename = "videoID")]
    video_id: String,
    #[serde(default)]
    segments: Vec<RawSegment>,
}

#[derive(Deserialize)]
struct RawSegment {
    segment: Vec<f64>,
    #[serde(rename = "UUID", default)]
    uuid: String,
    #[serde(default)]
    category: String,
    #[serde(rename = "actionType", default)]
    action_type: String,
    #[serde(rename = "videoDuration", default)]
    video_duration: f64,
}

fn secs(value: f64) -> Option<Duration> {
    Duration::try_from_secs_f64(value).ok()
}

fn convert(raw: &RawSegment, real: Duration) -> Option<Segment> {
    if raw.action_type != "skip" {
        return None;
    }
    let category = category_from_name(&raw.category)?;
    if raw.video_duration > 0.0 {
        let submitted = secs(raw.video_duration)?;
        if submitted.abs_diff(real) > MAX_DURATION_DRIFT {
            return None;
        }
    }
    let [start, end] = raw.segment[..] else {
        return None;
    };
    let start = secs(start)?;
    let end = secs(end)?.min(real);
    (start < end).then(|| Segment {
        start,
        end,
        category,
        id: raw.uuid.clone(),
    })
}

/// Parse a `skipSegments` answer for one video.
///
/// Keeps only the entry whose `videoID` equals `video` exactly, only `skip` actions and known
/// categories. Drops empty or reversed spans, clamps ends to `real_duration`, ignores segments
/// whose submitted duration is known and differs by more than 2 s, and sorts by start.
///
/// # Errors
/// [`SegmentError::Parse`] when the body is not the expected JSON array.
pub fn parse_response(
    body: &str,
    video: &VideoId,
    real_duration: Duration,
) -> Result<Vec<Segment>, SegmentError> {
    let entries: Vec<Entry> = serde_json::from_str(body)
        .map_err(|_| SegmentError::Parse("not the expected JSON array".to_owned()))?;
    let mut out: Vec<Segment> = entries
        .iter()
        .filter(|e| e.video_id == video.as_str())
        .flat_map(|e| e.segments.iter())
        .filter_map(|raw| convert(raw, real_duration))
        .collect();
    out.sort_by(|a, b| a.start.cmp(&b.start).then(a.end.cmp(&b.end)));
    Ok(out)
}

/// A finished HTTP answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response body as text.
    pub body: String,
    /// Value of `Retry-After` in seconds, if the server sent one.
    pub retry_after: Option<Duration>,
}

/// Minimal HTTP GET, so tests can inject a fake. No real implementation exists in this crate yet.
pub trait HttpGet: Send + Sync {
    /// GET one URL. The error text must not contain the URL.
    fn get<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<HttpResponse, String>>;
}

/// `SponsorBlock` as a [`SegmentSource`].
#[derive(Debug, Clone)]
pub struct SponsorBlockSource<H: HttpGet> {
    http: H,
    categories: Vec<SegmentCategory>,
}

impl<H: HttpGet> SponsorBlockSource<H> {
    /// Use the ADR 0003 default categories.
    #[must_use]
    pub fn new(http: H) -> Self {
        Self::with_categories(http, DEFAULT_CATEGORIES.to_vec())
    }

    /// Request the given categories.
    #[must_use]
    pub const fn with_categories(http: H, categories: Vec<SegmentCategory>) -> Self {
        Self { http, categories }
    }
}

impl<H: HttpGet> SegmentSource for SponsorBlockSource<H> {
    fn segments<'a>(
        &'a self,
        video: &'a VideoId,
        duration: Duration,
    ) -> BoxFuture<'a, Result<Vec<Segment>, SegmentError>> {
        Box::pin(async move {
            let url = request_url(video, &self.categories);
            let response = self.http.get(&url).await.map_err(SegmentError::Network)?;
            match response.status {
                200 => parse_response(&response.body, video, duration),
                404 => Ok(Vec::new()),
                429 => Err(SegmentError::RateLimited {
                    retry_after: response.retry_after,
                }),
                other => Err(SegmentError::Network(format!("HTTP status {other}"))),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    fn vid(s: &str) -> VideoId {
        VideoId::parse(s).expect("test id is valid")
    }

    const ID: &str = "abcdefghijk";
    const OTHER: &str = "zzzzzzzzzzz";

    #[test]
    fn sha256_matches_the_nist_test_vector() {
        // FIPS 180-2 / NIST example: SHA-256("abc").
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // NIST: SHA-256("") as well.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn prefix_is_four_hex_chars_of_the_digest() {
        // Digest computed independently with .NET SHA256 over the ASCII id.
        assert_eq!(hash_prefix(&vid(ID)), "ca2f");
    }

    #[test]
    fn url_has_prefix_action_and_categories_only() {
        let url = request_url(&vid(ID), &DEFAULT_CATEGORIES);
        assert_eq!(
            url,
            "https://sponsor.ajay.app/api/skipSegments/ca2f?actionType=skip\
             &category=music_offtopic&category=sponsor&category=selfpromo&category=interaction"
        );
        assert!(!url.contains(ID));
    }

    #[test]
    fn percent_encoding_escapes_reserved_bytes() {
        assert_eq!(percent_encode("a b&c=d/é"), "a%20b%26c%3Dd%2F%C3%A9");
        assert_eq!(percent_encode("az-._~09"), "az-._~09");
    }

    fn seg(range: &str, cat: &str, action: &str, dur: &str, uuid: &str) -> String {
        format!(
            r#"{{"segment":{range},"UUID":"{uuid}","category":"{cat}","actionType":"{action}","videoDuration":{dur}}}"#
        )
    }

    fn body(entries: &[(&str, Vec<String>)]) -> String {
        let items: Vec<String> = entries
            .iter()
            .map(|(id, segs)| format!(r#"{{"videoID":"{id}","segments":[{}]}}"#, segs.join(",")))
            .collect();
        format!("[{}]", items.join(","))
    }

    const REAL: Duration = Duration::from_secs(200);

    #[test]
    fn filters_sorts_clamps_and_drops() {
        let json = body(&[
            (
                ID,
                vec![
                    seg("[100.0, 120.5]", "sponsor", "skip", "200", "u2"),
                    seg("[10, 20]", "music_offtopic", "skip", "0", "u1"),
                    seg("[190, 999]", "outro", "skip", "201.5", "u3"),
                    seg("[30, 40]", "sponsor", "mute", "200", "mute"),
                    seg("[30, 40]", "poi_highlight", "skip", "200", "unk"),
                    seg("[50, 50]", "sponsor", "skip", "200", "empty"),
                    seg("[60, 55]", "sponsor", "skip", "200", "rev"),
                    seg("[210, 220]", "sponsor", "skip", "200", "beyond"),
                    seg("[70, 80]", "sponsor", "skip", "230", "stale"),
                    seg("[-5, 3]", "sponsor", "skip", "200", "neg"),
                    seg("[1]", "sponsor", "skip", "200", "short"),
                ],
            ),
            (
                OTHER,
                vec![seg("[1, 2]", "sponsor", "skip", "200", "other")],
            ),
        ]);
        let got = parse_response(&json, &vid(ID), REAL).expect("parses");
        let ids: Vec<&str> = got.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["u1", "u2", "u3"]);
        assert_eq!(got[0].category, SegmentCategory::MusicOffTopic);
        assert_eq!(got[1].end, Duration::from_secs_f64(120.5));
        assert_eq!(got[2].end, REAL);
    }

    #[test]
    fn other_videos_sharing_the_prefix_are_ignored() {
        let json = body(&[(OTHER, vec![seg("[1, 2]", "sponsor", "skip", "200", "x")])]);
        assert_eq!(parse_response(&json, &vid(ID), REAL), Ok(Vec::new()));
    }

    #[test]
    fn garbage_is_a_typed_error() {
        for bad in ["", "nope", "{}", r#"[{"segments":[]}]"#, "[1,2]"] {
            assert!(
                matches!(
                    parse_response(bad, &vid(ID), REAL),
                    Err(SegmentError::Parse(_))
                ),
                "{bad:?}"
            );
        }
        assert_eq!(parse_response("[]", &vid(ID), REAL), Ok(Vec::new()));
    }

    struct Fake {
        reply: Result<HttpResponse, String>,
        seen: Mutex<Vec<String>>,
    }

    impl Fake {
        fn new(status: u16, body: &str) -> Self {
            Self {
                reply: Ok(HttpResponse {
                    status,
                    body: body.to_owned(),
                    retry_after: Some(Duration::from_secs(7)),
                }),
                seen: Mutex::new(Vec::new()),
            }
        }
    }

    impl HttpGet for Fake {
        fn get<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<HttpResponse, String>> {
            self.seen.lock().expect("lock").push(url.to_owned());
            let reply = self.reply.clone();
            Box::pin(async move { reply })
        }
    }

    fn run(source: &SponsorBlockSource<Fake>) -> Result<Vec<Segment>, SegmentError> {
        let id = vid(ID);
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime builds")
            .block_on(source.segments(&id, REAL))
    }

    #[test]
    fn source_returns_filtered_segments_and_sends_only_the_prefix() {
        let json = body(&[(ID, vec![seg("[10, 20]", "sponsor", "skip", "200", "u1")])]);
        let source = SponsorBlockSource::new(Fake::new(200, &json));
        let got = run(&source).expect("ok");
        assert_eq!(got.len(), 1);
        let seen = source.http.seen.lock().expect("lock");
        assert_eq!(seen.len(), 1);
        assert!(seen[0].contains("/ca2f?"));
        assert!(!seen[0].contains(ID));
    }

    #[test]
    fn status_404_is_no_segments() {
        let source = SponsorBlockSource::new(Fake::new(404, "Not Found"));
        assert_eq!(run(&source), Ok(Vec::new()));
    }

    #[test]
    fn status_429_is_rate_limited_with_retry_after() {
        let source = SponsorBlockSource::new(Fake::new(429, ""));
        assert_eq!(
            run(&source),
            Err(SegmentError::RateLimited {
                retry_after: Some(Duration::from_secs(7))
            })
        );
    }

    #[test]
    fn other_statuses_and_transport_errors_are_network_errors() {
        let source = SponsorBlockSource::new(Fake::new(500, ""));
        assert!(matches!(run(&source), Err(SegmentError::Network(_))));
        let mut failing = Fake::new(200, "");
        failing.reply = Err("connection refused".to_owned());
        let source = SponsorBlockSource::new(failing);
        assert!(matches!(run(&source), Err(SegmentError::Network(_))));
    }

    mod props {
        use super::{ID, REAL, vid};
        use crate::sponsorblock::parse_response;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn parse_response_never_panics(s in ".*") {
                let _ = parse_response(&s, &vid(ID), REAL);
            }
        }
    }
}
