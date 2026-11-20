//! One blocking GET of a playlist or segment, with the retry rules of the stream reader.
//!
//! HTTPS only (also across redirects) on the allowed hosts, a size cap, at most
//! [`SEGMENT_TRIES`] tries, a 429 answered by waiting its `Retry-After` (up to
//! [`MAX_RETRY_AFTER`]). No address is logged or put in an error.

use std::io::Read;
use std::thread;
use std::time::{Duration, Instant};

use nemo_core::track::MediaUrl;
use reqwest::blocking::Client;

use crate::hosts::HOSTS;
use crate::http::{TIMEOUT, USER_AGENT, https_redirects, parse_retry_after};

/// Tries for one request: the first and two more.
pub const SEGMENT_TRIES: usize = 3;
/// The longest a 429 may make the reader wait.
pub const MAX_RETRY_AFTER: Duration = Duration::from_secs(10);
/// Longest segment, init segment or playlist read, in bytes. A ten-second segment of a 320 kbps
/// stream is 400 KB.
pub const MAX_BODY_BYTES: u64 = 16 * 1024 * 1024;

/// Why a fetch gave nothing. No variant holds an address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Failure {
    /// 403, 404 or 410: the address stopped working or the file is gone.
    #[error("the stream address no longer works")]
    Gone,
    /// 429 with a wait that is too long, or after the tries.
    #[error("SoundCloud asked to slow down")]
    Busy,
    /// A connection error, a timeout, a server error, a too large or cut-off answer.
    #[error("could not read the stream")]
    Network,
}

/// Which addresses may be fetched, and the waits between tries. Tests use [`Rules::loopback`].
#[derive(Debug, Clone, Copy)]
pub struct Rules {
    /// Accept `http://127.0.0.1:<port>` instead of the SoundCloud hosts (tests only).
    loopback: bool,
    /// Waits before the second and third try.
    pub retry_delays: [Duration; 2],
}

impl Rules {
    /// The real rules: https on [`HOSTS`], waits of 0.3 s and 1 s.
    #[must_use]
    pub const fn production() -> Self {
        Self {
            loopback: false,
            retry_delays: [Duration::from_millis(300), Duration::from_secs(1)],
        }
    }

    /// Loopback http, no waits.
    #[cfg(test)]
    #[must_use]
    pub const fn loopback() -> Self {
        Self {
            loopback: true,
            retry_delays: [Duration::ZERO, Duration::ZERO],
        }
    }

    /// Whether `url` may be fetched.
    #[must_use]
    pub fn allows(&self, url: &str) -> bool {
        if self.loopback {
            return reqwest::Url::parse(url)
                .is_ok_and(|u| u.scheme() == "http" && u.host_str() == Some("127.0.0.1"));
        }
        MediaUrl::parse(url, HOSTS).is_ok()
    }

    /// `rel` (absolute or relative to `base`) as an allowed absolute address.
    #[must_use]
    pub fn resolve(&self, base: &str, rel: &str) -> Option<String> {
        let joined = reqwest::Url::parse(base).ok()?.join(rel).ok()?;
        let text = String::from(joined);
        self.allows(&text).then_some(text)
    }

    /// The blocking client. Call it from a plain thread, never from an async task.
    ///
    /// # Errors
    /// [`Failure::Network`] when the TLS backend cannot start.
    pub fn client(&self) -> Result<Client, Failure> {
        Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(TIMEOUT)
            .timeout(TIMEOUT)
            .redirect(https_redirects())
            .https_only(!self.loopback)
            .build()
            .map_err(|_| Failure::Network)
    }
}

/// One try: the body of a 200 answer, or what went wrong. `wait` is how long a 429 asks to wait.
fn once(client: &Client, url: &str) -> Result<Vec<u8>, (Failure, Option<Duration>)> {
    let response = client
        .get(url)
        .send()
        .map_err(|_| (Failure::Network, None))?;
    match response.status().as_u16() {
        200 => {
            let mut body = Vec::new();
            response
                .take(MAX_BODY_BYTES + 1)
                .read_to_end(&mut body)
                .map_err(|_| (Failure::Network, None))?;
            if u64::try_from(body.len()).is_ok_and(|n| n > MAX_BODY_BYTES) || body.is_empty() {
                return Err((Failure::Network, None));
            }
            Ok(body)
        }
        403 | 404 | 410 => Err((Failure::Gone, None)),
        429 => {
            let wait = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(parse_retry_after);
            Err((Failure::Busy, wait))
        }
        _ => Err((Failure::Network, None)),
    }
}

/// GET `url`, trying up to [`SEGMENT_TRIES`] times.
///
/// # Errors
/// [`Failure`]: `Gone` at once (a retry cannot help), `Busy` when 429 asks for more than
/// [`MAX_RETRY_AFTER`] or persists, `Network` after the last try.
pub fn get(client: &Client, url: &str, rules: &Rules) -> Result<Vec<u8>, Failure> {
    let mut delays = rules.retry_delays.iter();
    let started = Instant::now();
    let mut attempt = 0_u64;
    loop {
        attempt += 1;
        let tried = once(client, url);
        if let Err((failure, _)) = &tried {
            crate::trace::stream_failed(url, *failure, started, attempt);
        }
        match tried {
            Ok(body) => return Ok(body),
            Err((Failure::Gone, _)) => return Err(Failure::Gone),
            Err((Failure::Busy, wait)) => {
                let wait = wait.unwrap_or(Duration::from_secs(1));
                if wait > MAX_RETRY_AFTER || delays.next().is_none() {
                    return Err(Failure::Busy);
                }
                thread::sleep(wait);
            }
            Err((failure, _)) => match delays.next() {
                Some(delay) => thread::sleep(*delay),
                None => return Err(failure),
            },
        }
    }
}
