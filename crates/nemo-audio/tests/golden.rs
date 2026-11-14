//! Golden decoder tests for the files in `tests/golden/` (see its `README.md`).
//!
//! ALAC and AIFF are lossless: they must decode to exactly the samples of the WAV they were
//! made from. Ogg Vorbis and Opus are lossy: they must match a reference decode (libvorbis,
//! libopus) within a stated tolerance.

mod common;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use common::{test_signal, to_ints};
use nemo_audio::decoder::decode_file;
use nemo_audio::source::FileSource;
use nemo_audio::stream_decoder::StreamDecoder;
use nemo_core::audio::{AudioFormat, Codec, Origin};
use sha2::{Digest, Sha256};

/// Frames in every golden file: 0.25 s at 48 kHz.
const FRAMES: usize = 12_000;

/// Largest allowed difference from the libvorbis reference, in 16-bit steps. Both decoders
/// work in floating point; the reference is rounded to 16 bits, so a few steps of difference
/// are rounding, not a decoder fault.
const VORBIS_MAX_STEPS: i32 = 4;

/// Frames the Ogg reader of `ffmpeg` drops from the end of the reference decode (see README). The
/// reference is compared over the frames it holds, which start at the same frame.
const VORBIS_REFERENCE_SHORT: usize = 128;

/// Largest allowed difference from the libopus reference decode, in 16-bit steps. Both decoders
/// are libopus in floating point, possibly of different versions; the reference is rounded to 16
/// bits.
const OPUS_MAX_STEPS: i32 = 4;

/// Frames the `WebM` file decodes to. The `OpusHead` pre-skip is dropped, but symphonia 0.6.1's
/// Matroska reader ignores `DiscardPadding`, so the padding of the last packet stays (see
/// README): 13 packets of 960 frames, less 312 frames of pre-skip.
const OPUS_WEBM_FRAMES: usize = 13 * 960 - 312;

fn golden(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(name)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut hex, b| {
            write!(hex, "{b:02x}").expect("invariant: writing to a String cannot fail");
            hex
        })
}

#[test]
fn golden_files_match_their_checksums() {
    let sums = std::fs::read_to_string(golden("SHA256SUMS")).expect("SHA256SUMS");
    let mut checked = 0;
    for line in sums.lines().filter(|l| !l.trim().is_empty()) {
        let (want, name) = line.split_once("  ").expect("`<sha256>  <file>` line");
        let bytes = std::fs::read(golden(name)).expect("golden file");
        assert_eq!(sha256_hex(&bytes), want, "{name} changed");
        checked += 1;
    }
    assert_eq!(checked, 9, "every golden file is listed");
}

fn check_lossless(name: &str, bits: u32, codec: Codec) {
    let decoded = decode_file(&golden(name)).expect("decode");
    assert_eq!(decoded.info.origin, Origin::Local);
    assert_eq!(decoded.info.codec, codec, "{name}");
    assert!(codec.is_lossless());
    assert_eq!(decoded.format.sample_rate, 48_000);
    assert_eq!(decoded.format.channels, 2);
    assert_eq!(
        decoded.format.bit_depth,
        Some(u8::try_from(bits).expect("bits"))
    );
    assert!(decoded.is_exact());
    assert_eq!(decoded.frames(), FRAMES, "{name}: no frame added or lost");
    assert_eq!(
        to_ints(&decoded.samples, bits),
        test_signal(bits, FRAMES),
        "{name} must decode to the WAV samples it was made from"
    );
}

#[test]
fn alac_16_bit_decodes_to_the_wav_samples() {
    check_lossless("alac-16.m4a", 16, Codec::Alac);
}

#[test]
fn alac_24_bit_decodes_to_the_wav_samples() {
    check_lossless("alac-24.m4a", 24, Codec::Alac);
}

#[test]
fn aiff_16_bit_decodes_to_the_wav_samples() {
    check_lossless("aiff-16.aiff", 16, Codec::Pcm);
}

#[test]
fn aiff_24_bit_decodes_to_the_wav_samples() {
    check_lossless("aiff-24.aiff", 24, Codec::Pcm);
}

