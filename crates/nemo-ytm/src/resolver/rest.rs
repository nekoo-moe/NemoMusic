//! When the native path rests: the accounting behind [`FallbackResolver`](super::FallbackResolver).
//!
//! Two kinds of native failure are told apart:
//! - A request that failed (an HTTP error, a timeout, an unreadable answer): after
//!   [`PRIMARY_MISS_LIMIT`] in a row the native path rests for [`PRIMARY_PAUSE`].
//! - A refusal of one video ([`StreamError::NotAvailable`]: the player answer said the video
//!   cannot be played for this client, which includes YouTube's "confirm you are not a bot"
//!   answer): the request itself worked, so it does not count as a failed request. After
//!   [`PRIMARY_REFUSAL_LIMIT`] refusals in a row the native path rests for the shorter
//!   [`PRIMARY_REFUSAL_PAUSE`].
//!
//! Measured 2026-11-19, within one minute from one machine: one video resolved natively in
//! 66 ms while two others were refused with the bot check. Resting the whole native path after
//! two refusals sent every later song through yt-dlp (3.5 s for a resolve with the onedir
//! build) for 10 minutes. Nothing here logs a URL or a video id.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use nemo_core::stream::{ResolvedStream, StreamError};

/// Failed native requests in a row after which the native path rests.
pub const PRIMARY_MISS_LIMIT: u32 = 2;
/// How long the native path rests after [`PRIMARY_MISS_LIMIT`] failed requests.
pub const PRIMARY_PAUSE: Duration = Duration::from_mins(10);
/// Refusals (different videos) in a row after which the native path rests.
pub const PRIMARY_REFUSAL_LIMIT: u32 = 5;
/// How long the native path rests after [`PRIMARY_REFUSAL_LIMIT`] refusals.
pub const PRIMARY_REFUSAL_PAUSE: Duration = Duration::from_mins(2);

/// What one native resolve came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Outcome {
    /// A stream.
    Answered,
    /// The answer said this video cannot be played (a bot check, an unplayable video).
    Refused,
    /// The request failed.
    Failed,
}

impl Outcome {
    pub(super) fn of(answer: &Result<ResolvedStream, StreamError>) -> Self {
        match answer {
            Ok(_) => Self::Answered,
            Err(StreamError::NotAvailable) => Self::Refused,
            Err(_) => Self::Failed,
        }
    }
}

/// Counters of the native path and the moment until which it rests.
#[derive(Debug, Default)]
pub(super) struct Rest {
    /// Failed requests in a row.
    misses: AtomicU32,
    /// Refusals in a row.
    refusals: AtomicU32,
    /// The native path is not asked before this moment.
    until: Mutex<Option<Instant>>,
}

impl Rest {
    /// `true` while the native path rests.
    pub(super) fn resting(&self) -> bool {
        self.until
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some_and(|until| Instant::now() < until)
    }

    /// Count one native resolve.
    pub(super) fn record(&self, outcome: Outcome) {
        match outcome {
            Outcome::Answered => {
                self.misses.store(0, Ordering::SeqCst);
                self.refusals.store(0, Ordering::SeqCst);
            }
            // The request worked: the client is answering, so no failed request in a row.
            Outcome::Refused => {
                self.misses.store(0, Ordering::SeqCst);
                self.count(&self.refusals, PRIMARY_REFUSAL_LIMIT, PRIMARY_REFUSAL_PAUSE);
            }
            Outcome::Failed => self.count(&self.misses, PRIMARY_MISS_LIMIT, PRIMARY_PAUSE),
        }
    }

    fn count(&self, counter: &AtomicU32, limit: u32, pause: Duration) {
        if counter.fetch_add(1, Ordering::SeqCst) + 1 >= limit {
            counter.store(0, Ordering::SeqCst);
            let until = Instant::now() + pause;
            let mut slot = self.until.lock().unwrap_or_else(PoisonError::into_inner);
            // A longer rest that is already running is not shortened.
            if slot.is_none_or(|old| old < until) {
                *slot = Some(until);
            }
            tracing::info!("native resolver rests after repeated refusals");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_refusals_in_a_row_rest_the_path_and_a_stream_in_between_starts_the_count_over() {
        let rest = Rest::default();
        for _ in 0..PRIMARY_REFUSAL_LIMIT - 1 {
            rest.record(Outcome::Refused);
        }
        assert!(!rest.resting(), "one refusal short of the limit");
        rest.record(Outcome::Answered);
        for _ in 0..PRIMARY_REFUSAL_LIMIT - 1 {
            rest.record(Outcome::Refused);
        }
        assert!(!rest.resting(), "the stream reset the count");
        rest.record(Outcome::Refused);
        assert!(rest.resting());
    }

    #[test]
    fn two_failed_requests_in_a_row_rest_the_path_and_a_refusal_between_them_does_not() {
        let rest = Rest::default();
        rest.record(Outcome::Failed);
        rest.record(Outcome::Refused);
        rest.record(Outcome::Failed);
        assert!(!rest.resting(), "a refusal proves the client answers");
        rest.record(Outcome::Failed);
        assert!(rest.resting());
    }

    #[test]
    fn a_short_rest_never_cuts_a_long_one() {
        let rest = Rest::default();
        rest.record(Outcome::Failed);
        rest.record(Outcome::Failed);
        let long = rest.until.lock().expect("lock").expect("resting");
        for _ in 0..PRIMARY_REFUSAL_LIMIT {
            rest.record(Outcome::Refused);
        }
        assert!(rest.until.lock().expect("lock").expect("resting") >= long);
    }

    #[test]
    fn only_not_available_counts_as_a_refusal() {
        let none: Result<ResolvedStream, StreamError> = Err(StreamError::NotAvailable);
        assert_eq!(Outcome::of(&none), Outcome::Refused);
        let net = Err(StreamError::Network("HTTP 429".to_owned()));
        assert_eq!(Outcome::of(&net), Outcome::Failed);
        let parse = Err(StreamError::Parse("not json".to_owned()));
        assert_eq!(Outcome::of(&parse), Outcome::Failed);
    }
}
