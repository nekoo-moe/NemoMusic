//! UI animation state outside the reducer (`docs/ui-redesign.md` section 3): the page entrance,
//! the play button morph, the right pane, the capsule, the sign-in sheet and the cover and lyric
//! cross-fades. Pure and deterministic: time is passed in. With reduced motion every value jumps
//! to its target, and an idle `Anim` reports `active() == false`, so nothing redraws.

use crate::motion::{Spring, emphasized_decelerate};

/// `spatial.fast`: stiffness and damping ratio.
pub const SPATIAL_FAST: (f32, f32) = (1400.0, 1.0);
/// `spatial.default`.
pub const SPATIAL_DEFAULT: (f32, f32) = (700.0, 1.0);
/// `spatial.slow`.
pub const SPATIAL_SLOW: (f32, f32) = (300.0, 1.0);
/// `effects.fast`: colour and opacity (state layers).
pub const EFFECTS_FAST: (f32, f32) = (3800.0, 1.0);
/// The one expressive overshoot: the play button morph.
pub const EXPRESSIVE_PLAY: (f32, f32) = (1400.0, 0.8);
/// Length of the page entrance and of each staggered item, in ms.
pub const ENTRANCE_MS: f32 = 320.0;
/// Delay between two staggered items, in ms.
pub const STAGGER_MS: f32 = 45.0;
/// Most items that stagger (later ones appear with the last).
pub const STAGGER_MAX: usize = 8;
/// Slide distance of an entrance, in px (`design.md` section 8: at most 16).
pub const SLIDE_PX: f32 = 16.0;
/// Length of the cover cross-fade, in ms.
pub const COVER_FADE_MS: f32 = 400.0;
/// Length of the live lyric line swap, in ms.
pub const LINE_MS: f32 = 240.0;

/// Eased 0 to 1 progress of an entrance that started `elapsed_ms` ago after `delay_ms`.
#[must_use]
pub fn entrance(elapsed_ms: f32, delay_ms: f32) -> f32 {
    emphasized_decelerate(((elapsed_ms - delay_ms) / ENTRANCE_MS).clamp(0.0, 1.0))
}

/// Corner radius of the play button for a morph value: a circle at 0, a rounded square at 1.
/// The expressive spring overshoots, so the value is clamped at 0.
#[must_use]
pub fn play_radius(size: f32, morph: f32) -> f32 {
    let circle = size / 2.0;
    let square = size * 0.3;
    (circle + (square - circle) * morph).max(0.0)
}

/// Every running animation of the window.
#[derive(Debug, Clone, PartialEq)]
pub struct Anim {
    page_ms: f32,
    cover_ms: f32,
    line_ms: f32,
    /// Play button shape: 0 circle (paused), 1 rounded square (playing).
    pub play: Spring,
    /// Right pane open fraction.
    pub pane: Spring,
    /// Capsule presence.
    pub capsule: Spring,
    /// Sign-in sheet presence.
    pub sheet: Spring,
    reduced: bool,
}

impl Anim {
    /// Settled animations, everything closed and at rest; the app points the targets.
    #[must_use]
    pub fn new(reduced: bool) -> Self {
        let unit = |tokens: (f32, f32)| Spring::tuned(0.0, tokens.0, tokens.1);
        Self {
            page_ms: ENTRANCE_MS * 4.0,
            cover_ms: COVER_FADE_MS,
            line_ms: LINE_MS,
            play: unit(EXPRESSIVE_PLAY),
            pane: unit(SPATIAL_DEFAULT),
            capsule: unit(SPATIAL_DEFAULT),
            sheet: unit(SPATIAL_DEFAULT),
            reduced,
        }
    }

    /// Follow the reduce-motion setting.
    pub fn set_reduced(&mut self, reduced: bool) {
        self.reduced = reduced;
        if reduced {
            self.finish();
        }
    }

    /// A new page started: fade and slide it in.
    pub fn restart_page(&mut self) {
        self.page_ms = if self.reduced { ENTRANCE_MS * 4.0 } else { 0.0 };
    }

    /// A new cover arrived: cross-fade from the old one.
    pub fn restart_cover(&mut self) {
        self.cover_ms = if self.reduced { COVER_FADE_MS } else { 0.0 };
    }

    /// The live lyric line changed.
    pub fn restart_line(&mut self) {
        self.line_ms = if self.reduced { LINE_MS } else { 0.0 };
    }

