//! Skippable segments of music videos and the source trait (ADR 0003).
//!
//! Skipping is a seek only: no crossfade, no gain change, no DSP (bit-perfect rule).

use std::time::Duration;

use crate::BoxFuture;
use crate::track::VideoId;

/// Kind of segment, named after the SponsorBlock categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SegmentCategory {
    /// Paid promotion.
    Sponsor,
    /// Self promotion.
    SelfPromo,
    /// Reminder to like or subscribe.
    Interaction,
    /// Intro without content.
    Intro,
    /// Outro or end cards.
    Outro,
    /// Preview or recap.
    Preview,
    /// Hook shown before the real start.
    Hook,
    /// Tangent or filler. Aggressive, so off by default.
    Filler,
    /// Non-music section of a music video.
    MusicOffTopic,
}

/// A span of a track that can be skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// Where the span starts.
    pub start: Duration,
    /// Where the span ends.
    pub end: Duration,
    /// What kind of span it is.
    pub category: SegmentCategory,
    /// Source identifier of the segment, for undo and de-duplication.
    pub id: String,
}

/// A segment lookup failed. Callers treat this as "no segments"; playback never stops for it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SegmentError {
    /// The service asked us to slow down.
    #[error("rate limited by segment service")]
    RateLimited {
        /// How long to wait, if the service says.
        retry_after: Option<Duration>,
    },
    /// The network request failed.
    #[error("network error: {0}")]
    Network(String),
    /// The answer could not be parsed.
    #[error("could not parse segment answer: {0}")]
    Parse(String),
}

/// A source of skippable segments.
pub trait SegmentSource: Send + Sync {
    /// Segments for one video. Empty when there are none or the service is offline.
    /// `duration` is the real track duration, used to drop outdated segments.
    fn segments<'a>(
        &'a self,
        video: &'a VideoId,
        duration: Duration,
    ) -> BoxFuture<'a, Result<Vec<Segment>, SegmentError>>;
}
