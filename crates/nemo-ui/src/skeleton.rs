//! Skeleton loading (skeleton-loading spec): the timing of the grey blocks a page shows while
//! its first data is on the way. Pure: time is passed in, never read.
//!
//! A [`Gate`] follows one region. The page says whether a skeleton is wanted (loading, nothing
//! cached); the gate waits [`DELAY_MS`] before showing it, keeps it at least [`HOLD_MS`], then
//! fades it out over [`FADE_MS`]. [`Skeletons`] holds one gate per [`Region`] and the one
//! shimmer clock.

use crate::motion::{cubic_bezier, emphasized_decelerate};
use crate::theme::motion::BASE_MS;

/// How long a skeleton is wanted before it is drawn, so a fast answer never flashes it.
pub const DELAY_MS: f32 = 150.0;
/// Least time a skeleton stays once drawn.
pub const HOLD_MS: f32 = 300.0;
/// One sweep of the shimmer band (`design.md` section 17).
pub const SHIMMER_MS: f32 = 1400.0;
/// Cross-fade from the skeleton to the content, `motion.base`.
// 200 is exact in f32.
#[allow(clippy::cast_precision_loss)]
pub const FADE_MS: f32 = BASE_MS as f32;

/// A part of the app that can show a skeleton. A region is wanted only while it is on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    /// The answer of the Search page (All or the picked chip).
    Search,
    /// The Home page.
    Home,
    /// The Explore page.
    Explore,
    /// The Library page.
    Library,
    /// The `Your library` list of the sidebar.
    Sidebar,
    /// An album, playlist or artist page.
    Detail,
}

impl Region {
    /// Every region, in the order of [`Skeletons`].
    pub const ALL: [Self; 6] = [
        Self::Search,
        Self::Home,
        Self::Explore,
        Self::Library,
        Self::Sidebar,
        Self::Detail,
    ];

    fn index(self) -> usize {
        Self::ALL.iter().position(|r| *r == self).unwrap_or(0)
    }
}

/// What a region draws now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Show {
    /// The real content (or the empty, error and refresh states it already has).
    Content,
    /// Only the skeleton.
    Skeleton,
    /// The content fading in under the skeleton, which is at this opacity (1 down to 0).
    Fade(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum Stage {
    #[default]
    Off,
    /// Wanted, not drawn yet.
    Waiting,
    /// Drawn; `wanted` says whether the page still asks for it.
    Shown { wanted: bool },
    /// The page has its data: the skeleton fades out.
    Leaving,
}

/// The timing of one skeleton.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Gate {
    stage: Stage,
    ms: f32,
}

impl Gate {
    /// The page now asks for a skeleton (`wanted`) or does not. Call after every update.
    pub fn set(&mut self, wanted: bool, reduced: bool) {
        match (self.stage, wanted) {
            (Stage::Off, true) => {
                self.stage = Stage::Waiting;
                self.ms = 0.0;
            }
            // Gone before it showed: the fast answer never saw it.
            (Stage::Waiting, false) => self.stage = Stage::Off,
            (Stage::Shown { .. }, _) => {
                self.stage = Stage::Shown { wanted };
                self.settle(reduced);
            }
            // Wanted again while it fades (another load began): it is back at full strength.
            (Stage::Leaving, true) => {
                self.stage = Stage::Shown { wanted: true };
                self.ms = HOLD_MS;
            }
            _ => {}
        }
    }

    /// Move to `Leaving` (or `Off` with reduced motion) once the page has its data and the hold
    /// is over.
    fn settle(&mut self, reduced: bool) {
        if self.stage == (Stage::Shown { wanted: false }) && self.ms >= HOLD_MS {
            self.ms = 0.0;
            self.stage = if reduced { Stage::Off } else { Stage::Leaving };
        }
    }

    /// Advance by `dt_ms` milliseconds.
    pub fn advance(&mut self, dt_ms: f32, reduced: bool) {
        self.ms += dt_ms.max(0.0);
        match self.stage {
            Stage::Waiting if self.ms >= DELAY_MS => {
                self.stage = Stage::Shown { wanted: true };
                self.ms = 0.0;
            }
            Stage::Shown { .. } => self.settle(reduced),
            Stage::Leaving if self.ms >= FADE_MS => {
                self.stage = Stage::Off;
                self.ms = 0.0;
            }
            _ => {}
        }
    }

