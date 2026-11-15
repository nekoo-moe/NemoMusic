use nemo_core::effects::{DelaySettings, EffectsSettings, EqSettings, ReverbSettings};
use rtrb::Producer;

use super::{EffectsChain, EffectsUpdate, effects_ring, tenth_db_to_gain};
use crate::effects::{DelayParams, ten_band};

const RATE: u32 = 48_000;
/// 10 ms at 48 kHz.
const RAMP: usize = 480;

fn chain() -> (Producer<EffectsUpdate>, EffectsChain) {
    let (tx, rx) = effects_ring();
    (tx, EffectsChain::new(2, RATE, rx))
}

fn push(tx: &mut Producer<EffectsUpdate>, settings: &EffectsSettings) {
    assert!(tx.push(EffectsUpdate::new(settings, RATE)).is_ok());
}

/// The equalizer on with only a preamp, so the settled output is `x * gain` exactly.
fn preamp_only(preamp_tenth_db: i16) -> EffectsSettings {
    EffectsSettings {
        equalizer: EqSettings {
            enabled: true,
            preamp_tenth_db,
            ..EqSettings::default()
        },
        ..EffectsSettings::default()
    }
}

fn delay(time_ms: u16, feedback_pct: u8, mix_pct: u8) -> EffectsSettings {
    EffectsSettings {
        delay: DelaySettings {
            enabled: true,
            time_ms,
            feedback_pct,
            mix_pct,
        },
        ..EffectsSettings::default()
    }
}

/// `frames` stereo frames of `value`, run through `chain` in blocks of 128 frames.
fn run(chain: &mut EffectsChain, frames: usize, value: f32) -> Vec<f32> {
    let mut out = vec![value; frames * 2];
    for block in out.chunks_mut(256) {
        chain.process(block);
    }
    out
}

fn sine(frames: usize) -> Vec<f32> {
    (0..frames * 2)
        .map(|i| f32::from(u16::try_from(i % 4096).unwrap_or(0)) * 0.05)
        .map(|phase| phase.sin() * 0.4)
        .collect()
}

#[test]
fn every_effect_off_leaves_the_samples_bit_identical() {
    let (mut tx, mut chain) = chain();
    let input = sine(2_000);
    let mut out = input.clone();
    chain.process(&mut out);
    push(&mut tx, &EffectsSettings::default());
    chain.process(&mut out);
    let bits = |s: &[f32]| s.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&out), bits(&input));
    assert!(!chain.is_active());
    assert_eq!(chain.clipped_samples(), 0);
}

#[test]
fn turning_the_equalizer_on_ramps_to_its_level_inside_10_ms() {
    let (mut tx, mut chain) = chain();
    push(&mut tx, &preamp_only(-60));
    let target = 0.5 * tenth_db_to_gain(-60);
    let out = run(&mut chain, RAMP + 512, 0.5);
    let left: Vec<f32> = out.iter().step_by(2).copied().collect();
    assert!(
        left[..RAMP].windows(2).all(|w| w[1] < w[0]),
        "falls each frame"
    );
    assert!(
        (left[RAMP - 1] - target).abs() < 1e-6,
        "at the target after 10 ms"
    );
    assert!(left[RAMP..].iter().all(|s| s.to_bits() == target.to_bits()));
    assert!(chain.is_active());
}

#[test]
fn turning_every_effect_off_again_gives_bit_identical_output() {
    let (mut tx, mut chain) = chain();
    let mut on = preamp_only(-60);
    on.delay = delay(20, 40, 30).delay;
    on.reverb = ReverbSettings {
        enabled: true,
        ..ReverbSettings::default()
    };
    push(&mut tx, &on);
    run(&mut chain, 4_800, 0.25);
    push(&mut tx, &EffectsSettings::default());
    run(&mut chain, RAMP + 256, 0.25);
    assert!(!chain.is_active(), "off once the ramp ended");
    let input = sine(1_000);
    let mut out = input.clone();
    chain.process(&mut out);
    assert!(
        out.iter()
            .zip(&input)
            .all(|(a, b)| a.to_bits() == b.to_bits())
    );
}

#[test]
fn a_preamp_change_crossfades_without_a_step() {
    let (mut tx, mut chain) = chain();
    push(&mut tx, &preamp_only(-60));
    run(&mut chain, 1_024, 0.5);
    push(&mut tx, &preamp_only(-120));
    let (old, new) = (0.5 * tenth_db_to_gain(-60), 0.5 * tenth_db_to_gain(-120));
    let out = run(&mut chain, RAMP + 256, 0.5);
    let left: Vec<f32> = out.iter().step_by(2).copied().collect();
    let largest_step = left
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold((left[0] - old).abs(), f32::max);
    assert!(largest_step <= (old - new) / 400.0, "step {largest_step}");
    assert!(left[RAMP..].iter().all(|s| s.to_bits() == new.to_bits()));
}

