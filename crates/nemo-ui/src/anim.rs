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
pub const ENTRANCE_MS: f32 = 240.0;
/// Delay between two staggered items, in ms.
pub const STAGGER_MS: f32 = 28.0;
/// Most items that stagger (later ones appear with the last).
pub const STAGGER_MAX: usize = 5;
/// Slide distance of an entrance, in px (`design.md` section 8: at most 16).
pub const SLIDE_PX: f32 = 0.0;
/// How much smaller an entrance starts (it grows from 98% to 100%).
pub const POP: f32 = 0.02;
/// Scale an opened cover grows from (the detail header, the full-window Now Playing): the
/// shared-element feel without a long flight across the window.
pub const GROW_FROM: f32 = 0.86;
/// Rise of the capsule when the first track starts, in px (`design.md` section 8: at most 16).
pub const CAPSULE_RISE: f32 = 16.0;
/// Length of the cover cross-fade, in ms.
pub const COVER_FADE_MS: f32 = 400.0;
/// Length of the bloom of the window backdrop on a song change, in ms (owner override of the
/// 400 ms cap, song-colour-theme spec).
// 600 is exact in f32.
#[allow(clippy::cast_precision_loss)]
pub const BLOOM_MS: f32 = crate::theme::motion::BLOOM_MS as f32;
/// Scale the new colour fields bloom in from (they settle to 1.0).
pub const BLOOM_FROM: f32 = 1.04;

