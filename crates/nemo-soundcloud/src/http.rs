//! The HTTP seam of this crate: one trait the client calls, the real `reqwest` implementation and
//! the rules every request follows.
//!
//! Nothing here logs a URL. Error values carry fixed texts only, because `reqwest` error text can
//! contain the request URL, and SoundCloud addresses carry the client id and signatures. This is
//! a small copy of the rules in `nemo-ytm/src/http.rs`: crates must not depend on each other.

use std::time::Duration;

use nemo_core::BoxFuture;
use reqwest::redirect::Policy;

/// `User-Agent` of every request. It names the app, as the other non-YouTube hosts get it.
pub const USER_AGENT: &str = "NemoMusic/0.2 (+https://github.com/nekoo-moe/NemoMusic)";

/// Most redirects a request may follow.
pub const MAX_REDIRECTS: usize = 3;

/// Total time one request may take.
pub const TIMEOUT: Duration = Duration::from_secs(15);

/// Longest JSON answer read, in bytes. Real answers are a few hundred KiB.
pub const MAX_JSON_BYTES: usize = 4 * 1024 * 1024;

/// Longest script read when the client id is searched, in bytes. The web app's script that holds
/// it was about 3 MB on 2026-11-19.
pub const MAX_SCRIPT_BYTES: usize = 8 * 1024 * 1024;

/// One GET request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScRequest {
    /// Full address. It can hold the client id: never log it.
    pub url: String,
    /// Longest body to read; a longer answer fails with [`ScHttpError::TooLarge`].
    pub max_bytes: usize,
}

/// A finished answer, whatever its status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScAnswer {
    /// HTTP status code.
    pub status: u16,
    /// Body bytes.
    pub body: Vec<u8>,
    /// `Retry-After` in seconds, if the server sent one.
    pub retry_after: Option<Duration>,
}

/// A request that produced no answer. The texts are fixed and never hold the address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ScHttpError {
    /// The request took longer than [`TIMEOUT`].
    #[error("the request timed out")]
    Timeout,
    /// The connection or the TLS handshake failed, or a redirect left HTTPS.
    #[error("the request failed")]
    Failed,
    /// The answer was longer than the request allowed.
    #[error("the answer is too large")]
    TooLarge,
    /// The request was dropped before it finished.
    #[error("the request was cancelled")]
    Cancelled,
}

/// Building the HTTP client failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("could not build the HTTP client")]
pub struct InitError;

/// One GET, so tests can inject a fake. [`ReqwestHttp`] is the real one.
pub trait ScHttp: Send + Sync {
    /// Send `request` and read the whole body (up to `max_bytes`).
    fn send(&self, request: ScRequest) -> BoxFuture<'_, Result<ScAnswer, ScHttpError>>;
}

/// Follow a redirect only when it stays on HTTPS, and never more than [`MAX_REDIRECTS`] times.
fn https_redirects() -> Policy {
    Policy::custom(|attempt| {
        if attempt.url().scheme() == "https" && attempt.previous().len() < MAX_REDIRECTS {
            attempt.follow()
        } else {
            attempt.stop()
        }
    })
}

/// Parse a `Retry-After` header given in seconds. HTTP dates are not supported and give `None`.
pub(crate) fn parse_retry_after(value: &str) -> Option<Duration> {
    value.trim().parse::<u64>().ok().map(Duration::from_secs)
}

/// [`ScHttp`] over `reqwest`, run on the app's tokio runtime: HTTPS only (also across redirects),
/// [`TIMEOUT`], no cookie store.
#[derive(Debug, Clone)]
pub struct ReqwestHttp {
    client: reqwest::Client,
    handle: tokio::runtime::Handle,
}

impl ReqwestHttp {
    /// Build the client. `handle` is the runtime that performs the requests.
    ///
    /// # Errors
    /// [`InitError`] when the TLS backend cannot start.
    pub fn new(handle: tokio::runtime::Handle) -> Result<Self, InitError> {
        Self::build(handle, true)
    }

    /// Build a client that also accepts plain `http` (loopback test servers only).
    #[cfg(test)]
    pub(crate) fn new_for_tests(handle: tokio::runtime::Handle) -> Result<Self, InitError> {
        Self::build(handle, false)
    }

    fn build(handle: tokio::runtime::Handle, https_only: bool) -> Result<Self, InitError> {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(TIMEOUT)
            .redirect(https_redirects())
            .https_only(https_only)
            .build()
            .map_err(|_| InitError)?;
        Ok(Self { client, handle })
    }
}

async fn fetch(client: reqwest::Client, request: ScRequest) -> Result<ScAnswer, ScHttpError> {
    let mut response = client.get(&request.url).send().await.map_err(|e| {
        if e.is_timeout() {
            ScHttpError::Timeout
        } else {
            ScHttpError::Failed
        }
    })?;
    let status = response.status().as_u16();
    let retry_after = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_retry_after);
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| {
        if e.is_timeout() {
            ScHttpError::Timeout
        } else {
            ScHttpError::Failed
        }
    })? {
        if body.len() + chunk.len() > request.max_bytes {
            return Err(ScHttpError::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(ScAnswer {
        status,
        body,
        retry_after,
    })
}

impl ScHttp for ReqwestHttp {
    fn send(&self, request: ScRequest) -> BoxFuture<'_, Result<ScAnswer, ScHttpError>> {
        let task = self.handle.spawn(fetch(self.client.clone(), request));
        Box::pin(async move { task.await.unwrap_or(Err(ScHttpError::Cancelled)) })
    }
}

#[cfg(test)]
mod tests;