    /// Entrance progress (0 to 1) of item `index` of a staggered list on this page.
    #[must_use]
    pub fn reveal(&self, index: usize) -> f32 {
        // Index is at most STAGGER_MAX, exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let delay = index.min(STAGGER_MAX) as f32 * STAGGER_MS;
        entrance(self.page_ms, delay)
    }

    /// Entrance progress of the whole page.
    #[must_use]
    pub fn page(&self) -> f32 {
        self.reveal(0)
    }

    /// Cover cross-fade progress: 0 shows the old cover, 1 the new one.
    #[must_use]
    pub fn cover(&self) -> f32 {
        emphasized_decelerate((self.cover_ms / COVER_FADE_MS).clamp(0.0, 1.0))
    }

    /// Live lyric line swap progress.
    #[must_use]
    pub fn line(&self) -> f32 {
        emphasized_decelerate((self.line_ms / LINE_MS).clamp(0.0, 1.0))
    }

    /// Advance by `dt_ms` milliseconds.
    pub fn step(&mut self, dt_ms: f32) {
        if self.reduced {
            self.finish();
            return;
        }
        let dt = dt_ms.clamp(0.0, 100.0);
        self.page_ms += dt;
        self.cover_ms = (self.cover_ms + dt).min(COVER_FADE_MS);
        self.line_ms = (self.line_ms + dt).min(LINE_MS);
        for s in [
            &mut self.play,
            &mut self.pane,
            &mut self.capsule,
            &mut self.sheet,
        ] {
            s.step(dt / 1000.0);
        }
    }

    /// Jump every animation to its end (reduced motion, headless snapshots).
    pub fn finish(&mut self) {
        self.page_ms = ENTRANCE_MS * 4.0;
        self.cover_ms = COVER_FADE_MS;
        self.line_ms = LINE_MS;
        for s in [
            &mut self.play,
            &mut self.pane,
            &mut self.capsule,
            &mut self.sheet,
        ] {
            s.finish();
        }
    }

    /// `true` while anything moves; the app subscribes to frames only then.
    #[must_use]
    pub fn active(&self) -> bool {
        self.page_ms < ENTRANCE_MS + STAGGER_MS * 8.0 + 1.0
            || self.cover_ms < COVER_FADE_MS
            || self.line_ms < LINE_MS
            || [&self.play, &self.pane, &self.capsule, &self.sheet]
                .iter()
                .any(|s| !s.is_settled())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(a: &mut Anim, ms: f32) {
        let mut left = ms;
        while left > 0.0 {
            a.step(16.0);
            left -= 16.0;
        }
    }

    #[test]
    fn an_idle_anim_asks_for_no_frames() {
        let a = Anim::new(false);
        assert!(!a.active());
        assert!((a.page() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_page_entrance_runs_and_staggered_items_follow() {
        let mut a = Anim::new(false);
        a.restart_page();
        assert!(a.active() && a.page() < 0.01);
        run(&mut a, 100.0);
        assert!(a.reveal(0) > a.reveal(3), "later items start later");
        run(&mut a, 1000.0);
        assert!(!a.active());
        assert!((a.reveal(STAGGER_MAX + 5) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_play_button_overshoots_then_settles_on_a_square() {
        let mut a = Anim::new(false);
        a.play.target = 1.0;
        let mut max = 0.0_f32;
        for _ in 0..120 {
            a.step(16.0);
            max = max.max(a.play.value);
        }
        assert!(max > 1.0, "damping 0.8 overshoots a little, got {max}");
        assert!(max < 1.2);
        assert!((a.play.value - 1.0).abs() < 0.01 && !a.active());
        assert!((play_radius(56.0, 0.0) - 28.0).abs() < 1e-4);
        assert!((play_radius(56.0, 1.0) - 16.8).abs() < 1e-4);
        assert!(play_radius(56.0, 5.0) >= 0.0);
    }

    #[test]
    fn reduced_motion_jumps() {
        let mut a = Anim::new(true);
        a.pane.target = 1.0;
        a.restart_page();
        a.restart_cover();
        a.step(16.0);
        assert!((a.pane.value - 1.0).abs() < 1e-6 && !a.active());
        assert!((a.cover() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_pane_spring_does_not_overshoot() {
        let mut a = Anim::new(false);
        a.pane.target = 1.0;
        let mut max = 0.0_f32;
        for _ in 0..80 {
            a.step(16.0);
            max = max.max(a.pane.value);
        }
        assert!(max <= 1.001, "{max}");
    }
}
