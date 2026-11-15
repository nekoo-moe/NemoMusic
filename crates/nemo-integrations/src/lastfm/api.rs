//! The signed Last.fm API client (criteria 13, 21 and 29).
//!
//! Every call is a `POST` of form fields to the API root, signed with [`sign`]: `api_sig` is the
//! MD5 of the parameters sorted by name, each written as `name` then `value`, followed by the
//! user's API secret. The answer is read as JSON (`format=json`) whatever the HTTP status,
//! because Last.fm reports its own errors in the body (`{"error": 9, ...}`).
//!
//! Only HTTPS is used: the session key travels in the body, so a plain `http` address is
//! refused, and the HTTP client itself is HTTPS-only. Unit tests may use plain HTTP on
//! `127.0.0.1` to reach a local test server; nothing else can.
//!
//! The key, the secret and the session key are [`Secret`]s. Request bodies, answers, song
//! titles and artists never reach a log; the only log lines name the API method, the HTTP
//! status or the Last.fm error code.

use std::fmt::Write as _;
use std::time::Duration;

use md5::{Digest, Md5};
use nemo_core::secret::Secret;
use reqwest::Url;
use serde_json::Value;

use crate::lastfm::queue::{BATCH, NewScrobble};

/// The Last.fm API root. Its host is the one in the Last.fm settings row
/// ([`crate::info::LASTFM`]) and the README egress list.
pub const BASE_URL: &str = "https://ws.audioscrobbler.com/2.0/";

/// Total time for one call.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// `User-Agent` sent to Last.fm.
pub const USER_AGENT: &str = "NemoMusic/0.1 (+https://github.com/nekoo-moe/NemoMusic)";

/// Answers larger than this are not Last.fm answers.
const MAX_BODY_BYTES: usize = 256 * 1024;

/// Parameter names that are not signed (Last.fm API authentication spec).
const UNSIGNED: [&str; 2] = ["format", "callback"];

/// Why a Last.fm call failed. No variant carries a key, a body or a song.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    /// The API address is not `https` (plain HTTP would expose the session key).
    #[error("the Last.fm address is not HTTPS")]
    Insecure,
    /// The API address is not a URL.
    #[error("the Last.fm address is not a valid URL")]
    BadUrl,
    /// More than [`BATCH`] scrobbles in one call.
    #[error("too many scrobbles in one call")]
    BatchTooLarge,
    /// No answer, a transport error, or an HTTP error without a Last.fm error in the body.
    #[error("Last.fm could not be reached: {0}")]
    Network(String),
    /// Last.fm answered with its own error code (for example 9, invalid session; 11, service
    /// offline; 16, temporary error).
    #[error("Last.fm refused the call (error {0})")]
    Api(u32),
    /// The answer is not the JSON Last.fm sends.
    #[error("Last.fm sent an answer that is not understood")]
    Parse,
}

/// What Last.fm did with one `track.scrobble` batch. An ignored scrobble (for example one older
/// than about two weeks) was received and must not be sent again.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScrobbleOutcome {
    /// Scrobbles Last.fm recorded.
    pub accepted: u32,
    /// Scrobbles Last.fm received and ignored.
    pub ignored: u32,
}

/// The track for `track.updateNowPlaying`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NowPlaying {
    /// Track artist.
    pub artist: String,
    /// Track title.
    pub track: String,
    /// Album title, when known.
    pub album: Option<String>,
    /// Track length in seconds, when known.
    pub duration_s: Option<u32>,
}

