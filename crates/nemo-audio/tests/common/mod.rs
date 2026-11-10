//! Synthetic WAV fixtures written from code. Shared by the integration tests.

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
    let bytes_per = bits / 8;
    let data_len = u32::try_from(samples.len()).expect("invariant: small file") * bytes_per;
    let block = u16::try_from(bytes_per).expect("invariant: 2 or 3") * channels;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * u32::from(block)).to_le_bytes());
    wav.extend_from_slice(&block.to_le_bytes());
    wav.extend_from_slice(
        &u16::try_from(bits)
            .expect("invariant: 16 or 24")
            .to_le_bytes(),
    );
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        wav.extend_from_slice(&s.to_le_bytes()[..usize::try_from(bytes_per).expect("invariant")]);
    }
    fs::write(path, wav).expect("test fixture write");
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
