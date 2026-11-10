//! Small HTTP helpers shared by the network parts of this crate.
//!
//! Nothing here logs a URL. Error values carry fixed texts only, because `reqwest` error text
//! can contain the request URL, and stream URLs are secrets.

use std::time::Duration;

use reqwest::redirect::Policy;

/// `User-Agent` sent to non-YouTube hosts (app-mvp addendum).
pub const USER_AGENT: &str = "NemoMusic/0.0.1 (+https://github.com/nekoo-moe/NemoMusic)";

/// Most redirects a request may follow.
pub const MAX_REDIRECTS: usize = 3;

/// Building a network service failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InitError {
    /// The HTTP client could not be built (TLS backend missing, for example).
    #[error("could not build the HTTP client")]
    Client,
    /// The cache directory could not be created.
    #[error("could not create the cache directory")]
    CacheDir,
}

/// Follow a redirect only when it stays on HTTPS, and never more than [`MAX_REDIRECTS`] times.
pub(crate) fn https_redirects() -> Policy {
    Policy::custom(|attempt| {
        if attempt.url().scheme() == "https" && attempt.previous().len() < MAX_REDIRECTS {
            attempt.follow()
        } else {
            attempt.stop()
        }
    })
}

/// True when `url` parses and uses the `https` scheme with a host.
pub(crate) fn is_https(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| u.scheme() == "https" && u.host_str().is_some())
}

/// Parse a `Retry-After` header given in seconds. HTTP dates are not supported and give `None`.
pub(crate) fn parse_retry_after(value: &str) -> Option<Duration> {
    value.trim().parse::<u64>().ok().map(Duration::from_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_check() {
        assert!(is_https("https://example.invalid/a?b=c"));
        assert!(!is_https("http://example.invalid/"));
        assert!(!is_https("ftp://example.invalid/"));
        assert!(!is_https("file:///C:/x"));
        assert!(!is_https("https://"));
        assert!(!is_https("not a url"));
        assert!(!is_https(""));
    }

    #[test]
    fn retry_after_seconds_only() {
        assert_eq!(parse_retry_after("30"), Some(Duration::from_secs(30)));
        assert_eq!(parse_retry_after(" 5 "), Some(Duration::from_secs(5)));
        assert_eq!(parse_retry_after("Wed, 21 Oct 2026 07:28:00 GMT"), None);
        assert_eq!(parse_retry_after("-1"), None);
        assert_eq!(parse_retry_after(""), None);
    }
}
