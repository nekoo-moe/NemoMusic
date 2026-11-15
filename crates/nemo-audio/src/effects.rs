//! Effects the user turns on: the equalizer with its presets and `AutoEq` profile files, the
//! delay and the reverb.
//!
//! Every effect is off by default, and an effect that is off is bypassed with no arithmetic
//! (`code.md`, Bit-perfect). Code here runs off the real-time path unless its doc says
//! `RT-SAFE`.

mod biquad;
mod delay;
mod presets;
mod profile;
mod reverb;

pub use biquad::{Biquad, Coefficients, TEN_BAND_HZ, TEN_BAND_Q_HUNDREDTHS, auto_preamp, ten_band};
pub use delay::{DELAY_MAX_CHANNELS, Delay, DelayParams};
pub use presets::{
    BUILTIN_PRESETS, BuiltinPreset, PRESET_FILE_MAX_BYTES, PresetError, PresetStore,
};
pub use profile::{EqProfile, ProfileError, parse_autoeq, write_autoeq};
pub use reverb::{REVERB_MAX_CHANNELS, Reverb, ReverbParams};
