//! Effects the user turns on: the equalizer with its presets and `AutoEq` profile files, the
//! delay and the reverb, and the [`EffectsChain`] that runs them in the audio callback.
//!
//! Every effect is off by default, and an effect that is off is bypassed with no arithmetic
//! (`code.md`, Bit-perfect). Code here runs off the real-time path unless its doc says
//! `RT-SAFE`.

mod biquad;
mod chain;
mod delay;
mod equalizer;
mod fade;
mod presets;
mod profile;
mod reverb;
mod stage;

pub use biquad::{Biquad, Coefficients, TEN_BAND_HZ, TEN_BAND_Q_HUNDREDTHS, auto_preamp, ten_band};
pub use chain::{EFFECTS_RING_CAPACITY, EffectsChain, EffectsUpdate, effects_ring};
pub use delay::{DELAY_MAX_CHANNELS, Delay, DelayParams};
pub use fade::RAMP_MS;
pub use presets::{
    BUILTIN_PRESETS, BuiltinPreset, PRESET_FILE_MAX_BYTES, PresetError, PresetStore,
};
pub use profile::{EqProfile, ProfileError, parse_autoeq, write_autoeq};
pub use reverb::{REVERB_MAX_CHANNELS, Reverb, ReverbParams};