    /// What to draw now.
    #[must_use]
    pub fn show(&self) -> Show {
        match self.stage {
            Stage::Off | Stage::Waiting => Show::Content,
            Stage::Shown { .. } => Show::Skeleton,
            Stage::Leaving => Show::Fade(1.0 - emphasized_decelerate(self.ms / FADE_MS)),
        }
    }

    /// `true` while a skeleton is drawn (also while it fades).
    #[must_use]
    pub fn visible(&self) -> bool {
        matches!(self.stage, Stage::Shown { .. } | Stage::Leaving)
    }

    /// `true` while the gate needs frames: the wait, the hold, the fade, and the shimmer when
    /// motion is on. A shown skeleton with reduced motion is still and needs none.
    #[must_use]
    pub fn needs_frames(&self, reduced: bool) -> bool {
        match self.stage {
            Stage::Off => false,
            Stage::Waiting | Stage::Leaving | Stage::Shown { wanted: false } => true,
            Stage::Shown { wanted: true } => !reduced,
        }
    }
}

/// Position of the shimmer band across the page for a loop position `t` (0 to 1): 0 is the band
/// before the left edge, 1 past the right edge. Standard easing (`cubic-bezier(0.2, 0, 0, 1)`).
#[must_use]
pub fn band_position(t: f32) -> f32 {
    cubic_bezier(0.2, 0.0, 0.0, 1.0, t.rem_euclid(1.0))
}

/// One gate per region and the one shimmer clock.
#[derive(Debug, Clone, Default)]
pub struct Skeletons {
    gates: [Gate; 6],
    ms: f32,
}

impl Skeletons {
    /// The page asks for a skeleton in `region` or not.
    pub fn set(&mut self, region: Region, wanted: bool, reduced: bool) {
        self.gates[region.index()].set(wanted, reduced);
    }

    /// Advance by `dt_ms`. The shimmer clock runs only while a skeleton is drawn and motion is on.
    pub fn advance(&mut self, dt_ms: f32, reduced: bool) {
        let was_visible = self.any_visible();
        for g in &mut self.gates {
            g.advance(dt_ms, reduced);
        }
        // The band starts at the left edge on the frame after a skeleton appears.
        if reduced || !was_visible || !self.any_visible() {
            self.ms = 0.0;
        } else {
            self.ms = (self.ms + dt_ms.max(0.0)) % SHIMMER_MS;
        }
    }

    /// What `region` draws now.
    #[must_use]
    pub fn show(&self, region: Region) -> Show {
        self.gates[region.index()].show()
    }

    /// Loop position of the shimmer, 0 (just started) to under 1.
    #[must_use]
    pub fn phase(&self) -> f32 {
        self.ms / SHIMMER_MS
    }

    /// `true` when any gate needs frames.
    #[must_use]
    pub fn needs_frames(&self, reduced: bool) -> bool {
        self.gates.iter().any(|g| g.needs_frames(reduced))
    }

    /// `true` while any skeleton is drawn.
    #[must_use]
    pub fn any_visible(&self) -> bool {
        self.gates.iter().any(Gate::visible)
    }

    /// The route changed: a page skeleton that waited or showed for the page before is dropped
    /// at once, never held for the next page. The sidebar list does not follow the route.
    pub fn leave_page(&mut self) {
        for r in Region::ALL {
            if r != Region::Sidebar {
                self.gates[r.index()] = Gate::default();
            }
        }
    }

    /// Draw every region that waits as a skeleton now, without the delay (headless snapshots).
    pub fn skip_waits(&mut self) {
        for g in &mut self.gates {
            if g.stage == Stage::Waiting {
                g.stage = Stage::Shown { wanted: true };
                g.ms = 0.0;
            }
        }
    }

    /// Jump to the loop position `t` (headless snapshots).
    pub fn seek(&mut self, t: f32) {
        self.ms = t.rem_euclid(1.0) * SHIMMER_MS;
    }
}

#[cfg(test)]
mod tests;
