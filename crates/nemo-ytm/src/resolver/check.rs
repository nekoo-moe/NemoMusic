//! The one cheap request that decides whether a stream kept from an earlier run is reused.

use std::time::Duration;

use nemo_core::BoxFuture;
use nemo_core::stream::ResolvedStream;

use crate::http::{InitError, https_redirects};

/// Longest the check may take. A slow answer counts as a dead address.
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(4);

/// Tells whether the address of a kept stream still answers.
pub trait UrlCheck: Send + Sync {
    /// `true` when `stream` can be read. Never logs the address.
    fn alive<'a>(&'a self, stream: &'a ResolvedStream) -> BoxFuture<'a, bool>;
}

/// The real check: a range request for the first byte, HTTPS only.
#[derive(Debug, Clone)]
pub struct HttpUrlCheck {
    client: reqwest::Client,
}

impl HttpUrlCheck {
    /// Build the check's HTTP client.
    ///
    /// # Errors
    /// [`InitError::Client`] when the client cannot be built.
    pub fn new() -> Result<Self, InitError> {
        let client = reqwest::Client::builder()
            .connect_timeout(CHECK_TIMEOUT)
            .timeout(CHECK_TIMEOUT)
            .redirect(https_redirects())
            .https_only(true)
            .build()
            .map_err(|_| InitError::Client)?;
        Ok(Self { client })
    }
}

impl UrlCheck for HttpUrlCheck {
    fn alive<'a>(&'a self, stream: &'a ResolvedStream) -> BoxFuture<'a, bool> {
        Box::pin(async move {
            let sent = self
                .client
                .get(stream.url.expose())
                .header(reqwest::header::RANGE, "bytes=0-0")
                .send()
                .await;
            // 200 or 206: it answers. 403 and the rest: the address is dead (the error text
            // can hold the address, so it is not looked at).
            sent.is_ok_and(|r| matches!(r.status().as_u16(), 200 | 206))
        })
    }
}
