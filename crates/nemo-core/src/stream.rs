//! Resolving a YouTube Music track to a playable stream.

use std::time::SystemTime;

use crate::BoxFuture;
use crate::audio::StreamInfo;
use crate::secret::Secret;
use crate::track::VideoId;

/// A resolved stream. The URL is a secret: it can carry the listener's IP and a signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStream {
    /// Direct audio URL. Never log it.
    pub url: Secret<String>,
    /// Measured facts about the stream, for the quality badge.
    pub info: StreamInfo,
    /// When the URL stops working, if known.
    pub expires_at: Option<SystemTime>,
}

/// Resolving a stream failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StreamError {
    /// The track cannot be played with this account (region, age, Premium only).
    #[error("track not available")]
    NotAvailable,
    /// The resolver program or library is missing.
    #[error("stream resolver is not installed")]
    ResolverMissing,
    /// The network request failed.
    #[error("network error: {0}")]
    Network(String),
    /// The resolver's answer could not be parsed.
    #[error("could not parse resolver answer: {0}")]
    Parse(String),
}

/// Turns a video id into a playable stream.
pub trait StreamResolver: Send + Sync {
    /// Resolve one track. Human-paced: callers must not resolve a whole queue at once.
    fn resolve<'a>(
        &'a self,
        video: &'a VideoId,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>>;
}