/// The `api_sig` of a call: MD5, as 32 lowercase hex digits, over every parameter except
/// `format` and `callback`, sorted by name in byte order (so `artist[10]` comes before
/// `artist[2]`), each written as name then value in UTF-8, followed by `secret`.
#[must_use]
pub fn sign(params: &[(&str, &str)], secret: &str) -> String {
    let mut signed: Vec<&(&str, &str)> = params
        .iter()
        .filter(|(name, _)| !UNSIGNED.contains(name))
        .collect();
    signed.sort_by(|a, b| a.0.cmp(b.0));
    let mut md5 = Md5::new();
    for (name, value) in signed {
        md5.update(name.as_bytes());
        md5.update(value.as_bytes());
    }
    md5.update(secret.as_bytes());
    md5.finalize()
        .iter()
        .fold(String::with_capacity(32), |mut hex, byte| {
            // Writing to a `String` cannot fail.
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

/// Whether `url` may carry a Last.fm call: `https`, or plain `http` to `127.0.0.1` in this
/// crate's unit tests.
fn address_allowed(url: &Url) -> bool {
    url.scheme() == "https"
        || (cfg!(test) && url.scheme() == "http" && url.host_str() == Some("127.0.0.1"))
}

fn network(e: reqwest::Error) -> ApiError {
    ApiError::Network(e.without_url().to_string())
}

/// The Last.fm error code in an answer, as a number or a numeric string.
fn error_code(answer: &Value) -> Option<u32> {
    let code = answer.get("error")?;
    let code = code
        .as_u64()
        .or_else(|| code.as_str().and_then(|s| s.trim().parse().ok()))?;
    Some(u32::try_from(code).unwrap_or(u32::MAX))
}

/// A count in an answer, as a number or a numeric string.
fn count(value: Option<&Value>) -> Option<u32> {
    let value = value?;
    let n = value
        .as_u64()
        .or_else(|| value.as_str().and_then(|s| s.trim().parse().ok()))?;
    u32::try_from(n).ok()
}

/// The track fields of a call, borrowed from a [`NowPlaying`] or a [`NewScrobble`].
struct TrackFields<'a> {
    artist: &'a str,
    track: &'a str,
    album: Option<&'a str>,
    duration_s: Option<u32>,
}

impl<'a> From<&'a NowPlaying> for TrackFields<'a> {
    fn from(n: &'a NowPlaying) -> Self {
        Self {
            artist: &n.artist,
            track: &n.track,
            album: n.album.as_deref(),
            duration_s: n.duration_s,
        }
    }
}

impl<'a> From<&'a NewScrobble> for TrackFields<'a> {
    fn from(s: &'a NewScrobble) -> Self {
        Self {
            artist: &s.artist,
            track: &s.track,
            album: s.album.as_deref(),
            duration_s: s.duration_s,
        }
    }
}

/// Append `artist`, `track`, and when known `album` and `duration`, each name followed by
/// `suffix` (`""` or `"[i]"`).
fn push_track(params: &mut Vec<(String, String)>, suffix: &str, t: &TrackFields<'_>) {
    params.push((format!("artist{suffix}"), t.artist.to_owned()));
    params.push((format!("track{suffix}"), t.track.to_owned()));
    if let Some(album) = t.album.filter(|a| !a.is_empty()) {
        params.push((format!("album{suffix}"), album.to_owned()));
    }
    if let Some(duration) = t.duration_s {
        params.push((format!("duration{suffix}"), duration.to_string()));
    }
}

/// A Last.fm API client for one user's API account.
#[derive(Debug)]
pub struct LastFmApi {
    http: reqwest::Client,
    base: Url,
    key: Secret<String>,
    secret: Secret<String>,
}

impl LastFmApi {
    /// A client that calls `base` (normally [`BASE_URL`]) with the user's API key and secret.
    ///
    /// # Errors
    /// [`ApiError::BadUrl`] when `base` is not a URL, [`ApiError::Insecure`] when it is not
    /// `https`, [`ApiError::Network`] when the HTTP client cannot be built.
    pub fn new(base: &str, key: Secret<String>, secret: Secret<String>) -> Result<Self, ApiError> {
        let base = Url::parse(base).map_err(|_| ApiError::BadUrl)?;
        if !address_allowed(&base) {
            return Err(ApiError::Insecure);
        }
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(REQUEST_TIMEOUT)
            .https_only(!cfg!(test))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(network)?;
        Ok(Self {
            http,
            base,
            key,
            secret,
        })
    }

