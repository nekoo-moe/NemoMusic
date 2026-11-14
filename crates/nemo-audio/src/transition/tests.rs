use nemo_core::audio::AudioFormat;
use nemo_core::effects::{TransitionKind, TransitionMode};

use super::{
    SILENCE_FLOOR, TransitionMixer, TransitionPlan, equal_power, leading_silence, trailing_silence,
};

const RATE: u32 = 48_000;
const MS: usize = 48;

fn stereo() -> AudioFormat {
    AudioFormat {
        sample_rate: RATE,
        channels: 2,
        bit_depth: None,
    }
}

fn mixer(mode: TransitionMode, frames: usize) -> TransitionMixer {
    let plan = TransitionPlan {
        kind_hint: mode,
        frames: u64::try_from(frames).expect("small"),
    };
    TransitionMixer::new(stereo(), plan)
}

/// `frames` stereo frames of a steady tone at `level`.
fn tone(frames: usize, level: f32) -> Vec<f32> {
    (0..frames)
        .flat_map(|i| {
            let s = level * if i % 2 == 0 { 1.0 } else { -0.5 };
            [s, -s]
        })
        .collect()
}

/// `frames` stereo frames just below the silence floor.
fn hush(frames: usize) -> Vec<f32> {
    tone(frames, SILENCE_FLOOR * 0.5)
}

fn joined(parts: &[Vec<f32>]) -> Vec<f32> {
    parts.concat()
}

/// Push `song` in 512-frame blocks; returns what reached the ring.
fn play(m: &mut TransitionMixer, song: &[f32]) -> Vec<f32> {
    let mut out = Vec::new();
    for block in song.chunks(1024) {
        m.push(block, &mut out);
    }
    out
}

#[test]
fn equal_power_gains_keep_the_power_constant() {
    for n in [1, 7, 300, 48_000] {
        for i in 0..n {
            let (out_gain, in_gain) = equal_power(i, n);
            let power = out_gain * out_gain + in_gain * in_gain;
            assert!((power - 1.0).abs() < 1e-6, "{i} of {n}: {power}");
        }
        for i in 0..n {
            let (out_gain, _) = equal_power(i, n);
            let (_, in_gain) = equal_power(n - 1 - i, n);
            assert!(
                (out_gain - in_gain).abs() < 1e-6,
                "the fades mirror each other"
            );
        }
    }
    let (start_out, start_in) = equal_power(0, 48_000);
    assert!(
        start_out > 0.999_99 && start_in < 1e-4,
        "the outgoing song starts at full level"
    );
}

#[test]
fn a_crossfade_overlaps_the_whole_tail_with_constant_power() {
    let l = 600 * MS;
    let fade = |tail: f32, head: f32| {
        let mut m = mixer(TransitionMode::Crossfade, l);
        let mut out = play(&mut m, &vec![tail; 2 * 2 * l]);
        assert_eq!(out.len(), 2 * l, "the window holds back L frames");
        assert_eq!(m.window_frames(), l);
        let kind = m.at_end(&vec![head; 2 * 3 * l], false);
        let start = out.len();
        let t = m.mix_into(kind, &vec![head; 2 * 3 * l], &mut out);
        (t, out.split_off(start))
    };
    let (t, outgoing) = fade(1.0, 0.0);
    let (_, incoming) = fade(0.0, 1.0);
    assert_eq!(t.kind, TransitionKind::Crossfade);
    assert_eq!(
        (t.overlap_start, t.overlap_frames, t.head_skipped),
        (0, l, 0)
    );
    assert_eq!(t.head_used(), l);
    assert_eq!(t.handover_position(), l / 2, "Started carries base = L/2");
    assert_eq!(t.handover_frame(), l / 2);
    for (a, b) in outgoing.iter().zip(&incoming) {
        assert!((a * a + b * b - 1.0).abs() < 1e-5, "{a} {b}");
    }
}

#[test]
fn a_short_window_or_head_shrinks_the_crossfade() {
    let l = 1000 * MS;
    let mut m = mixer(TransitionMode::Crossfade, l);
    let mut out = play(&mut m, &tone(300 * MS, 0.5));
    assert!(out.is_empty());
    assert_eq!(m.window_frames(), 300 * MS);
    let t = m.mix_into(TransitionKind::Crossfade, &tone(5000 * MS, 0.5), &mut out);
    assert_eq!(t.overlap_frames, 300 * MS, "the window was not full");
    let mut m = mixer(TransitionMode::Crossfade, l);
    let mut out = play(&mut m, &tone(3000 * MS, 0.5));
    let t = m.mix_into(TransitionKind::Crossfade, &tone(200 * MS, 0.5), &mut out);
    assert_eq!(t.overlap_frames, 200 * MS, "the next song is short");
    let frames = out.len() / 2;
    assert_eq!(
        frames,
        3000 * MS + 200 * MS - t.overlap_frames,
        "no frame lost or doubled"
    );
}

