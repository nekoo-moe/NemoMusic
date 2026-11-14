//! Effects the user turns on: the equalizer and its `AutoEq` profile files.
//!
//! Every effect is off by default, and an effect that is off is bypassed with no arithmetic
//! (`code.md`, Bit-perfect). Code here runs off the real-time path unless its doc says
//! `RT-SAFE`.

mod profile;

pub use profile::{EqProfile, ProfileError, parse_autoeq, write_autoeq};
