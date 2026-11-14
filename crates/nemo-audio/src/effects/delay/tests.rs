use nemo_core::effects::DelaySettings;

use super::{DELAY_MAX_CHANNELS, Delay, DelayParams};

fn settings(time_ms: u16, feedback_pct: u8, mix_pct: u8) -> DelaySettings {
    DelaySettings {
        enabled: true,
        time_ms,
        feedback_pct,
        mix_pct,
    }
}

/// Mono output of an impulse followed by `frames - 1` zeros.
fn impulse_response(params: DelayParams, rate: u32, frames: usize) -> Vec<f32> {
    let mut delay = Delay::new(1, rate, params);
    let mut out = vec![0.0_f32; frames];
    if let Some(first) = out.first_mut() {
        *first = 1.0;
    }
    for block in out.chunks_mut(512) {
        delay.process(block);
    }
    out
}

#[test]
fn an_impulse_at_85_percent_feedback_decays_below_60_db_inside_the_tail() {
    let rate = 48_000;
    let params = DelayParams::new(&settings(200, 85, 100), rate);
    assert_eq!(params.frames(), 9_600);
    // 43 echoes are at or above -60 dB (0.85^42 = 0.0011); the 44th is below (0.85^43).
    assert_eq!(params.tail_frames(), 43 * 9_600);
    let tail = usize::try_from(params.tail_frames()).expect("tail fits usize");
    let out = impulse_response(params, rate, tail + 48_000);
    let (inside, after) = out.split_at(tail + 1);
    assert!(
        after.iter().all(|s| s.abs() < 1e-3),
        "an echo past the tail"
    );
    let last = inside.last().copied().unwrap_or(0.0);
    assert!(
        last >= 1e-3,
        "the tail ends right after the last loud echo: {last}"
    );
    for (k, &echo) in out.iter().step_by(9_600).enumerate().skip(1).take(43) {
        let want = 0.85_f32.powi(i32::try_from(k).expect("small") - 1);
        assert!((echo - want).abs() < 1e-5, "echo {k}: {echo}");
    }
}

#[test]
fn settings_are_clamped_and_the_tail_is_capped_at_ten_seconds() {
    let rate = 48_000;
    let longest = DelayParams::new(&settings(2000, 85, 30), rate);
    assert_eq!(longest.frames(), 96_000);
    assert_eq!(longest.tail_frames(), 10 * 48_000);
    assert_eq!(
        DelayParams::new(&settings(9999, 99, 200), rate),
        DelayParams::new(&settings(2000, 85, 100), rate)
    );
    assert_eq!(DelayParams::new(&settings(0, 30, 30), rate).frames(), 48);
    let single = DelayParams::new(&settings(100, 0, 30), rate);
    assert_eq!(single.tail_frames(), 4_800, "no feedback: one echo");
    let exact = DelayParams::new(&settings(100, 10, 30), rate);
    assert_eq!(
        exact.tail_frames(),
        4 * 4_800,
        "echo 4 is exactly -60 dB and stays inside"
    );
}

#[test]
fn echoes_land_on_their_own_channel_with_the_mix() {
    let rate = 48_000;
    let params = DelayParams::new(&settings(10, 0, 30), rate);
    let mut delay = Delay::new(2, rate, params);
    let mut block = vec![0.0_f32; 2 * 1024];
    block[0] = 1.0;
    delay.process(&mut block);
    assert!((block[0] - 0.7).abs() < 1e-6, "dry share");
    assert!((block[2 * 480] - 0.3).abs() < 1e-6, "wet share after 10 ms");
    let others = block
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 0 && *i != 2 * 480);
    assert!(
        others.into_iter().all(|(_, s)| *s == 0.0),
        "nothing else, nothing on the right"
    );
}

#[test]
fn reset_forgets_every_echo() {
    let params = DelayParams::new(&settings(5, 85, 50), 48_000);
    let mut delay = Delay::new(1, 48_000, params);
    let mut block = vec![0.5_f32; 4_800];
    delay.process(&mut block);
    delay.reset();
    let mut silence = vec![0.0_f32; 48_000];
    delay.process(&mut silence);
    assert!(silence.iter().all(|s| *s == 0.0));
}

#[test]
fn channels_past_the_eighth_pass_through() {
    let channels = DELAY_MAX_CHANNELS + 2;
    let params = DelayParams::new(&settings(1, 50, 100), 48_000);
    let mut delay = Delay::new(channels, 48_000, params);
    let mut block: Vec<f32> = (0..channels * 64)
        .map(|i| f32::from(u16::try_from(i % 7).unwrap_or(0)) * 0.1)
        .collect();
    let input = block.clone();
    delay.process(&mut block);
    for (frame, original) in block
        .chunks_exact(channels)
        .zip(input.chunks_exact(channels))
    {
        assert_eq!(frame[8..], original[8..]);
    }
    assert_ne!(block[..8], input[..8], "the first eight are delayed");
}

#[test]
fn a_mix_of_zero_keeps_every_sample() {
    let params = DelayParams::new(&settings(250, 85, 0), 44_100);
    let mut delay = Delay::new(2, 44_100, params);
    let input: Vec<f32> = (0..44_100_u16)
        .map(|i| (f32::from(i) * 0.01).sin() * 0.9)
        .collect();
    let mut block = input.clone();
    delay.process(&mut block);
    assert!(
        block
            .iter()
            .zip(&input)
            .all(|(a, b)| a.to_bits() == b.to_bits())
    );
}