/// Scale of the new colour fields at bloom progress `p` (0 to 1): 1.04 settling to 1.0.
#[must_use]
pub fn bloom_scale(p: f32) -> f32 {
    BLOOM_FROM + (1.0 - BLOOM_FROM) * p.clamp(0.0, 1.0)
}
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
    bloom_ms: f32,
    line_ms: f32,
    list_ms: f32,
    /// Play button shape: 0 circle (paused), 1 rounded square (playing).
    pub play: Spring,
    /// Right pane open fraction.
    pub pane: Spring,
    /// Capsule presence.
    pub capsule: Spring,
    /// Sign-in sheet presence.
    pub sheet: Spring,
    /// Now Playing full screen / full window transition progress.
    pub now_playing: Spring,
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
            bloom_ms: BLOOM_MS,
            line_ms: LINE_MS,
            list_ms: ENTRANCE_MS,
            play: unit(EXPRESSIVE_PLAY),
            pane: unit(SPATIAL_DEFAULT),
            capsule: unit(SPATIAL_DEFAULT),
            sheet: unit(SPATIAL_DEFAULT),
            now_playing: unit(SPATIAL_SLOW),
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
        self.bloom_ms = if self.reduced { BLOOM_MS } else { 0.0 };
    }

    /// The live lyric line changed.
    pub fn restart_line(&mut self) {
        self.line_ms = if self.reduced { LINE_MS } else { 0.0 };
    }

    /// The sidebar list swapped to the other listening area: the new rows fade in over the pane
    /// (a fade through, the old rows are already gone). Instant with reduced motion.
    pub fn restart_list(&mut self) {
        self.list_ms = if self.reduced { ENTRANCE_MS } else { 0.0 };
    }

    /// Fade-in progress (0 to 1) of the sidebar list after a swap of the listening area.
    #[must_use]
    pub fn list(&self) -> f32 {
        emphasized_decelerate((self.list_ms / ENTRANCE_MS).clamp(0.0, 1.0))
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

    /// Bloom progress of the window backdrop: 0 shows the old colour fields, 1 the new ones.
    #[must_use]
    pub fn bloom(&self) -> f32 {
        emphasized_decelerate((self.bloom_ms / BLOOM_MS).clamp(0.0, 1.0))
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
        self.bloom_ms = (self.bloom_ms + dt).min(BLOOM_MS);
        self.line_ms = (self.line_ms + dt).min(LINE_MS);
        self.list_ms = (self.list_ms + dt).min(ENTRANCE_MS);
        for s in [
            &mut self.play,
            &mut self.pane,
            &mut self.capsule,
            &mut self.sheet,
            &mut self.now_playing,
        ] {
            s.step(dt / 1000.0);
        }
    }

    /// Jump every animation to its end (reduced motion, headless snapshots).
    pub fn finish(&mut self) {
        self.page_ms = ENTRANCE_MS * 4.0;
        self.cover_ms = COVER_FADE_MS;
        self.bloom_ms = BLOOM_MS;
        self.line_ms = LINE_MS;
        self.list_ms = ENTRANCE_MS;
        for s in [
            &mut self.play,
            &mut self.pane,
            &mut self.capsule,
            &mut self.sheet,
            &mut self.now_playing,
        ] {
            s.finish();
        }
    }

    /// `true` while anything moves; the app subscribes to frames only then.
    #[must_use]
    pub fn active(&self) -> bool {
        self.page_ms < ENTRANCE_MS + STAGGER_MS * 8.0 + 1.0
            || self.cover_ms < COVER_FADE_MS
            || self.bloom_ms < BLOOM_MS
            || self.line_ms < LINE_MS
            || self.list_ms < ENTRANCE_MS
            || [
                &self.play,
                &self.pane,
                &self.capsule,
                &self.sheet,
                &self.now_playing,
            ]
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
    fn the_sidebar_list_fades_in_after_a_swap_and_draws_nothing_when_it_is_done() {
        let mut a = Anim::new(false);
        assert!(
            (a.list() - 1.0).abs() < 1e-6 && !a.active(),
            "settled at start"
        );
        a.restart_list();
        assert!(a.active() && a.list() < 0.01);
        run(&mut a, 100.0);
        assert!(
            a.list() > 0.05 && a.list() < 1.0,
            "on its way: {}",
            a.list()
        );
        run(&mut a, 400.0);
        assert!((a.list() - 1.0).abs() < 1e-6 && !a.active());
    }

    #[test]
    fn the_sidebar_list_swap_is_instant_with_reduced_motion() {
        let mut a = Anim::new(true);
        a.restart_list();
        assert!((a.list() - 1.0).abs() < 1e-6 && !a.active());
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
    fn the_bloom_runs_600_ms_decelerating_and_the_fields_settle_from_1_04() {
        let mut a = Anim::new(false);
        assert!((a.bloom() - 1.0).abs() < 1e-6 && !a.active());
        a.restart_cover();
        assert!(a.active() && a.bloom() < 0.01);
        assert!((bloom_scale(a.bloom()) - 1.04).abs() < 1e-3);
        // Still running after the 400 ms accent cross-fade, done by 600 ms.
        run(&mut a, 416.0);
        assert!((a.cover() - 1.0).abs() < 1e-6 && a.bloom() < 0.999 && a.active());
        let early = {
            let mut b = Anim::new(false);
            b.restart_cover();
            run(&mut b, 112.0);
            b.bloom()
        };
        assert!(
            early > 0.3,
            "emphasized-decelerate starts fast, got {early}"
        );
        run(&mut a, 200.0);
        assert!((a.bloom() - 1.0).abs() < 1e-6 && !a.active());
        assert!((bloom_scale(1.0) - 1.0).abs() < 1e-6);
        assert!(bloom_scale(0.5) < bloom_scale(0.1));
    }

    #[test]
    fn reduced_motion_swaps_the_colours_at_once() {
        let mut a = Anim::new(true);
        a.restart_cover();
        assert!((a.bloom() - 1.0).abs() < 1e-6 && !a.active());
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

    #[test]
    fn the_now_playing_spring_does_not_overshoot() {
        let mut a = Anim::new(false);
        a.now_playing.target = 1.0;
        let mut max = 0.0_f32;
        for _ in 0..120 {
            a.step(16.0);
            max = max.max(a.now_playing.value);
        }
        assert!(max <= 1.001, "{max}");
    }
}
