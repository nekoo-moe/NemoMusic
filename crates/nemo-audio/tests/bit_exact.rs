//! Local 16-bit and 24-bit WAV files survive decode -> ring -> render -> sink bit for bit.

mod common;

use std::sync::atomic::AtomicU64;

use common::{test_signal, to_ints, write_wav};
use nemo_audio::decoder::decode_file;
use nemo_audio::engine::Engine;
use nemo_audio::render::render;
use nemo_audio::ring::{new_ring, push_slice};
use nemo_audio::sink::CaptureSink;
use nemo_core::audio::{Codec, Origin, OutputMode};

const FRAMES: usize = 48_000;

fn temp_wav(bits: u32, tag: &str) -> (std::path::PathBuf, Vec<i32>) {
    let dir = std::env::temp_dir().join(format!("nemo-audio-bit-exact-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(format!("{tag}-{bits}.wav"));
    let source = test_signal(bits, FRAMES);
    write_wav(&path, &source, bits, 48_000, 2);
    (path, source)
}

fn check_bits(bits: u32) {
    let (path, source) = temp_wav(bits, "roundtrip");
    let decoded = decode_file(&path).expect("decode");
    assert_eq!(decoded.info.origin, Origin::Local);
    assert_eq!(decoded.info.codec, Codec::Pcm);
    assert_eq!(decoded.format.sample_rate, 48_000);
    assert_eq!(decoded.format.channels, 2);
    assert_eq!(
        decoded.format.bit_depth,
        Some(u8::try_from(bits).expect("bits"))
    );
    assert!(decoded.is_exact());
    assert_eq!(to_ints(&decoded.samples, bits), source, "decode alone");

    // Direct ring + render path, 512-frame callbacks, whole file buffered.
    let (mut producer, mut consumer) = new_ring(decoded.samples.len());
    assert_eq!(
        push_slice(&mut producer, &decoded.samples),
        decoded.samples.len()
    );
    let underruns = AtomicU64::new(0);
    let mut captured = Vec::new();
    let mut out = vec![0.0_f32; 512 * 2];
    while captured.len() < decoded.samples.len() {
        let got = render(&mut consumer, &mut out, &underruns);
        captured.extend_from_slice(&out[..got]);
    }
    assert_eq!(
        captured, decoded.samples,
        "ring + render must not change a sample"
    );
    assert_eq!(to_ints(&captured, bits), source);

    // Whole engine into the capture sink.
    let sink = CaptureSink::new(decoded.format);
    let mut engine = Engine::start(decoded.clone(), sink, decoded.samples.len()).expect("start");
    while !engine.is_finished() {
        engine.sink_mut().pump(512);
    }
    let got = &engine.sink_mut().captured()[..decoded.samples.len()];
    assert_eq!(to_ints(got, bits), source, "engine output");
    assert_eq!(engine.underruns(), 0);
    let path_report = engine.signal_path();
    assert_eq!(path_report.processing, Vec::new());
    assert_eq!(path_report.output.mode, OutputMode::Shared);
    assert!(
        !path_report.is_bit_perfect(),
        "shared mode is never bit-perfect"
    );
    engine.stop();
    std::fs::remove_file(path).expect("cleanup");
}

#[test]
fn sixteen_bit_wav_is_bit_exact() {
    check_bits(16);
}

#[test]
fn twenty_four_bit_wav_is_bit_exact() {
    check_bits(24);
}

#[test]
fn engine_rejects_a_sink_with_another_rate() {
    let (path, _) = temp_wav(16, "mismatch");
    let decoded = decode_file(&path).expect("decode");
    let mut wrong = decoded.format;
    wrong.sample_rate = 44_100;
    let err = Engine::start(decoded, CaptureSink::new(wrong), 1024).err();
    assert!(err.is_some_and(|e| e.to_string().contains("does not match")));
    std::fs::remove_file(path).expect("cleanup");
}

/// The equalizer stage: a Flat ten-band equalizer is ten identity filters, and running every
/// sample through them in `f64` gives back every decoded sample bit for bit.
#[test]
fn a_flat_ten_band_equalizer_keeps_every_sample() {
    use nemo_audio::effects::{Biquad, Coefficients, ten_band};

    for bits in [16, 24] {
        let (path, source) = temp_wav(bits, "flat-eq");
        let decoded = decode_file(&path).expect("decode");
        let flat = ten_band([0; 10]).map(|band| {
            band.map_or(Coefficients::IDENTITY, |f| {
                Coefficients::for_filter(&f, 48_000)
            })
        });
        assert!(flat.iter().all(|c| *c == Coefficients::IDENTITY));
        let mut channels = [flat.map(Biquad::new); 2];
        let mut out = decoded.samples.clone();
        for frame in out.chunks_exact_mut(2) {
            for (sample, bank) in frame.iter_mut().zip(channels.iter_mut()) {
                *sample = bank.iter_mut().fold(*sample, |s, b| b.process(s));
            }
        }
        let same = out
            .iter()
            .zip(&decoded.samples)
            .all(|(a, b)| a.to_bits() == b.to_bits());
        assert!(same, "{bits}-bit samples changed");
        assert_eq!(to_ints(&out, bits), source);
        std::fs::remove_file(path).expect("cleanup");
    }
}

/// The delay stage: at a mix of 0 the dry path keeps every decoded sample bit for bit, with
/// the line running underneath.
#[test]
fn a_delay_at_mix_0_keeps_every_sample() {
    use nemo_audio::effects::{Delay, DelayParams};
    use nemo_core::effects::DelaySettings;

    for bits in [16, 24] {
        let (path, source) = temp_wav(bits, "dry-delay");
        let decoded = decode_file(&path).expect("decode");
        let settings = DelaySettings {
            enabled: true,
            time_ms: 120,
            feedback_pct: 85,
            mix_pct: 0,
        };
        let mut delay = Delay::new(2, 48_000, DelayParams::new(&settings, 48_000));
        let mut out = decoded.samples.clone();
        for block in out.chunks_mut(512 * 2) {
            delay.process(block);
        }
        let same = out
            .iter()
            .zip(&decoded.samples)
            .all(|(a, b)| a.to_bits() == b.to_bits());
        assert!(same, "{bits}-bit samples changed");
        assert_eq!(to_ints(&out, bits), source);
        std::fs::remove_file(path).expect("cleanup");
    }
}

/// The reverb stage: at a mix of 0 the dry path keeps every decoded sample bit for bit, with
/// the combs running underneath.
#[test]
fn a_reverb_at_mix_0_keeps_every_sample() {
    use nemo_audio::effects::{Reverb, ReverbParams};
    use nemo_core::effects::ReverbSettings;

    for bits in [16, 24] {
        let (path, source) = temp_wav(bits, "dry-reverb");
        let decoded = decode_file(&path).expect("decode");
        let settings = ReverbSettings {
            enabled: true,
            size_pct: 100,
            damping_pct: 0,
            mix_pct: 0,
        };
        let mut reverb = Reverb::new(2, 48_000, ReverbParams::new(&settings, 48_000));
        let mut out = decoded.samples.clone();
        for block in out.chunks_mut(512 * 2) {
            reverb.process(block);
        }
        let same = out
            .iter()
            .zip(&decoded.samples)
            .all(|(a, b)| a.to_bits() == b.to_bits());
        assert!(same, "{bits}-bit samples changed");
        assert_eq!(to_ints(&out, bits), source);
        std::fs::remove_file(path).expect("cleanup");
    }
}
