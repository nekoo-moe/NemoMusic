//! At most one update per slot, the newest winning (criterion 14).

use std::time::{Duration, Instant};

/// Holds back items so that no two are released less than `gap` apart. While one waits, a newer
/// item replaces it. The clock is a parameter, so a test needs no sleep.
#[derive(Debug, Clone)]
pub struct RateLimiter<T> {
    gap: Duration,
    sent_at: Option<Instant>,
    pending: Option<T>,
}

impl<T> RateLimiter<T> {
    /// A limiter that releases the first item at once.
    #[must_use]
    pub const fn new(gap: Duration) -> Self {
        Self {
            gap,
            sent_at: None,
            pending: None,
        }
    }

    fn due(&self, now: Instant) -> bool {
        self.sent_at
            .is_none_or(|at| now.saturating_duration_since(at) >= self.gap)
    }

    /// Offer a new item. It is returned when it may be sent now (the first one, or one after
    /// at least `gap`); otherwise it replaces any waiting item and `None` is returned.
    pub fn offer(&mut self, now: Instant, item: T) -> Option<T> {
        if self.due(now) {
            self.pending = None;
            self.sent_at = Some(now);
            Some(item)
        } else {
            self.pending = Some(item);
            None
        }
    }

    /// The waiting item, once its slot has come.
    pub fn poll(&mut self, now: Instant) -> Option<T> {
        if self.pending.is_some() && self.due(now) {
            self.sent_at = Some(now);
            self.pending.take()
        } else {
            None
        }
    }

    /// How long until the waiting item may go; `None` when nothing waits.
    #[must_use]
    pub fn wait(&self, now: Instant) -> Option<Duration> {
        self.pending.as_ref()?;
        let at = self.sent_at?;
        Some(self.gap.saturating_sub(now.saturating_duration_since(at)))
    }

    /// Send `item` at once, whatever the slot, and start a new slot. Used when leaving `Away`.
    pub fn force(&mut self, now: Instant, item: T) -> T {
        self.pending = None;
        self.sent_at = Some(now);
        item
    }

    /// The next item goes at once (after a connect). A waiting item stays.
    pub const fn reset(&mut self) {
        self.sent_at = None;
    }

    /// Forget the waiting item (the presence was cleared).
    pub fn drop_pending(&mut self) {
        self.pending = None;
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::RateLimiter;

    const GAP: Duration = Duration::from_secs(15);

    fn secs(base: Instant, n: u64) -> Instant {
        base + Duration::from_secs(n)
    }

    #[test]
    fn the_first_item_goes_at_once() {
        let t0 = Instant::now();
        let mut l = RateLimiter::new(GAP);
        assert_eq!(l.offer(t0, 1), Some(1));
    }

    #[test]
    fn a_second_item_waits_and_the_newest_wins() {
        let t0 = Instant::now();
        let mut l = RateLimiter::new(GAP);
        assert_eq!(l.offer(t0, 1), Some(1));
        assert_eq!(l.offer(secs(t0, 3), 2), None);
        assert_eq!(l.offer(secs(t0, 6), 3), None);
        assert_eq!(l.wait(secs(t0, 6)), Some(Duration::from_secs(9)));
        assert_eq!(l.poll(secs(t0, 14)), None);
        assert_eq!(l.poll(secs(t0, 15)), Some(3));
        assert_eq!(l.poll(secs(t0, 40)), None);
        assert_eq!(l.wait(secs(t0, 40)), None);
    }

    #[test]
    fn an_item_after_a_long_quiet_goes_at_once() {
        let t0 = Instant::now();
        let mut l = RateLimiter::new(GAP);
        assert_eq!(l.offer(t0, 1), Some(1));
        assert_eq!(l.offer(secs(t0, 600), 2), Some(2));
    }

    #[test]
    fn the_slot_restarts_when_a_waiting_item_is_released() {
        let t0 = Instant::now();
        let mut l = RateLimiter::new(GAP);
        assert_eq!(l.offer(t0, 1), Some(1));
        assert_eq!(l.offer(secs(t0, 5), 2), None);
        assert_eq!(l.poll(secs(t0, 15)), Some(2));
        // A seek right after goes into the next slot, not at once.
        assert_eq!(l.offer(secs(t0, 16), 3), None);
        assert_eq!(l.poll(secs(t0, 29)), None);
        assert_eq!(l.poll(secs(t0, 30)), Some(3));
    }

    #[test]
    fn force_sends_at_once_and_drops_the_waiting_item() {
        let t0 = Instant::now();
        let mut l = RateLimiter::new(GAP);
        assert_eq!(l.offer(t0, 1), Some(1));
        assert_eq!(l.offer(secs(t0, 2), 2), None);
        assert_eq!(l.force(secs(t0, 3), 9), 9);
        assert_eq!(l.poll(secs(t0, 60)), None);
        assert_eq!(l.offer(secs(t0, 10), 4), None);
    }

    #[test]
    fn reset_lets_the_next_item_go_at_once_and_keeps_a_waiting_one() {
        let t0 = Instant::now();
        let mut l = RateLimiter::new(GAP);
        assert_eq!(l.offer(t0, 1), Some(1));
        assert_eq!(l.offer(secs(t0, 2), 2), None);
        l.reset();
        assert_eq!(l.poll(secs(t0, 3)), Some(2));
        l.reset();
        assert_eq!(l.offer(secs(t0, 4), 5), Some(5));
    }

    #[test]
    fn drop_pending_forgets_the_waiting_item() {
        let t0 = Instant::now();
        let mut l = RateLimiter::new(GAP);
        assert_eq!(l.offer(t0, 1), Some(1));
        assert_eq!(l.offer(secs(t0, 2), 2), None);
        l.drop_pending();
        assert_eq!(l.poll(secs(t0, 60)), None);
    }
}
