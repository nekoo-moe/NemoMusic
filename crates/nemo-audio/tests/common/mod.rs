//! Synthetic fixtures and fakes written from code. Shared by the integration tests.
#![allow(dead_code)] // reason: each test binary uses a different part of this module

pub mod fakes;
pub mod fixtures;

use std::fs;
use std::path::Path;

/// Stereo test signal as integers: left is a full-range ramp, right is a full-scale sine with a
/// toggling low bit, so every bit position changes somewhere in the file.
pub fn test_signal(bits: u32, frames: usize) -> Vec<i32> {
    let max = (1_i64 << (bits - 1)) - 1;
    let min = -(1_i64 << (bits - 1));
    let span = max - min;
    let mut out = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let i64_i = i64::try_from(i).expect("invariant: small frame index");
        // Ramp: steps through the whole range with a prime stride so low bits vary too.
        let ramp = min + (i64_i * 7_919) % span;
        let phase = f64::from(u32::try_from(i).expect("invariant: small index"))
            * 2.0
            * std::f64::consts::PI
            * 997.0
            / 48_000.0;
        #[allow(clippy::cast_possible_truncation)] // sine is within the integer range
        let sine = (phase.sin() * f64::from(i32::try_from(max).expect("invariant: 24-bit max")))
            .round() as i64;
        let sine = ((sine & !1) | (i64_i & 1)).clamp(min, max);
        out.push(i32::try_from(ramp).expect("invariant: ramp fits i32"));
        out.push(i32::try_from(sine).expect("invariant: sine fits i32"));
    }
    out
}

/// Write interleaved integers as a little-endian PCM WAV with `bits` bits (16 or 24).
pub fn write_wav(path: &Path, samples: &[i32], bits: u32, rate: u32, channels: u16) {
    fs::write(path, fixtures::wav_bytes(samples, bits, rate, channels))
        .expect("test fixture write");
}

/// Convert decoded `f32` back to integers the way a lossless path would: scale by `2^(bits-1)`.
pub fn to_ints(samples: &[f32], bits: u32) -> Vec<i32> {
    let scale = f64::from(1_u32 << (bits - 1));
    #[allow(clippy::cast_possible_truncation)] // rounded value is within the integer range
    samples
        .iter()
        .map(|s| (f64::from(*s) * scale).round() as i32)
        .collect()
}
