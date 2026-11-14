//! Audio content through the streaming player: bit-exact copy, volume, resampling, channel
//! mix, seeking and streaming over slow reads. No device, no network.
#![allow(
    clippy::float_cmp,
    reason = "bit-exact checks compare samples exactly, so exact float equality is the point"
)]

mod common;

use std::time::Duration;

use common::fakes::{Rig, RigOptions, local_track, rig, video_id, video_track};
use common::fixtures::{counter_index, counter_wav, flac_bytes, tone_i16, wav_bytes};
use nemo_audio::decoder::supported_stream_codecs;
use nemo_core::audio::{Codec, ProcessingStage, QualityClass, SignalPath};
use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent};

const LIMIT: Duration = Duration::from_secs(20);

/// The newest signal path the player reported.
fn last_path(r: &mut Rig) -> SignalPath {
    r.drain()
        .iter()
        .rev()
        .find_map(|e| match e {
            PlayerEvent::SignalPathChanged(p) => Some(p.clone()),
            _ => None,
        })
        .expect("a signal path event")
}

fn as_f32(ints: &[i32], bits: u32) -> Vec<f32> {
    let scale = f64::from(1_u32 << (bits - 1));
    #[allow(clippy::cast_possible_truncation)] // exact: 16/24-bit integers fit f32
    ints.iter()
        .map(|v| (f64::from(*v) / scale) as f32)
        .collect()
}

