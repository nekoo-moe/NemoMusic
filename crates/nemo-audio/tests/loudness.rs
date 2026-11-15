//! `AudioMeter` on real files written by the test: a calibrated tone, an unreadable path, a
//! file that is not audio and a pre-set cancel flag. No fixture file is needed.

mod common;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use common::fixtures::wav_bytes;
use nemo_audio::loudness::AudioMeter;
use nemo_core::library::{LoudnessError, LoudnessMeter};

fn temp_file(tag: &str, bytes: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nemo-loudness-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(tag);
    std::fs::write(&path, bytes).expect("write fixture");
    path
}

/// A stereo 24-bit 1 kHz sine at -23 dBFS, 48 kHz, 3 s (EBU Tech 3341 case 1).
fn calibrated_tone() -> Vec<i32> {
    let amplitude = 10_f64.powf(-23.0 / 20.0) * f64::from((1 << 23) - 1);
    (0..3 * 48_000_u32)
        .flat_map(|n| {
            let phase = 2.0 * std::f64::consts::PI * 1000.0 * f64::from(n) / 48_000.0;
            #[allow(clippy::cast_possible_truncation, reason = "within the 24-bit range")]
            let v = (amplitude * phase.sin()).round() as i32;
            [v, v]
        })
        .collect()
}

#[test]
fn a_calibrated_tone_file_reads_minus_23_lufs() {
    let path = temp_file("tone.wav", &wav_bytes(&calibrated_tone(), 24, 48_000, 2));
    let got = AudioMeter
        .measure(&path, &AtomicBool::new(false))
        .expect("measured");
    assert!(
        (got.integrated_lufs + 23.0).abs() < 0.1,
        "{} LUFS",
        got.integrated_lufs
    );
    assert!((got.true_peak - 0.0708).abs() < 0.001, "{}", got.true_peak);
    std::fs::remove_file(path).expect("cleanup");
}

#[test]
fn failures_name_no_path() {
    let missing = std::env::temp_dir().join("nemo-loudness-missing-Secret Title.flac");
    let unreadable = AudioMeter.measure(&missing, &AtomicBool::new(false));
    assert_eq!(unreadable, Err(LoudnessError::Unreadable));
    let junk = temp_file("Secret Title.flac", b"not an audio file at all");
    let decode = AudioMeter.measure(&junk, &AtomicBool::new(false));
    assert_eq!(decode, Err(LoudnessError::Decode));
    for err in [unreadable, decode].into_iter().filter_map(Result::err) {
        assert!(!err.to_string().contains("Secret Title"), "{err}");
    }
    let cancelled = AudioMeter.measure(&junk, &AtomicBool::new(true));
    assert_eq!(
        cancelled,
        Err(LoudnessError::Cancelled),
        "checked before opening"
    );
    std::fs::remove_file(junk).expect("cleanup");
}
