//! Effects the user turns on: the equalizer and its `AutoEq` profile files, and the delay.
//!
//! Every effect is off by default, and an effect that is off is bypassed with no arithmetic
//! (`code.md`, Bit-perfect). Code here runs off the real-time path unless its doc says
//! `RT-SAFE`.

mod biquad;
mod delay;
mod profile;

pub use biquad::{Biquad, Coefficients, TEN_BAND_HZ, TEN_BAND_Q_HUNDREDTHS, auto_preamp, ten_band};
pub use delay::{DELAY_MAX_CHANNELS, Delay, DelayParams};
pub use profile::{EqProfile, ProfileError, parse_autoeq, write_autoeq};
