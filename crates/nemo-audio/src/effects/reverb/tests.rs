use nemo_core::effects::ReverbSettings;
use proptest::prelude::*;

use super::{ALLPASS_FEEDBACK, INPUT_GAIN, REVERB_MAX_CHANNELS, Reverb, ReverbParams};

fn settings(size_pct: u8, damping_pct: u8, mix_pct: u8) -> ReverbSettings {
    ReverbSettings {
        enabled: true,
        size_pct,
        damping_pct,
        mix_pct,
    }
}

fn frames(n: u64) -> usize {
    usize::try_from(n).expect("frame count fits usize")
}

#[test]
fn the_tail_ends_inside_tail_frames_at_the_largest_size() {
    for rate in [44_100, 192_000] {
        let params = ReverbParams::new(&settings(100, 0, 100), rate);
        let tail = frames(params.tail_frames());
        assert!(tail <= frames(10 * u64::from(rate)), "capped at 10 s");
        let mut reverb = Reverb::new(1, rate, params);
        let mut out = vec![0.0_f32; tail + frames(u64::from(rate) / 2)];
        if let Some(first) = out.first_mut() {
            *first = 1.0;
        }
        for block in out.chunks_mut(512) {
            reverb.process(block);
        }
        let (inside, after) = out.split_at(tail);
        let late = inside.get(frames(u64::from(rate))..).unwrap_or(&[]);
        assert!(
            late.iter().any(|s| s.abs() > 1e-4),
            "{rate}: it still rings after 1 s"
        );
        let highest = after.iter().fold(0.0_f32, |m, s| m.max(s.abs()));
        assert!(highest < 1e-3, "{rate}: {highest} after the tail");
    }
}

#[test]
fn tail_frames_follow_the_size_and_settings_are_clamped() {
    let rate = 48_000;
    let tails: Vec<u64> = [0, 25, 50, 75, 100]
        .iter()
        .map(|&size| ReverbParams::new(&settings(size, 50, 30), rate).tail_frames())
        .collect();
    assert!(tails.windows(2).all(|w| w[0] < w[1]), "{tails:?}");
    let damped = ReverbParams::new(&settings(100, 100, 30), rate).tail_frames();
    assert_eq!(damped, tails[4], "damping does not change the bound");
    assert_eq!(
        ReverbParams::new(&settings(255, 255, 255), rate),
        ReverbParams::new(&settings(100, 100, 100), rate)
    );
}

#[test]
fn reset_forgets_the_tail_and_channels_stay_apart() {
    let params = ReverbParams::new(&settings(80, 20, 100), 48_000);
    let mut reverb = Reverb::new(2, 48_000, params);
    let mut block = vec![0.0_f32; 2 * 48_000];
    for frame in block.chunks_exact_mut(2).take(4_800) {
        frame[0] = 0.8;
    }
    reverb.process(&mut block);
    let right_silent = block.chunks_exact(2).all(|frame| frame[1] == 0.0);
    assert!(right_silent, "the left input stays on the left");
    reverb.reset();
    let mut silence = vec![0.0_f32; 2 * 48_000];
    reverb.process(&mut silence);
    assert!(silence.iter().all(|s| *s == 0.0));
}

#[test]
fn channels_past_the_eighth_pass_through() {
    let channels = REVERB_MAX_CHANNELS + 2;
    let params = ReverbParams::new(&settings(50, 50, 50), 48_000);
    let mut reverb = Reverb::new(channels, 48_000, params);
    let input: Vec<f32> = (0..u16::try_from(channels * 4_096).unwrap_or(0))
        .map(|i| f32::from(i % 11) * 0.05)
        .collect();
    let mut block = input.clone();
    reverb.process(&mut block);
    for (frame, original) in block
        .chunks_exact(channels)
        .zip(input.chunks_exact(channels))
    {
        assert_eq!(frame[8..], original[8..]);
    }
    assert_ne!(block, input, "the first eight get reverb");
}

#[test]
fn a_mix_of_zero_keeps_every_sample() {
    let params = ReverbParams::new(&settings(100, 0, 0), 44_100);
    let mut reverb = Reverb::new(2, 44_100, params);
    let input: Vec<f32> = (0..44_100_u16)
        .map(|i| (f32::from(i) * 0.013).sin() * 0.9)
        .collect();
    let mut block = input.clone();
    reverb.process(&mut block);
    assert!(
        block
            .iter()
            .zip(&input)
            .all(|(a, b)| a.to_bits() == b.to_bits())
    );
}

/// A bound that holds for any input within full scale: each comb gives at most
/// `INPUT_GAIN / (1 - feedback)`, each all-pass at most 1 + 1 / (1 - 0.5) = 3 times its input
/// (the sum of its impulse response), and the dry share passes once.
fn proven_bound(params: &ReverbParams) -> f32 {
    let combs = 8.0 * INPUT_GAIN / (1.0 - params.feedback);
    let allpasses = (1.0 + 1.0 / (1.0 - ALLPASS_FEEDBACK)).powi(4);
    params.dry + params.wet * combs * allpasses
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn output_stays_bounded_for_any_settings(
        size in any::<u8>(),
        damping in any::<u8>(),
        mix in any::<u8>(),
        rate in proptest::sample::select(vec![44_100_u32, 48_000, 96_000]),
        seed in any::<u32>(),
    ) {
        let params = ReverbParams::new(&settings(size, damping, mix), rate);
        let mut reverb = Reverb::new(2, rate, params);
        // Full-scale noise for 0.25 s, then silence for 0.25 s.
        let quarter = frames(u64::from(rate) / 4) * 2;
        let mut state = seed | 1;
        let mut block: Vec<f32> = (0..quarter)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                f32::from(u16::try_from(state >> 16).unwrap_or(0)) / 32_768.0 - 1.0
            })
            .chain(std::iter::repeat_n(0.0, quarter))
            .collect();
        reverb.process(&mut block);
        let highest = block.iter().fold(0.0_f32, |m, s| m.max(s.abs()));
        prop_assert!(block.iter().all(|s| s.is_finite()));
        prop_assert!(highest <= proven_bound(&params), "{} over the bound", highest);
        prop_assert!(highest < 4.0, "{} is far above full scale", highest);
    }
}
