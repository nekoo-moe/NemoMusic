#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "test signals and frame counts are written from positive f64 values in range"
)]

use std::f64::consts::PI;
use std::sync::atomic::{AtomicBool, Ordering};

use nemo_core::audio::AudioFormat;
use nemo_core::library::LoudnessError;

use super::measure_blocks;

const BLOCK_FRAMES: u32 = 4096;

fn format(sample_rate: u32, channels: u16) -> AudioFormat {
    AudioFormat {
        sample_rate,
        channels,
        bit_depth: None,
    }
}

/// A block source of `seconds` of a sine at `freq` Hz and `dbfs` peak level on every channel.
fn sine(
    format: AudioFormat,
    freq: f64,
    dbfs: f64,
    seconds: f64,
) -> impl FnMut(&mut Vec<f32>) -> Result<bool, LoudnessError> {
    let amplitude = 10_f64.powf(dbfs / 20.0);
    let rate = f64::from(format.sample_rate);
    let total = (seconds * rate) as u32;
    let channels = usize::from(format.channels);
    let mut at = 0_u32;
    move |block: &mut Vec<f32>| {
        block.clear();
        if at >= total {
            return Ok(false);
        }
        let end = (at + BLOCK_FRAMES).min(total);
        for n in at..end {
            let s = (amplitude * (2.0 * PI * freq * f64::from(n) / rate).sin()) as f32;
            block.extend(std::iter::repeat_n(s, channels));
        }
        at = end;
        Ok(true)
    }
}

#[test]
fn a_1_khz_sine_at_minus_23_dbfs_reads_minus_23_lufs() {
    // EBU Tech 3341 case 1 and 2: a stereo 1 kHz sine at -23 or -33 dBFS reads that many LUFS.
    for (rate, dbfs) in [(48_000, -23.0), (44_100, -23.0), (48_000, -33.0)] {
        let f = format(rate, 2);
        let got = measure_blocks(f, &AtomicBool::new(false), sine(f, 1000.0, dbfs, 3.0))
            .expect("measured");
        let lufs = f64::from(got.integrated_lufs);
        assert!(
            (lufs - dbfs).abs() < 0.1,
            "{rate} Hz, {dbfs} dBFS: {lufs} LUFS"
        );
        let peak = 10_f64.powf(dbfs / 20.0);
        let ratio = f64::from(got.true_peak) / peak;
        assert!(
            (0.99..1.01).contains(&ratio),
            "true peak {} of {peak}",
            got.true_peak
        );
    }
}

#[test]
fn a_mono_file_reads_like_the_same_signal_on_two_speakers() {
    let mono = format(48_000, 1);
    let got = measure_blocks(
        mono,
        &AtomicBool::new(false),
        sine(mono, 1000.0, -23.0, 3.0),
    )
    .expect("measured");
    assert!(
        (f64::from(got.integrated_lufs) + 23.0).abs() < 0.1,
        "{got:?}"
    );
}

#[test]
fn cancel_stops_before_the_next_block() {
    let cancel = AtomicBool::new(false);
    let f = format(48_000, 2);
    let mut source = sine(f, 1000.0, -23.0, 10.0);
    let mut blocks = 0;
    let result = measure_blocks(f, &cancel, |block| {
        blocks += 1;
        if blocks == 3 {
            cancel.store(true, Ordering::Relaxed);
        }
        source(block)
    });
    assert_eq!(result, Err(LoudnessError::Cancelled));
    assert_eq!(blocks, 3, "no block is decoded after the flag is set");
}

#[test]
fn silence_too_little_audio_or_no_channel_is_not_measurable() {
    let f = format(48_000, 2);
    let quiet = measure_blocks(f, &AtomicBool::new(false), sine(f, 1000.0, -200.0, 2.0));
    assert_eq!(quiet, Err(LoudnessError::Measure), "silence");
    let short = measure_blocks(f, &AtomicBool::new(false), sine(f, 1000.0, -23.0, 0.2));
    assert_eq!(short, Err(LoudnessError::Measure), "under one 400 ms block");
    let none = format(48_000, 0);
    let empty = measure_blocks(none, &AtomicBool::new(false), |_| Ok(false));
    assert_eq!(empty, Err(LoudnessError::Measure));
    let decode = measure_blocks(f, &AtomicBool::new(false), |_| Err(LoudnessError::Decode));
    assert_eq!(
        decode,
        Err(LoudnessError::Decode),
        "a decode error is passed on"
    );
}