    /// Tell Last.fm what is playing now (`track.updateNowPlaying`). The caller does not retry a
    /// failure (criterion 13).
    ///
    /// # Errors
    /// See [`ApiError`].
    pub async fn update_now_playing(
        &self,
        session: &Secret<String>,
        now: &NowPlaying,
    ) -> Result<(), ApiError> {
        let mut params = Vec::with_capacity(4);
        push_track(&mut params, "", &now.into());
        self.call("track.updateNowPlaying", params, Some(session))
            .await
            .map(|_| ())
    }

    /// Send up to [`BATCH`] finished plays, oldest first (`track.scrobble`). An empty batch
    /// sends nothing.
    ///
    /// # Errors
    /// [`ApiError::BatchTooLarge`] over [`BATCH`] rows (nothing is sent); otherwise see
    /// [`ApiError`].
    pub async fn scrobble(
        &self,
        session: &Secret<String>,
        batch: &[NewScrobble],
    ) -> Result<ScrobbleOutcome, ApiError> {
        if batch.is_empty() {
            return Ok(ScrobbleOutcome::default());
        }
        if batch.len() > BATCH {
            return Err(ApiError::BatchTooLarge);
        }
        let mut params = Vec::with_capacity(batch.len() * 5);
        for (i, s) in batch.iter().enumerate() {
            let suffix = format!("[{i}]");
            push_track(&mut params, &suffix, &s.into());
            params.push((format!("timestamp{suffix}"), s.ts.to_string()));
        }
        let answer = self.call("track.scrobble", params, Some(session)).await?;
        let attr = answer.get("scrobbles").and_then(|s| s.get("@attr"));
        let accepted = count(attr.and_then(|a| a.get("accepted"))).ok_or(ApiError::Parse)?;
        let ignored = count(attr.and_then(|a| a.get("ignored"))).ok_or(ApiError::Parse)?;
        Ok(ScrobbleOutcome { accepted, ignored })
    }

    /// One signed call. Adds `method`, `api_key`, `sk` (when given), `api_sig` and
    /// `format=json` to `params`, posts them as a form and returns the JSON answer.
    async fn call(
        &self,
        method: &str,
        mut params: Vec<(String, String)>,
        session: Option<&Secret<String>>,
    ) -> Result<Value, ApiError> {
        params.push(("method".to_owned(), method.to_owned()));
        params.push(("api_key".to_owned(), self.key.expose().clone()));
        if let Some(session) = session {
            params.push(("sk".to_owned(), session.expose().clone()));
        }
        let mut fields: Vec<(&str, &str)> = params
            .iter()
            .map(|(n, v)| (n.as_str(), v.as_str()))
            .collect();
        let sig = sign(&fields, self.secret.expose());
        fields.push(("api_sig", &sig));
        fields.push(("format", "json"));

        let resp = self
            .http
            .post(self.base.clone())
            .form(&fields)
            .send()
            .await
            .map_err(network)?;
        let status = resp.status().as_u16();
        tracing::debug!(method, status, "Last.fm answered");
        if resp
            .content_length()
            .is_some_and(|n| n > MAX_BODY_BYTES as u64)
        {
            return Err(ApiError::Parse);
        }
        let body = resp.bytes().await.map_err(network)?;
        if body.len() > MAX_BODY_BYTES {
            return Err(ApiError::Parse);
        }
        read_answer(status, &body).inspect_err(|e| {
            if let ApiError::Api(code) = e {
                tracing::debug!(method, code, "Last.fm refused the call");
            }
        })
    }
}

/// Turn an HTTP status and body into the JSON answer or an error. A Last.fm error in the body
/// wins over the status.
fn read_answer(status: u16, body: &[u8]) -> Result<Value, ApiError> {
    let ok = (200..300).contains(&status);
    match serde_json::from_slice::<Value>(body) {
        Ok(answer) => match error_code(&answer) {
            Some(code) => Err(ApiError::Api(code)),
            None if ok => Ok(answer),
            None => Err(ApiError::Network(format!("HTTP status {status}"))),
        },
        Err(_) if ok => Err(ApiError::Parse),
        Err(_) => Err(ApiError::Network(format!("HTTP status {status}"))),
    }
}

#[cfg(test)]
mod tests;