#[test]
fn the_newest_waiting_update_wins() {
    let (mut tx, mut chain) = chain();
    for preamp in [-30, -60, -90] {
        push(&mut tx, &preamp_only(preamp));
    }
    let out = run(&mut chain, RAMP + 256, 0.5);
    let want = 0.5 * tenth_db_to_gain(-90);
    assert_eq!(out.last().map(|s| s.to_bits()), Some(want.to_bits()));
}

#[test]
fn an_update_waits_for_the_running_ramp() {
    let (mut tx, mut chain) = chain();
    push(&mut tx, &preamp_only(-60));
    run(&mut chain, 128, 0.5);
    push(&mut tx, &EffectsSettings::default());
    run(&mut chain, 128, 0.5);
    assert!(chain.is_active(), "the on ramp runs to its end first");
    run(&mut chain, 2 * RAMP, 0.5);
    assert!(!chain.is_active());
}

#[test]
fn new_delay_parameters_settle_inside_100_ms() {
    let (mut tx, mut chain) = chain();
    push(&mut tx, &delay(250, 30, 30));
    run(&mut chain, 1_024, 0.1);
    assert!(chain.is_settled());
    push(&mut tx, &delay(40, 30, 30));
    run(&mut chain, 256, 0.1);
    assert!(!chain.is_settled(), "fades out before the switch");
    run(&mut chain, 768, 0.1);
    assert!(
        chain.is_settled(),
        "switched and back inside 1024 frames (21 ms)"
    );
    let want = DelayParams::new(&delay(40, 30, 30).delay, RATE);
    assert_eq!(chain.delay.effect.params(), want);
}

#[test]
fn samples_past_full_scale_are_clamped_and_counted() {
    let (mut tx, mut chain) = chain();
    push(&mut tx, &delay(1, 85, 50));
    let out = run(&mut chain, 48_000, 0.9);
    assert!(out.iter().all(|s| (-1.0..=1.0).contains(s)));
    assert!(chain.clipped_samples() > 0);
}

#[test]
fn a_non_finite_output_becomes_zero_and_counts() {
    let (_tx, mut chain) = chain();
    let mut block = [0.5, f32::NAN, -f32::INFINITY, 2.0];
    chain.clamp(&mut block);
    assert_eq!(block, [0.5, 0.0, 0.0, 1.0]);
    assert_eq!(chain.clipped_samples(), 3);
}

#[test]
fn a_non_finite_input_sample_is_silenced_before_the_filters() {
    let (mut tx, mut chain) = chain();
    let settings = EffectsSettings {
        equalizer: EqSettings {
            enabled: true,
            preamp_tenth_db: 0,
            filters: ten_band([60, 40, 0, 0, 0, 0, 0, 0, 30, 60]),
        },
        ..EffectsSettings::default()
    };
    push(&mut tx, &settings);
    let mut warm = vec![0.05_f32; 4_096];
    chain.process(&mut warm);
    let mut block = vec![0.05_f32; 512];
    block[10] = f32::NAN;
    block[11] = f32::INFINITY;
    chain.process(&mut block);
    assert!(block[10].abs() < 0.1 && block[11].abs() < 0.1);
    assert!(block.iter().all(|s| s.is_finite()));
    assert_eq!(chain.clipped_samples(), 2);
    let mut after = vec![0.05_f32; 4_096];
    chain.process(&mut after);
    assert!(after.iter().all(|s| s.is_finite() && s.abs() < 1.0));
}

#[test]
fn the_tail_rings_out_then_stops() {
    let (mut tx, mut chain) = chain();
    let mut silent = vec![1.0_f32; 64];
    assert!(
        !chain.process_tail(&mut silent),
        "nothing rings with every effect off"
    );
    assert!(silent.iter().all(|s| *s == 0.0));

    let settings = delay(10, 50, 50);
    push(&mut tx, &settings);
    run(&mut chain, 1_024, 0.5);
    let tail = DelayParams::new(&settings.delay, RATE).tail_frames();
    let mut block = vec![0.0_f32; 512];
    let (mut frames, mut heard) = (0_u64, false);
    while chain.process_tail(&mut block) {
        heard |= block.iter().any(|s| s.abs() > 1e-3);
        frames += 256;
        assert!(frames <= tail + 256, "rings past its tail");
    }
    assert!(heard, "the echoes play after the input stopped");
    assert!(frames >= tail);
    assert!(block.iter().all(|s| *s == 0.0));
}
