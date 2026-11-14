//! Cross-fades of the neutral set and of the background field.

use crate::motion::emphasized_decelerate;
use crate::palette::mix_rgb;
use crate::theme::{Neutrals, Rgba, motion::ACCENT_CROSSFADE_MS};

/// A cross-fade between two neutral sets (the animated theme switch), as long as the accent
/// cross-fade so both finish together.
#[derive(Debug, Clone, PartialEq)]
pub struct SchemeFade {
    from: Neutrals,
    to: Neutrals,
    elapsed_ms: f32,
}

impl SchemeFade {
    /// Duration in ms.
    #[allow(clippy::cast_precision_loss)] // 400 is exact in f32
    const DURATION_MS: f32 = ACCENT_CROSSFADE_MS as f32;

    /// A settled fade that shows `n`.
    #[must_use]
    pub fn settled(n: Neutrals) -> Self {
        Self {
            from: n,
            to: n,
            elapsed_ms: Self::DURATION_MS,
        }
    }

    /// Fade to `target` from what shows now; instant with reduced motion.
    pub fn retarget(&mut self, target: Neutrals, reduced_motion: bool) {
        if target == self.to {
            return;
        }
        self.from = self.current();
        self.to = target;
        self.elapsed_ms = if reduced_motion {
            Self::DURATION_MS
        } else {
            0.0
        };
    }

    /// Advance by `dt_ms`.
    pub fn advance(&mut self, dt_ms: f32) {
        self.elapsed_ms = (self.elapsed_ms + dt_ms.max(0.0)).min(Self::DURATION_MS);
    }

    /// `true` when the target is reached.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.elapsed_ms >= Self::DURATION_MS
    }

    /// The neutrals to draw now.
    #[must_use]
    pub fn current(&self) -> Neutrals {
        if self.is_done() {
            return self.to;
        }
        self.from.mix(
            &self.to,
            emphasized_decelerate(self.elapsed_ms / Self::DURATION_MS),
        )
    }
}

/// The cross-fade of the background field: a critically damped spring of about 0.7 s (a snap
/// with reduced motion).
#[derive(Debug, Clone)]
pub struct BgFade {
    from: [Rgba; 3],
    to: [Rgba; 3],
    spring: crate::motion::Spring,
}

impl BgFade {
    /// A settled field showing `colors`.
    #[must_use]
    pub fn settled(colors: [Rgba; 3]) -> Self {
        Self {
            from: colors,
            to: colors,
            spring: crate::motion::Spring::tuned(1.0, 150.0, 1.0),
        }
    }

    /// Fade to `target` from what shows now.
    pub fn retarget(&mut self, target: [Rgba; 3], reduced_motion: bool) {
        if target == self.to {
            return;
        }
        self.from = self.current();
        self.to = target;
        self.spring.snap(0.0);
        self.spring.target = 1.0;
        if reduced_motion {
            self.spring.snap(1.0);
        }
    }

    /// Advance by `dt_ms`.
    pub fn advance(&mut self, dt_ms: f32) {
        self.spring.step(dt_ms.clamp(0.0, 100.0) / 1000.0);
    }

    /// `true` when the target is reached.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.spring.is_settled() && (self.spring.value - 1.0).abs() < 0.004
    }

    /// Jump to the end.
    pub fn finish(&mut self) {
        self.spring.snap(1.0);
    }

    /// The colours to draw now.
    #[must_use]
    pub fn current(&self) -> [Rgba; 3] {
        let t = f64::from(self.spring.value.clamp(0.0, 1.0));
        if t >= 0.999 {
            return self.to;
        }
        [0, 1, 2].map(|i| mix_rgb(self.from[i], self.to[i], t))
    }
}
