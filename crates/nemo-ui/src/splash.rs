//! The start-up splash (startup-flow spec, task 1): which real steps the app has finished, how
//! long the splash stays, and how it fades out.
//!
//! Pure logic. The caller passes the time in milliseconds since the first frame, so the tests
//! drive it with a fake clock. No step is guessed: a step counts only after the app reported it.

use crate::motion::emphasized_accelerate;
use crate::theme::motion::BASE_MS;

/// The splash stays at least this long, so it never flashes (requirements, criterion 3).
pub const MIN_SHOW_MS: u64 = 500;
/// After this long without the app being ready the splash says so (requirements, criterion 4).
pub const MAX_SHOW_MS: u64 = 6_000;

/// One real step of starting the app, in the order they run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartStep {
    /// The settings file was read (before the window opens).
    Settings,
    /// The local library and the offline cache were opened.
    Library,
    /// The operating system's credential store was opened.
    CredentialStore,
    /// The player was started.
    Player,
    /// The first screen was built and its first requests were started.
    FirstScreen,
}

impl StartStep {
    /// Every step, in the order they run.
    pub const ALL: [Self; 5] = [
        Self::Settings,
        Self::Library,
        Self::CredentialStore,
        Self::Player,
        Self::FirstScreen,
    ];

    /// What the app is doing while this step runs (`label-sm` under the progress line).
    #[must_use]
    pub const fn doing(self) -> &'static str {
        match self {
            Self::Settings => "Loading settings",
            Self::Library => "Opening your library",
            Self::CredentialStore => "Opening the credential store",
            Self::Player => "Starting the player",
            Self::FirstScreen => "Opening the first screen",
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Settings => 0,
            Self::Library => 1,
            Self::CredentialStore => 2,
            Self::Player => 3,
            Self::FirstScreen => 4,
        }
    }
}

/// What the splash shows now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplashPhase {
    /// Drawn in full.
    Showing,
    /// Fading into the app.
    Fading,
    /// Gone; the app is on screen.
    Gone,
}

/// The splash: finished steps, a possible failure, and the fade.
#[derive(Debug, Clone)]
pub struct Splash {
    done: [bool; StartStep::ALL.len()],
    failure: Option<String>,
    /// When the last step finished, in ms since the first frame.
    ready_at: Option<u64>,
    /// When the fade began.
    fade_from: Option<u64>,
    gone: bool,
    reduced_motion: bool,
}

impl Splash {
    /// A splash at the first frame. The settings are already read by then, so that step counts.
    #[must_use]
    pub fn new(reduced_motion: bool) -> Self {
        let mut s = Self {
            done: [false; StartStep::ALL.len()],
            failure: None,
            ready_at: None,
            fade_from: None,
            gone: false,
            reduced_motion,
        };
        s.done[StartStep::Settings.index()] = true;
        s
    }

    /// Record a finished step; `now_ms` is the time since the first frame.
    pub fn finish(&mut self, step: StartStep, now_ms: u64) {
        self.done[step.index()] = true;
        if self.ready_at.is_none() && self.done.iter().all(|d| *d) {
            self.ready_at = Some(now_ms);
        }
    }

    /// The app could not start. The splash stays and shows `message` (a plain sentence).
    pub fn fail(&mut self, message: String) {
        self.failure = Some(message);
    }

    /// The plain sentence of a failed start, if any.
    #[must_use]
    pub fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    /// Advance the clock to `now_ms`. Begins the fade once the app is ready and the splash has
    /// been up for [`MIN_SHOW_MS`]; with reduced motion it ends at once.
    pub fn tick(&mut self, now_ms: u64) {
        if self.gone || self.failure.is_some() {
            return;
        }
        let Some(ready) = self.ready_at else { return };
        match self.fade_from {
            None if now_ms >= MIN_SHOW_MS => {
                self.fade_from = Some(now_ms.max(ready));
                self.gone = self.reduced_motion;
            }
            Some(from) if now_ms >= from + u64::from(BASE_MS) => self.gone = true,
            _ => {}
        }
    }

    /// Where the splash is in its life.
    #[must_use]
    pub fn phase(&self) -> SplashPhase {
        if self.gone {
            SplashPhase::Gone
        } else if self.fade_from.is_some() {
            SplashPhase::Fading
        } else {
            SplashPhase::Showing
        }
    }

    /// `true` while the splash covers any part of the window.
    #[must_use]
    pub fn visible(&self) -> bool {
        self.phase() != SplashPhase::Gone
    }

    /// Opacity of the splash at `now_ms`: 1 while shown, then 1 to 0 over `motion.base` on the
    /// emphasized-accelerate curve.
    #[must_use]
    pub fn opacity(&self, now_ms: u64) -> f32 {
        match (self.phase(), self.fade_from) {
            (SplashPhase::Gone, _) => 0.0,
            (SplashPhase::Fading, Some(from)) => {
                let spent = now_ms.saturating_sub(from).min(u64::from(BASE_MS));
                let spent = u16::try_from(spent).unwrap_or(u16::MAX);
                1.0 - emphasized_accelerate(f32::from(spent) / f32::from(BASE_MS_U16))
            }
            _ => 1.0,
        }
    }

    /// Share of the steps that are finished, 0 to 1. It counts steps, not time.
    #[must_use]
    pub fn fraction(&self) -> f32 {
        let done = self.done.iter().filter(|d| **d).count();
        // At most 5 steps: both numbers are exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let fraction = done as f32 / self.done.len() as f32;
        fraction
    }

