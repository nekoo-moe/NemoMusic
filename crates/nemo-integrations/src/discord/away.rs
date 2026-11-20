//! The sleeping rule (criterion 6): when the app shows `Away`.

use std::time::{Duration, Instant};

/// How long the window stays out of focus with nothing playing before the app shows `Away`.
pub const AWAY_AFTER: Duration = Duration::from_mins(10);

/// Decides when the app is asleep.
///
/// Quiet starts when the window is not focused (or is minimized) AND no song plays; a paused or
/// stopped player counts as nothing playing. A focus or a song that plays ends it at once. The
/// app is asleep once quiet has lasted `after` without a break. The OS idle or lock state is not
/// read. The clock is a parameter.
#[derive(Debug, Clone)]
pub struct AwayRule {
    after: Duration,
    active: bool,
    playing: bool,
    quiet_since: Option<Instant>,
}

impl AwayRule {
    /// A rule for a window that has the focus and a player that does not play.
    #[must_use]
    pub const fn new(after: Duration) -> Self {
        Self {
            after,
            active: true,
            playing: false,
            quiet_since: None,
        }
    }

    /// The window got or lost the focus (a minimized window counts as not focused).
    pub fn set_window_active(&mut self, now: Instant, active: bool) {
        self.active = active;
        self.update(now);
    }

    /// A song plays (`true`) or not (stopped, paused, empty).
    pub fn set_playing(&mut self, now: Instant, playing: bool) {
        self.playing = playing;
        self.update(now);
    }

    fn update(&mut self, now: Instant) {
        if self.active || self.playing {
            self.quiet_since = None;
        } else if self.quiet_since.is_none() {
            self.quiet_since = Some(now);
        }
    }

    /// Whether the app is asleep at `now`.
    #[must_use]
    pub fn asleep(&self, now: Instant) -> bool {
        self.quiet_since
            .is_some_and(|since| now.saturating_duration_since(since) >= self.after)
    }

    /// How long until the app falls asleep, if it is counting down.
    #[must_use]
    pub fn wake_in(&self, now: Instant) -> Option<Duration> {
        let since = self.quiet_since?;
        let left = self
            .after
            .saturating_sub(now.saturating_duration_since(since));
        (!left.is_zero()).then_some(left)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{AWAY_AFTER, AwayRule};

    fn min(base: Instant, n: u64) -> Instant {
        base + Duration::from_mins(n)
    }

    #[test]
    fn a_focused_window_is_never_asleep() {
        let t0 = Instant::now();
        let rule = AwayRule::new(AWAY_AFTER);
        assert!(!rule.asleep(min(t0, 600)));
        assert_eq!(rule.wake_in(t0), None);
    }

    #[test]
    fn asleep_after_ten_minutes_unfocused_with_nothing_playing() {
        let t0 = Instant::now();
        let mut rule = AwayRule::new(AWAY_AFTER);
        rule.set_window_active(t0, false);
        assert!(!rule.asleep(min(t0, 9)));
        assert_eq!(rule.wake_in(min(t0, 9)), Some(Duration::from_mins(1)));
        assert!(rule.asleep(min(t0, 10)));
        assert_eq!(rule.wake_in(min(t0, 10)), None);
    }

    #[test]
    fn a_song_that_plays_keeps_the_app_awake_and_restarts_the_count_when_it_stops() {
        let t0 = Instant::now();
        let mut rule = AwayRule::new(AWAY_AFTER);
        rule.set_window_active(t0, false);
        rule.set_playing(min(t0, 5), true);
        assert!(!rule.asleep(min(t0, 60)));
        rule.set_playing(min(t0, 60), false);
        assert!(!rule.asleep(min(t0, 69)));
        assert!(rule.asleep(min(t0, 70)));
    }

    #[test]
    fn a_paused_song_counts_as_nothing_playing() {
        let t0 = Instant::now();
        let mut rule = AwayRule::new(AWAY_AFTER);
        rule.set_playing(t0, true);
        rule.set_window_active(t0, false);
        rule.set_playing(min(t0, 1), false);
        assert!(rule.asleep(min(t0, 11)));
    }

    #[test]
    fn the_focus_wakes_it_at_once() {
        let t0 = Instant::now();
        let mut rule = AwayRule::new(AWAY_AFTER);
        rule.set_window_active(t0, false);
        assert!(rule.asleep(min(t0, 30)));
        rule.set_window_active(min(t0, 30), true);
        assert!(!rule.asleep(min(t0, 30)));
        // Losing the focus again starts a new count.
        rule.set_window_active(min(t0, 31), false);
        assert!(!rule.asleep(min(t0, 40)));
        assert!(rule.asleep(min(t0, 41)));
    }

    #[test]
    fn repeating_the_same_fact_does_not_restart_the_count() {
        let t0 = Instant::now();
        let mut rule = AwayRule::new(AWAY_AFTER);
        rule.set_window_active(t0, false);
        rule.set_window_active(min(t0, 6), false);
        rule.set_playing(min(t0, 7), false);
        assert!(rule.asleep(min(t0, 10)));
    }
}