#[test]
fn smart_mode_picks_per_pair() {
    let l = 2000 * MS;
    let pick = |mode, tail: Vec<f32>, head: Vec<f32>, policy| {
        let mut m = mixer(mode, l);
        let _ = play(&mut m, &tail);
        m.at_end(&head, policy)
    };
    let loud = || tone(5000 * MS, 0.5);
    let smart = TransitionMode::Smart;
    assert_eq!(
        pick(smart, loud(), loud(), false),
        TransitionKind::Crossfade
    );
    let ends_quiet = joined(&[loud(), hush(1000 * MS)]);
    assert_eq!(
        pick(smart, ends_quiet.clone(), loud(), false),
        TransitionKind::TrimSilence
    );
    let starts_quiet = joined(&[hush(2000 * MS), loud()]);
    assert_eq!(
        pick(smart, loud(), starts_quiet, false),
        TransitionKind::TrimSilence
    );
    let clean_end = joined(&[loud(), hush(20 * MS)]);
    let clean_start = joined(&[hush(30 * MS), loud()]);
    assert_eq!(
        pick(smart, clean_end, clean_start.clone(), false),
        TransitionKind::Gapless
    );
    assert_eq!(
        pick(smart, loud(), clean_start, false),
        TransitionKind::Crossfade
    );
    assert_eq!(
        pick(smart, loud(), loud(), true),
        TransitionKind::Gapless,
        "policy"
    );
    let plain = TransitionMode::Crossfade;
    assert_eq!(
        pick(plain, ends_quiet, loud(), false),
        TransitionKind::Crossfade,
        "no scan"
    );
    assert_eq!(
        pick(TransitionMode::Off, loud(), loud(), false),
        TransitionKind::Gapless
    );
}

#[test]
fn the_mode_off_has_no_window() {
    let mut m = mixer(TransitionMode::Off, 6000 * MS);
    let song = tone(100 * MS, 0.5);
    assert_eq!(play(&mut m, &song), song);
    assert_eq!(m.window_frames(), 0);
}

#[test]
fn a_silence_trim_removes_only_samples_below_60_db() {
    let l = 3000 * MS;
    // A sample at -59 dBFS 400 ms before the end must stay; the quiet after it goes.
    let mut tail = joined(&[tone(4000 * MS, 0.5), hush(600 * MS), hush(400 * MS)]);
    let edge = tail.len() - 2 * 400 * MS - 2;
    tail[edge] = 1.12e-3;
    let head = joined(&[hush(700 * MS), tone(4000 * MS, 0.25)]);
    let mut m = mixer(TransitionMode::Smart, l);
    let mut out = play(&mut m, &tail);
    let before = out.len();
    let window: Vec<f32> = tail[tail.len() - 2 * l..].to_vec();
    assert_eq!(m.at_end(&head, false), TransitionKind::TrimSilence);
    let t = m.mix_into(TransitionKind::TrimSilence, &head, &mut out);
    let keep = l - 400 * MS;
    assert_eq!(
        t.overlap_start + t.overlap_frames,
        keep,
        "trimmed up to the -59 dB sample"
    );
    assert_eq!(t.overlap_frames, 300 * MS);
    assert_eq!(t.head_skipped, 700 * MS);
    let dropped_tail = &window[2 * keep..];
    let dropped_head = &head[..2 * t.head_skipped];
    assert!(
        dropped_tail
            .iter()
            .chain(dropped_head)
            .all(|s| s.abs() < SILENCE_FLOOR)
    );
    assert_eq!(
        trailing_silence(&window[..2 * keep], 2),
        0,
        "the kept tail ends in sound"
    );
    assert_eq!(leading_silence(&head[2 * t.head_skipped..], 2), 0);
    assert_eq!(
        out.len() - before,
        2 * keep,
        "the tail up to the overlap, then the overlap"
    );
}

#[test]
fn a_gapless_join_changes_nothing() {
    let mut m = mixer(TransitionMode::Smart, 500 * MS);
    let first = tone(1500 * MS, 0.7);
    let second = tone(800 * MS, 0.3);
    let mut out = play(&mut m, &first);
    let t = m.mix_into(TransitionKind::Gapless, &second, &mut out);
    assert_eq!(t.head_used(), 0);
    assert_eq!(t.handover_frame(), 500 * MS);
    out.extend_from_slice(&second[2 * t.head_used()..]);
    let same = out
        .iter()
        .zip(first.iter().chain(&second))
        .all(|(a, b)| a.to_bits() == b.to_bits());
    assert!(same && out.len() == first.len() + second.len());
}
