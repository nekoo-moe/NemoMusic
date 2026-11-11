//! Small HTTP helpers shared by the web providers (everything except LRCLIB, which predates
//! them).
//!
//! Rules every provider here follows: an identifying `User-Agent`, a short timeout, a body size
//! cap, no URL in errors or logs (the URL carries the track metadata), `Retry-After` honored.

use std::time::Duration;

use nemo_core::lyrics::LyricsError;
use reqwest::{RequestBuilder, Url};

/// `User-Agent` sent to every lyrics host.
pub const USER_AGENT: &str = crate::lrclib::USER_AGENT;

/// Total time for one HTTP request.
pub const REQUEST_TIMEOUT: Duration = Duration::from_millis(3500);

/// Answers larger than this are not lyrics.
pub const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

/// Build a client with the project `User-Agent` and a short timeout.
///
/// # Errors
/// [`LyricsError::Unavailable`] if the client cannot be built.
pub fn build_client() -> Result<reqwest::Client, LyricsError> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(REQUEST_TIMEOUT)
        .connect_timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| LyricsError::Unavailable(e.without_url().to_string()))
}

/// Build a URL with query parameters.
///
/// # Errors
/// [`LyricsError::Unavailable`] if `base` is not a URL.
pub fn url_with(base: &str, params: &[(&str, &str)]) -> Result<Url, LyricsError> {
    Url::parse_with_params(base, params).map_err(|e| LyricsError::Unavailable(e.to_string()))
}

/// Parse a `Retry-After` value in seconds.
#[must_use]
pub fn parse_retry_after(value: Option<&str>) -> Option<Duration> {
    value?.trim().parse::<u64>().ok().map(Duration::from_secs)
}

/// Map an HTTP status to an error, or `None` when the caller should look at the status itself.
/// 429 and 503 mean slow down; other 5xx are network errors.
#[must_use]
pub fn status_error(status: u16, retry_after: Option<&str>) -> Option<LyricsError> {
    match status {
        429 | 503 => Some(LyricsError::RateLimited {
            retry_after: parse_retry_after(retry_after),
        }),
        500..=599 => Some(LyricsError::Network(format!("HTTP status {status}"))),
        _ => None,
    }
}

/// Send a request. Returns the status and, for a 2xx answer, the body (empty otherwise).
///
/// # Errors
/// [`LyricsError::RateLimited`] on 429 and 503, [`LyricsError::Network`] on other failures,
/// [`LyricsError::Parse`] when the body is too large or not text.
pub async fn send_text(req: RequestBuilder) -> Result<(u16, String), LyricsError> {
    let resp = req
        .send()
        .await
        .map_err(|e| LyricsError::Network(e.without_url().to_string()))?;
    let status = resp.status().as_u16();
    let retry_after = resp
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    if let Some(e) = status_error(status, retry_after.as_deref()) {
        return Err(e);
    }
    if !(200..300).contains(&status) {
        return Ok((status, String::new()));
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
    let body = String::from_utf8(bytes.to_vec())
        .map_err(|_| LyricsError::Parse("answer is not UTF-8 text".to_owned()))?;
    Ok((status, body))
}

/// Whole seconds of the query duration, rounded, if it is known and sensible (1 s to 1 h).
#[must_use]
pub fn duration_secs(duration_ms: Option<u64>) -> Option<u64> {
    let secs = duration_ms?.saturating_add(500) / 1000;
    (1..=3600).contains(&secs).then_some(secs)
}

/// Split a credit line such as `Justice, Connan Mockasin` into artist names.
#[must_use]
pub fn split_artists(credit: &str) -> Vec<String> {
    credit
        .split([',', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses() {
        assert!(matches!(
            status_error(429, Some("7")),
            Some(LyricsError::RateLimited { retry_after: Some(d) }) if d.as_secs() == 7
        ));
        assert!(matches!(
            status_error(503, None),
            Some(LyricsError::RateLimited { retry_after: None })
        ));
        assert!(matches!(
            status_error(502, None),
            Some(LyricsError::Network(_))
        ));
        assert_eq!(status_error(404, None), None);
        assert_eq!(status_error(200, None), None);
    }

    #[test]
    fn helpers() {
        assert_eq!(duration_secs(Some(249_400)), Some(249));
        assert_eq!(duration_secs(Some(0)), None);
        assert_eq!(duration_secs(None), None);
        assert_eq!(split_artists("A, B & C"), ["A", "B & C"]);
    }
}
