//! The native SoundCloud resolver with `yt-dlp` behind it (soundcloud-full task 14).
//!
//! `yt-dlp` is asked only when the native lookup could not read what SoundCloud answered
//! ([`StreamError::Parse`]: the web app changed). It is never asked about a refusal: an
//! encrypted-only track, a preview, a blocked or removed track, a rate limit and a network
//! failure all keep their own answer, so the fallback cannot become a way around them
//! (`coc.md`). When both fail, the native error is the one reported.

use std::sync::Arc;

use nemo_core::BoxFuture;
use nemo_core::stream::{ResolvedStream, StreamError};
use nemo_core::track::MediaUrl;
use nemo_core::url_resolver::{FormatPolicy, MediaEntry, MediaQuery, ResolverState, UrlResolver};

/// Native first, `yt-dlp` when the native lookup does not understand SoundCloud's answer.
pub struct FallbackResolver {
    native: Arc<dyn UrlResolver>,
    fallback: Arc<dyn UrlResolver>,
}

impl FallbackResolver {
    /// Chain two resolvers.
    pub fn new(native: Arc<dyn UrlResolver>, fallback: Arc<dyn UrlResolver>) -> Self {
        Self { native, fallback }
    }
}

impl UrlResolver for FallbackResolver {
    fn resolve<'a>(
        &'a self,
        url: &'a MediaUrl,
        policy: &'a FormatPolicy,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        Box::pin(async move {
            let first = self.native.resolve(url, policy).await;
            let Err(error @ StreamError::Parse(_)) = first else {
                return first;
            };
            tracing::debug!("the native SoundCloud lookup failed; trying yt-dlp");
            self.fallback.resolve(url, policy).await.map_err(|_| error)
        })
    }

    fn list<'a>(
        &'a self,
        query: &'a MediaQuery,
    ) -> BoxFuture<'a, Result<Vec<MediaEntry>, StreamError>> {
        self.native.list(query)
    }

    fn state(&self) -> ResolverState {
        self.native.state()
    }

    fn warm(&self, url: &MediaUrl) {
        self.native.warm(url);
    }

    fn stream_failed(&self, url: &MediaUrl) {
        self.native.stream_failed(url);
        self.fallback.stream_failed(url);
    }
}

#[cfg(test)]
mod tests;