#[test]
fn ogg_vorbis_matches_the_libvorbis_reference() {
    let decoded = decode_file(&golden("vorbis-tone.ogg")).expect("decode");
    assert_eq!(decoded.info.codec, Codec::Vorbis);
    assert!(decoded.info.codec.is_lossy());
    assert_eq!(decoded.format.sample_rate, 48_000);
    assert_eq!(decoded.format.channels, 2);
    assert_eq!(
        decoded.format.bit_depth, None,
        "lossy: no bit depth claimed"
    );
    assert!(!decoded.is_exact());

    let reference: Vec<i32> = std::fs::read(golden("vorbis-tone-ref.s16"))
        .expect("reference")
        .chunks_exact(2)
        .map(|b| i32::from(i16::from_le_bytes([b[0], b[1]])))
        .collect();
    let got = to_ints(&decoded.samples, 16);
    assert_eq!(
        decoded.frames(),
        FRAMES,
        "the last Ogg page's granule position states {FRAMES} frames"
    );
    assert_eq!(
        reference.len(),
        (FRAMES - VORBIS_REFERENCE_SHORT) * 2,
        "reference length (see README)"
    );
    let worst = got
        .iter()
        .zip(&reference)
        .map(|(a, b)| (a - b).abs())
        .max()
        .unwrap_or(0);
    assert!(
        worst <= VORBIS_MAX_STEPS,
        "largest difference {worst} steps, allowed {VORBIS_MAX_STEPS}"
    );
}

/// The libopus reference decode of the Opus tone (16-bit little-endian, 12 000 stereo frames).
fn opus_reference() -> Vec<i32> {
    let reference: Vec<i32> = std::fs::read(golden("opus-tone-ref.s16"))
        .expect("reference")
        .chunks_exact(2)
        .map(|b| i32::from(i16::from_le_bytes([b[0], b[1]])))
        .collect();
    assert_eq!(reference.len(), FRAMES * 2, "reference length (see README)");
    reference
}

/// Lossy facts, then the first [`FRAMES`] frames against the libopus reference.
fn check_opus(format: AudioFormat, codec: Codec, samples: &[f32]) {
    assert_eq!(codec, Codec::Opus);
    assert!(codec.is_lossy());
    assert_eq!(format.sample_rate, 48_000);
    assert_eq!(format.channels, 2);
    assert_eq!(format.bit_depth, None, "lossy: no bit depth claimed");
    let got = to_ints(&samples[..FRAMES * 2], 16);
    let worst = got
        .iter()
        .zip(&opus_reference())
        .map(|(a, b)| (a - b).abs())
        .max()
        .unwrap_or(i32::MAX);
    assert!(
        worst <= OPUS_MAX_STEPS,
        "largest difference {worst} steps, allowed {OPUS_MAX_STEPS}"
    );
}

#[test]
fn ogg_opus_matches_the_libopus_reference_and_drops_the_pre_skip() {
    let decoded = decode_file(&golden("opus-tone.opus")).expect("decode");
    assert_eq!(decoded.info.origin, Origin::Local);
    assert!(!decoded.is_exact());
    assert_eq!(
        decoded.frames(),
        FRAMES,
        "pre-skip dropped and end padding trimmed by the granule position"
    );
    check_opus(decoded.format, decoded.info.codec, &decoded.samples);
}

#[test]
fn webm_opus_probes_with_no_hint_and_matches_the_libopus_reference() {
    let source = FileSource::open(&golden("opus-tone.webm")).expect("open");
    let mut decoder = StreamDecoder::open(Box::new(source), None).expect("probe");
    let info = *decoder.info();
    let mut samples = Vec::new();
    let mut block = Vec::new();
    while decoder.next_block(&mut block).expect("decode") {
        samples.extend_from_slice(&block);
    }
    assert_eq!(
        samples.len() / 2,
        OPUS_WEBM_FRAMES,
        "pre-skip dropped; the last packet's padding stays"
    );
    check_opus(info.format, info.codec, &samples);
}