fn temp_file(tag: &str, bytes: &[u8]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("nemo-player-audio-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(tag);
    std::fs::write(&path, bytes).expect("write fixture");
    path
}

#[test]
fn supported_codecs_list_only_what_decodes() {
    let codecs = supported_stream_codecs();
    for c in [Codec::Aac, Codec::Flac, Codec::Mp3, Codec::Pcm] {
        assert!(codecs.contains(&c), "{c:?}");
    }
    assert!(
        !codecs.contains(&Codec::Opus),
        "symphonia 0.6.1 has no Opus decoder"
    );
}

#[test]
fn volume_100_copies_samples_exactly_and_adds_no_stage() {
    let ints = tone_i16(48_000, 2, 5 * 48_000, 1000.0);
    let path = temp_file("v100.wav", &wav_bytes(&ints, 16, 48_000, 2));
    let mut r = rig(vec![], RigOptions::default());
    r.send(PlayerCommand::Play(local_track(1, path.clone())));
    r.wait_samples(2 * 48_000 + 2000, LIMIT);
    let want = as_f32(&ints[..2 * 48_000], 16);
    assert_eq!(
        r.captured()[..want.len()],
        want[..],
        "bit-exact at volume 100"
    );
    let path_report = last_path(&mut r);
    assert_eq!(path_report.processing, Vec::new());
    assert_eq!(r.player.snapshot().underruns, 0);
    std::fs::remove_file(path).expect("cleanup");
}

#[test]
fn volume_50_halves_every_sample_and_reports_gain() {
    let ints = tone_i16(48_000, 2, 10 * 48_000, 440.0);
    let path = temp_file("v50.wav", &wav_bytes(&ints, 16, 48_000, 2));
    let mut r = rig(vec![], RigOptions::default());
    r.send(PlayerCommand::SetVolume(50));
    r.send(PlayerCommand::Play(local_track(1, path.clone())));
    r.wait_samples(2 * 24_000 + 2000, LIMIT);
    let want: Vec<f32> = as_f32(&ints[..2 * 24_000], 16)
        .iter()
        .map(|s| s * 0.5)
        .collect();
    assert_eq!(r.captured()[..want.len()], want[..], "exactly half");
    let snap = r.player.snapshot();
    assert_eq!(snap.volume_percent, 50);
    let stages = last_path(&mut r).processing;
    assert!(
        matches!(stages.as_slice(), [ProcessingStage::Gain { db }] if (*db + 6.0206).abs() < 0.01),
        "{stages:?}"
    );
    // Back to 100: the stage disappears again.
    r.send(PlayerCommand::SetVolume(100));
    r.wait_event(
        "gain stage to go",
        LIMIT,
        |e| matches!(e, PlayerEvent::SignalPathChanged(p) if p.processing.is_empty()),
    );
    assert!(r.drain().contains(&PlayerEvent::VolumeChanged(50)));
    std::fs::remove_file(path).expect("cleanup");
}

/// Indices of the left channel where the sign changes.
fn zero_crossings(left: &[f32]) -> Vec<usize> {
    left.windows(2)
        .enumerate()
        .filter(|(_, w)| (w[0] < 0.0) != (w[1] < 0.0))
        .map(|(i, _)| i)
        .collect()
}

#[test]
fn resampling_44100_to_48000_keeps_length_and_a_single_frequency() {
    let ints = tone_i16(44_100, 2, 88_200, 1000.0); // 2 s
    let path = temp_file("rs.wav", &wav_bytes(&ints, 16, 44_100, 2));
    // Half real time: the sinc resampler is slow in an unoptimised build.
    let mut r = rig(
        vec![],
        RigOptions {
            interval: Duration::from_millis(20),
            ..RigOptions::default()
        },
    );
    r.send(PlayerCommand::Play(local_track(1, path.clone())));
    r.wait_event("end of track", LIMIT, |e| {
        *e == PlayerEvent::StateChanged(PlayState::Stopped)
    });
    let out = r.captured();
    let last = out.iter().rposition(|s| *s != 0.0).expect("audio");
    let frames = last.div_ceil(2);
    assert!(frames.abs_diff(96_000) <= 1, "got {frames} frames");
    let left: Vec<f32> = out
        .as_chunks::<2>()
        .0
        .iter()
        .take(96_000)
        .map(|f| f[0])
        .collect();
    let inner = &left[4800..left.len() - 4800];
    let crossings = zero_crossings(inner);
    // 1 kHz: two crossings per millisecond. Inner span is 1.8 s.
    assert!(crossings.len().abs_diff(3600) <= 2, "{}", crossings.len());
    assert!(
        crossings
            .windows(2)
            .all(|w| (23..=25).contains(&(w[1] - w[0]))),
        "crossing spacing is not a steady 24 samples"
    );
    let path_report = last_path(&mut r);
    assert_eq!(
        path_report.processing,
        vec![ProcessingStage::Resample {
            from_hz: 44_100,
            to_hz: 48_000
        }]
    );
    assert!(!path_report.is_bit_perfect());
    std::fs::remove_file(path).expect("cleanup");
}

#[test]
fn mono_source_is_duplicated_into_stereo() {
    let ints = tone_i16(48_000, 1, 24_000, 500.0);
    let path = temp_file("mono.wav", &wav_bytes(&ints, 16, 48_000, 1));
    let mut r = rig(vec![], RigOptions::default());
    r.send(PlayerCommand::Play(local_track(1, path.clone())));
    r.wait_event("end of track", LIMIT, |e| {
        *e == PlayerEvent::StateChanged(PlayState::Stopped)
    });
    let out = r.captured();
    let mono = as_f32(&ints, 16);
    for (i, m) in mono.iter().enumerate() {
        assert_eq!(out[2 * i], *m);
        assert_eq!(out[2 * i + 1], *m);
    }
    let stages = last_path(&mut r).processing;
    assert_eq!(stages, vec![ProcessingStage::ChannelMix { from: 1, to: 2 }]);
    std::fs::remove_file(path).expect("cleanup");
}

/// Left-channel counter values of every frame in `out`.
fn indices(out: &[f32]) -> Vec<i64> {
    out.as_chunks::<2>()
        .0
        .iter()
        .map(|f| counter_index(f[0]))
        .collect()
}

fn check_seek(bytes: &[u8], tag: &str) {
    let path = temp_file(tag, bytes);
    let mut r = rig(
        vec![],
        RigOptions {
            interval: Duration::from_millis(10), // real time, so we seek before 1 s is reached
            ..RigOptions::default()
        },
    );
    r.send(PlayerCommand::Play(local_track(1, path.clone())));
    r.wait_samples(2 * 4800, LIMIT);
    let before = r.captured().len();
    r.send(PlayerCommand::Seek(Duration::from_secs(1)));
    r.wait_samples(before + 2 * 24_000, LIMIT);
    let idx = indices(&r.captured());
    let at = idx
        .iter()
        .position(|i| *i == 48_000)
        .expect("frame 48000 is played after the seek");
    assert!(
        idx[at..at + 20_000]
            .iter()
            .enumerate()
            .all(|(k, i)| *i == 48_000 + i64::try_from(k).expect("small")),
        "frames after the seek are consecutive"
    );
    assert!(
        r.player.position() >= Duration::from_secs(1),
        "position {:?}",
        r.player.position()
    );
    std::fs::remove_file(path).expect("cleanup");
}

#[test]
fn seek_lands_on_the_exact_frame_in_wav() {
    check_seek(&counter_wav(48_000, 144_000), "seek.wav");
}

#[test]
fn seek_lands_on_the_exact_frame_in_flac() {
    // 24-bit counters do not fit the 16-bit FLAC fixture, so use 16-bit frame numbers.
    let frames = 4096 * 30; // 122 880 frames, 2.56 s
    let mut s = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let v = i16::try_from(i % 30_000 + 1).expect("fits");
        s.extend([v, -v]);
    }
    let path = temp_file("seek.flac", &flac_bytes(&s, 48_000, 2));
    let mut r = rig(
        vec![],
        RigOptions {
            interval: Duration::from_millis(10),
            ..RigOptions::default()
        },
    );
    r.send(PlayerCommand::Play(local_track(1, path.clone())));
    r.wait_samples(2 * 4800, LIMIT);
    let before = r.captured().len();
    r.send(PlayerCommand::Seek(Duration::from_secs(1)));
    r.wait_samples(before + 2 * 24_000, LIMIT);
    // Frame 48000 holds 48000 % 30000 + 1 = 18001 in the left channel.
    let out = r.captured();
    let left: Vec<f32> = out.as_chunks::<2>().0.iter().map(|f| f[0]).collect();
    let want: Vec<f32> = (0..2000)
        .map(|k| f32::from(i16::try_from(18_001 + k).expect("fits")) / 32768.0)
        .collect();
    assert!(
        left.windows(want.len()).any(|w| w == want.as_slice()),
        "the seek target frame and its successors play back unchanged"
    );
    std::fs::remove_file(path).expect("cleanup");
}

#[test]
fn a_stream_with_small_reads_plays_bit_exact_with_zero_underruns() {
    let ints = tone_i16(48_000, 2, 5 * 48_000, 330.0);
    let bytes = wav_bytes(&ints, 16, 48_000, 2);
    let mut r = rig(
        vec![(video_id(1), bytes)],
        RigOptions {
            chunk: Some(700),
            ..RigOptions::default() // 4x real time
        },
    );
    r.send(PlayerCommand::Play(video_track(1)));
    r.wait_event("end of track", LIMIT, |e| {
        *e == PlayerEvent::StateChanged(PlayState::Stopped)
    });
    let want = as_f32(&ints, 16);
    assert_eq!(r.captured()[..want.len()], want[..]);
    let snap = r.player.snapshot();
    assert_eq!(
        snap.underruns, 0,
        "no underruns while draining at 4x real time"
    );
    let info = last_path(&mut r).source;
    assert_eq!(
        info.quality_class(),
        QualityClass::Lossy,
        "a stream is never lossless"
    );
    assert!(
        r.drain()
            .contains(&PlayerEvent::StateChanged(PlayState::Playing))
    );
}
