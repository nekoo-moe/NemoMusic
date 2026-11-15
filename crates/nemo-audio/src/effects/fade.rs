//! Linear gain ramps for the effects chain: turning a stage on or off, and crossfading from old
//! to new equalizer coefficients.
//!
//! A ramp counts whole frames, so it ends exactly at 0 or 1 with no rounding drift.

/// How long a stage takes to turn on or off, and a coefficient change to blend in: 10 ms.
pub const RAMP_MS: u32 = 10;

/// A linear ramp between 0 and 1 over a fixed number of frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fade {
    /// Frames from 0 towards `len`; the gain is `pos / len`.
    pos: u16,
    /// Frames of a full ramp, at least 1.
    len: u16,
    /// The direction: towards 1 when `true`, towards 0 when `false`.
    on: bool,
}

impl Fade {
    /// A ramp of [`RAMP_MS`] at `rate` Hz, resting at 0.
    #[must_use]
    pub fn new(rate: u32) -> Self {
        let frames = u64::from(rate) * u64::from(RAMP_MS) / 1000;
        Self {
            pos: 0,
            len: u16::try_from(frames).unwrap_or(u16::MAX).max(1),
            on: false,
        }
    }

    /// Ramp towards 1 (`true`) or 0 (`false`) from where the gain is now.
    ///
    /// RT-SAFE: one store. No allocation, lock or logging.
    pub const fn set(&mut self, on: bool) {
        self.on = on;
    }

    /// Jump to 1 (`true`) or 0 (`false`) with no ramp.
    ///
    /// RT-SAFE: two stores. No allocation, lock or logging.
    pub const fn jump(&mut self, on: bool) {
        self.on = on;
        self.pos = if on { self.len } else { 0 };
    }

    /// Step one frame towards the target and return the gain of that frame.
    ///
    /// RT-SAFE: an integer step and one division. No allocation, lock or logging.
    #[inline]
    pub fn next_gain(&mut self) -> f32 {
        if self.on && self.pos < self.len {
            self.pos += 1;
        } else if !self.on && self.pos > 0 {
            self.pos -= 1;
        }
        self.gain()
    }

    /// The gain now, from 0 to 1.
    ///
    /// RT-SAFE: reads only. No allocation, lock or logging.
    #[must_use]
    pub fn gain(self) -> f32 {
        f32::from(self.pos) / f32::from(self.len)
    }

    /// The ramp rests at 1.
    ///
    /// RT-SAFE: reads only. No allocation, lock or logging.
    #[must_use]
    pub const fn is_full(self) -> bool {
        self.on && self.pos == self.len
    }

    /// The ramp rests at 0.
    ///
    /// RT-SAFE: reads only. No allocation, lock or logging.
    #[must_use]
    pub const fn is_off(self) -> bool {
        !self.on && self.pos == 0
    }

    /// The ramp rests at its target.
    ///
    /// RT-SAFE: reads only. No allocation, lock or logging.
    #[must_use]
    pub const fn is_settled(self) -> bool {
        self.is_full() || self.is_off()
    }
}

#[cfg(test)]
mod tests {
    use super::Fade;

    #[test]
    fn a_ramp_takes_ten_ms_and_ends_exactly_at_its_target() {
        let mut fade = Fade::new(48_000);
        assert_eq!(fade.len, 480);
        assert!(fade.is_off());
        fade.set(true);
        let gains: Vec<f32> = (0..480).map(|_| fade.next_gain()).collect();
        assert!(fade.is_full());
        assert_eq!(gains.last().copied(), Some(1.0));
        assert!(gains.windows(2).all(|w| w[1] > w[0]), "rises every frame");
        assert_eq!(fade.next_gain(), 1.0, "stays at 1");
        fade.set(false);
        for _ in 0..480 {
            fade.next_gain();
        }
        assert!(fade.is_off());
        assert_eq!(fade.gain(), 0.0);
    }

    #[test]
    fn a_reversed_ramp_turns_back_from_where_it_is() {
        let mut fade = Fade::new(44_100);
        fade.set(true);
        for _ in 0..100 {
            fade.next_gain();
        }
        fade.set(false);
        let first = fade.next_gain();
        assert!(first < 100.0 / 441.0 && first > 0.0);
        assert!(!fade.is_settled());
        fade.jump(true);
        assert!(fade.is_full());
        // A rate too low for one frame still ramps over one frame.
        assert_eq!(Fade::new(50).len, 1);
    }
}
