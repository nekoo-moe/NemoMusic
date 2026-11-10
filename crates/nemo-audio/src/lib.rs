//! Audio engine: decode, ring buffer, device output and signal-path report.
//!
//! Real-time code lives only in this crate (see `code.md`, "Real-time audio thread").
//!
//! This slice plays a local FLAC or WAV file: [`decoder`] -> [`ring`] -> [`render`] -> [`sink`],
//! tied together by [`engine`]. No resampling, gain or dither exists in the path, so the
//! signal path it reports has an empty processing list.

pub mod decoder;
pub mod engine;
pub mod render;
pub mod ring;
pub mod sink;
