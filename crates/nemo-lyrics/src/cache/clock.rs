//! The clock the cache reads, so tests can move time.

use std::time::{SystemTime, UNIX_EPOCH};

/// Source of the current time, so tests can move it.
pub trait Clock: Send + Sync {
    /// Seconds since the Unix epoch.
    fn now_secs(&self) -> i64;
}

/// The system clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_secs(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
    }
}
