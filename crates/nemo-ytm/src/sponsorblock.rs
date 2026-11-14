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

use crate::http::{InitError, USER_AGENT, https_redirects, parse_retry_after};

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

pub(crate) fn sha256_hex(data: &[u8]) -> String {
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

/// Minimal HTTP GET, so tests can inject a fake. [`ReqwestHttp`] is the real implementation.
pub trait HttpGet: Send + Sync {
    /// GET one URL. The error text must not contain the URL.
    fn get<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<HttpResponse, String>>;
}

/// Longest answer body accepted, in bytes. Real answers are a few KiB.
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
/// Total time one lookup may take (app-mvp addendum: 3 s).
pub const HTTP_TIMEOUT: Duration = Duration::from_secs(3);

/// [`HttpGet`] over `reqwest`, run on the app's tokio runtime.
///
/// HTTPS only (also across redirects), `User-Agent` as in the app-mvp addendum, 3 s timeout, and
/// `Retry-After` (seconds) read from every answer. Error texts never contain the URL. The future
/// it returns can be polled by any executor.
#[derive(Debug, Clone)]
pub struct ReqwestHttp {
    client: reqwest::Client,
    handle: tokio::runtime::Handle,
}

impl ReqwestHttp {
    /// Build the client. `handle` is the runtime that performs the requests.
    ///
    /// # Errors
    /// [`InitError::Client`] when the TLS backend cannot start.
    pub fn new(handle: tokio::runtime::Handle) -> Result<Self, InitError> {
        Self::build(handle, true)
    }

    fn build(handle: tokio::runtime::Handle, https_only: bool) -> Result<Self, InitError> {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(HTTP_TIMEOUT)
            .redirect(https_redirects())
            .https_only(https_only)
            .build()
            .map_err(|_| InitError::Client)?;
        Ok(Self { client, handle })
    }
}

async fn fetch(client: reqwest::Client, url: String) -> Result<HttpResponse, String> {
    let mut response = client.get(&url).send().await.map_err(|e| {
        if e.is_timeout() {
            "request timed out".to_owned()
        } else {
            "request failed".to_owned()
        }
    })?;
    let status = response.status().as_u16();
    let retry_after = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_retry_after);
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "could not read the answer".to_owned())?
    {
        if body.len() + chunk.len() > MAX_BODY_BYTES {
            return Err("answer too large".to_owned());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(HttpResponse {
        status,
        body: String::from_utf8_lossy(&body).into_owned(),
        retry_after,
    })
}

impl HttpGet for ReqwestHttp {
    fn get<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<HttpResponse, String>> {
        let client = self.client.clone();
        let url = url.to_owned();
        let task = self.handle.spawn(fetch(client, url));
        Box::pin(async move {
            task.await
                .unwrap_or_else(|_| Err("request was cancelled".to_owned()))
        })
    }
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
mod tests;

#[cfg(test)]
mod reqwest_tests;