    /// The step running now: the first one not finished.
    #[must_use]
    pub fn current(&self) -> Option<StartStep> {
        StartStep::ALL.into_iter().find(|s| !self.done[s.index()])
    }

    /// `true` when the app is still not ready [`MAX_SHOW_MS`] after the first frame. The app
    /// has nothing to show without its services, so the splash stays and says so.
    #[must_use]
    pub fn is_slow(&self, now_ms: u64) -> bool {
        self.ready_at.is_none() && self.failure.is_none() && now_ms >= MAX_SHOW_MS
    }

    /// The line under the progress bar: the running step, a note when the start is slow, or the
    /// failure.
    #[must_use]
    pub fn status(&self, now_ms: u64) -> String {
        if let Some(f) = &self.failure {
            return f.clone();
        }
        let doing = self.current().map_or("Ready", StartStep::doing);
        if self.is_slow(now_ms) {
            format!("{doing}. This is taking longer than usual.")
        } else {
            doing.to_owned()
        }
    }
}

/// `motion.base` as a `u16`, for the fade maths.
const BASE_MS_U16: u16 = 200;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fade_constant_is_motion_base() {
        assert_eq!(u32::from(BASE_MS_U16), BASE_MS);
    }

    fn run_all(s: &mut Splash, at: u64) {
        for step in StartStep::ALL {
            s.finish(step, at);
        }
    }

    #[test]
    fn the_settings_step_is_done_at_the_first_frame_and_the_rest_are_not_guessed() {
        let s = Splash::new(false);
        assert_eq!(s.current(), Some(StartStep::Library));
        assert!((s.fraction() - 0.2).abs() < f32::EPSILON);
        assert_eq!(s.status(0), "Opening your library");
    }

    #[test]
    fn the_progress_line_counts_reported_steps_only() {
        let mut s = Splash::new(false);
        s.finish(StartStep::Library, 10);
        s.finish(StartStep::CredentialStore, 20);
        assert!((s.fraction() - 0.6).abs() < f32::EPSILON);
        assert_eq!(s.current(), Some(StartStep::Player));
        run_all(&mut s, 30);
        assert!((s.fraction() - 1.0).abs() < f32::EPSILON);
        assert_eq!(s.current(), None);
        assert_eq!(s.status(30), "Ready");
    }

    #[test]
    fn a_fast_start_still_shows_the_splash_for_half_a_second() {
        let mut s = Splash::new(false);
        run_all(&mut s, 80);
        s.tick(100);
        s.tick(499);
        assert_eq!(s.phase(), SplashPhase::Showing);
        assert!((s.opacity(499) - 1.0).abs() < f32::EPSILON);
        s.tick(500);
        assert_eq!(s.phase(), SplashPhase::Fading);
    }

    #[test]
    fn the_fade_takes_motion_base_and_ends_gone() {
        let mut s = Splash::new(false);
        run_all(&mut s, 80);
        s.tick(500);
        assert!((s.opacity(500) - 1.0).abs() < f32::EPSILON);
        let mid = s.opacity(600);
        assert!(mid > 0.0 && mid < 1.0, "mid-fade opacity {mid}");
        s.tick(699);
        assert!(s.visible());
        s.tick(700);
        assert_eq!(s.phase(), SplashPhase::Gone);
        assert!(s.opacity(700).abs() < f32::EPSILON);
    }

    #[test]
    fn a_slow_start_fades_as_soon_as_the_app_is_ready() {
        let mut s = Splash::new(false);
        s.tick(2_000);
        assert_eq!(s.phase(), SplashPhase::Showing);
        run_all(&mut s, 2_100);
        s.tick(2_100);
        assert_eq!(s.phase(), SplashPhase::Fading);
        s.tick(2_300);
        assert_eq!(s.phase(), SplashPhase::Gone);
    }

    #[test]
    fn reduced_motion_ends_the_splash_without_a_fade() {
        let mut s = Splash::new(true);
        run_all(&mut s, 80);
        s.tick(499);
        assert!(s.visible());
        s.tick(500);
        assert_eq!(s.phase(), SplashPhase::Gone);
        assert!(s.opacity(500).abs() < f32::EPSILON);
    }

    #[test]
    fn after_six_seconds_without_a_ready_app_the_splash_says_it_is_slow() {
        let mut s = Splash::new(false);
        s.finish(StartStep::Library, 100);
        assert!(!s.is_slow(5_999));
        assert!(s.is_slow(6_000));
        s.tick(6_000);
        assert!(s.visible(), "nothing to show without the services");
        assert_eq!(
            s.status(6_000),
            "Opening the credential store. This is taking longer than usual."
        );
        run_all(&mut s, 6_500);
        assert!(!s.is_slow(6_500));
        s.tick(6_500);
        assert_eq!(s.phase(), SplashPhase::Fading);
    }

    #[test]
    fn a_failed_start_stays_on_screen_with_its_sentence() {
        let mut s = Splash::new(false);
        s.fail("NemoMusic could not start: the data folder is read-only.".to_owned());
        run_all(&mut s, 10);
        s.tick(10_000);
        assert!(s.visible());
        assert!(s.failure().is_some_and(|f| f.contains("read-only")));
        assert!(!s.is_slow(10_000));
        assert_eq!(
            s.status(10_000),
            "NemoMusic could not start: the data folder is read-only."
        );
    }
}
