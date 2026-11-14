#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "test signals and sample counts are written from f64 values in range"
)]

use std::f64::consts::PI;

use nemo_core::effects::{EQ_MAX_FILTERS, EqFilter, EqSettings, FilterKind};
use proptest::prelude::*;

use super::{Biquad, Coefficients, TEN_BAND_HZ, auto_preamp, ten_band};

const RATES: [u32; 4] = [44_100, 48_000, 96_000, 192_000];
const KINDS: [FilterKind; 3] = [
    FilterKind::Peak,
    FilterKind::LowShelf,
    FilterKind::HighShelf,
];

fn filter(kind: FilterKind, freq_hz: u16, gain_tenth_db: i16, q_hundredths: u16) -> EqFilter {
    EqFilter {
        kind,
        freq_hz,
        gain_tenth_db,
        q_hundredths,
    }
}

fn peak(freq_hz: u16, gain_tenth_db: i16) -> EqFilter {
    filter(FilterKind::Peak, freq_hz, gain_tenth_db, 141)
}

fn eq(preamp_tenth_db: i16, list: &[EqFilter]) -> EqSettings {
    let mut filters = [None; EQ_MAX_FILTERS];
    for (slot, f) in filters.iter_mut().zip(list) {
        *slot = Some(*f);
    }
    EqSettings {
        enabled: true,
        preamp_tenth_db,
        filters,
    }
}

fn is_stable(c: Coefficients) -> bool {
    // Both poles lie inside the unit circle (the stability triangle).
    c.a2.abs() < 1.0 && c.a1.abs() < 1.0 + c.a2
}

/// The gain of a sine at `freq` through `coefficients`, measured from `process` output by
/// projecting a whole number of periods onto sine and cosine after `settle_s` seconds.
fn measured_gain_db(coefficients: Coefficients, freq: f64, rate: u32, settle_s: f64) -> f64 {
    let fs = f64::from(rate);
    let w = 2.0 * PI * freq / fs;
    let settle = (settle_s * fs) as u32;
    let periods = (0.25 * freq).ceil().max(8.0);
    let measure = (periods * fs / freq).round() as u32;
    let amp_in = 0.25;
    let mut filter = Biquad::new(coefficients);
    let (mut in_phase, mut quadrature) = (0.0, 0.0);
    for n in 0..settle + measure {
        let phase = w * f64::from(n);
        let y = f64::from(filter.process((amp_in * phase.sin()) as f32));
        if n >= settle {
            in_phase += y * phase.sin();
            quadrature += y * phase.cos();
        }
    }
    let amp_out = 2.0 * f64::hypot(in_phase, quadrature) / f64::from(measure);
    20.0 * (amp_out / amp_in).log10()
}

#[test]
fn the_gain_at_each_band_centre_matches_the_analytic_value() {
    for rate in RATES {
        for freq in TEN_BAND_HZ {
            for gain in [-120, -35, 35, 120] {
                let c = Coefficients::for_filter(&peak(freq, gain), rate);
                let got = c.gain_db_at(f64::from(freq), rate);
                let want = f64::from(gain) / 10.0;
                assert!((got - want).abs() < 1e-6, "{freq} Hz at {rate}: {got} dB");
            }
        }
    }
}

#[test]
fn process_gives_the_band_gain_at_each_centre_and_rate() {
    for rate in RATES {
        for freq in TEN_BAND_HZ {
            let c = Coefficients::for_filter(&peak(freq, 60), rate);
            let got = measured_gain_db(c, f64::from(freq), rate, 0.25);
            assert!((got - 6.0).abs() < 0.01, "{freq} Hz at {rate}: {got} dB");
        }
    }
}

#[test]
fn shelves_reach_their_gain_at_the_far_end() {
    for rate in RATES {
        let nyquist = f64::from(rate) / 2.0;
        let low = Coefficients::for_filter(&filter(FilterKind::LowShelf, 105, 55, 70), rate);
        assert!((low.gain_db_at(0.0, rate) - 5.5).abs() < 1e-6);
        assert!(low.gain_db_at(nyquist, rate).abs() < 1e-6);
        let high = Coefficients::for_filter(&filter(FilterKind::HighShelf, 8000, -40, 70), rate);
        assert!((high.gain_db_at(nyquist, rate) + 4.0).abs() < 1e-6);
        assert!(high.gain_db_at(0.0, rate).abs() < 1e-6);
    }
}

#[test]
fn the_31_hz_band_is_stable_at_192_khz() {
    for kind in KINDS {
        for gain in [-120, 120] {
            for q in [10, 141, 1000] {
                let c = Coefficients::for_filter(&filter(kind, 31, gain, q), 192_000);
                assert!(is_stable(c), "{kind:?} {gain} {q}");
            }
        }
    }
    // The slowest pole: +12 dB at Q 10, which rings for about Q / (pi f) = 0.1 s.
    let c = Coefficients::for_filter(&filter(FilterKind::Peak, 31, 120, 1000), 192_000);
    let got = measured_gain_db(c, 31.0, 192_000, 1.5);
    assert!((got - 12.0).abs() < 0.01, "31 Hz at 192 kHz: {got} dB");
    let mut f = Biquad::new(c);
    assert!(f.process(1.0).is_finite());
    let mut tail = 0.0_f32;
    for n in 1..192_000 * 15 {
        let y = f.process(0.0).abs();
        assert!(y < 1.0, "sample {n}: {y}");
        if n >= 192_000 * 14 {
            tail = tail.max(y);
        }
    }
    assert_eq!(
        tail, 0.0,
        "the ring-down ends in exact zeros, with no limit cycle"
    );
    assert_eq!(f, Biquad::new(c), "the state is clear again");
}

#[test]
fn a_zero_gain_filter_is_the_identity_and_limits_hold() {
    for kind in KINDS {
        let c = Coefficients::for_filter(&filter(kind, 1000, 0, 141), 48_000);
        assert_eq!(c, Coefficients::IDENTITY);
    }
    let wild = Coefficients::for_filter(&filter(FilterKind::Peak, 1000, 900, 0), 48_000);
    let limited = Coefficients::for_filter(&filter(FilterKind::Peak, 1000, 120, 10), 48_000);
    assert_eq!(wild, limited, "gain and Q are limited to +12 dB and 0.10");
    let above = Coefficients::for_filter(&filter(FilterKind::Peak, 20_000, 120, 141), 32_000);
    assert!(
        is_stable(above),
        "a frequency past Nyquist is moved below it"
    );
}

#[test]
fn ten_band_is_ten_peaks_at_the_iso_centres() {
    let gains = [10, 20, 30, 40, 50, -10, -20, -30, -40, -50];
    let bands = ten_band(gains);
    for (i, band) in bands.iter().enumerate() {
        match (band, TEN_BAND_HZ.get(i), gains.get(i)) {
            (Some(f), Some(&hz), Some(&gain)) => assert_eq!(*f, peak(hz, gain)),
            (None, None, None) => {}
            other => panic!("slot {i}: {other:?}"),
        }
    }
}

#[test]
fn auto_preamp_lowers_by_the_peak_boost() {
    let off = EqSettings {
        enabled: false,
        ..eq(0, &[peak(1000, 60)])
    };
    assert_eq!(auto_preamp(&off, 48_000), 0, "off");
    assert_eq!(auto_preamp(&eq(0, &[]), 48_000), 0, "flat");
    assert_eq!(auto_preamp(&eq(0, &[peak(1000, 60)]), 48_000), 60);
    assert_eq!(auto_preamp(&eq(0, &[peak(31, 40)]), 192_000), 40);
    assert_eq!(auto_preamp(&eq(-60, &[peak(1000, 60)]), 48_000), 0);
    assert_eq!(auto_preamp(&eq(-40, &[peak(1000, 60)]), 48_000), 20);
    assert_eq!(
        auto_preamp(&eq(30, &[]), 48_000),
        30,
        "a positive preamp alone"
    );
    assert_eq!(auto_preamp(&eq(0, &[peak(500, -60)]), 48_000), 0, "a cut");
    let shelf = filter(FilterKind::LowShelf, 105, 60, 70);
    assert_eq!(auto_preamp(&eq(0, &[shelf]), 44_100), 60);
    let all_up = EqSettings {
        filters: ten_band([120; 10]),
        ..eq(0, &[])
    };
    let lowered = auto_preamp(&all_up, 48_000);
    assert!(lowered > 120, "neighbouring bands add up: {lowered}");
}

/// The highest output of a 1 s log sweep from 20 Hz to 0.45 times the rate, at 0 dBFS with a
/// 10 ms fade-in, through `settings` with the automatic preamp applied.
fn sweep_peak(settings: &EqSettings, rate: u32) -> f32 {
    let lowered = i32::from(auto_preamp(settings, rate));
    let preamp_db = f64::from(i32::from(settings.preamp_tenth_db) - lowered) / 10.0;
    let gain = 10_f64.powf(preamp_db / 20.0);
    let mut bank: Vec<Biquad> = settings
        .filters
        .iter()
        .flatten()
        .map(|f| Biquad::new(Coefficients::for_filter(f, rate)))
        .collect();
    let fs = f64::from(rate);
    let (start, growth) = (20.0, (0.45 * fs / 20.0).ln());
    let mut highest = 0.0_f32;
    for n in 0..rate {
        let t = f64::from(n) / fs;
        let phase = 2.0 * PI * start * ((growth * t).exp() - 1.0) / growth;
        let fade = (t / 0.01).min(1.0);
        let x = (gain * fade * phase.sin()) as f32;
        let y = bank.iter_mut().fold(x, |s, b| b.process(s));
        highest = highest.max(y.abs());
    }
    highest
}

/// What a 1 s sweep may add on top of the steady-state response, as the filters ring: 0.5 dB.
/// The automatic preamp works on the steady-state response (design, Overview), so a sweep
/// overshoots by what its transients add: 0.015 dB for one +12 dB band at Q 1.41 (0.004 dB
/// over 4 s), and at most 0.35 dB over 1300 random settings with Q up to 10, measured on
/// 2026-11-14. The clamp counter of the chain reports what passes full scale.
const SWEEP_ALLOWANCE: f32 = 1.059_254;

#[test]
fn a_boosted_band_sweeps_up_to_full_scale() {
    let highest = sweep_peak(&eq(0, &[peak(1000, 120)]), 48_000);
    assert!(highest <= 1.002, "{highest}");
    assert!(
        highest > 0.994,
        "the preamp is lowered only as far as needed: {highest}"
    );
}

fn any_filter(q_max: u16) -> impl Strategy<Value = EqFilter> {
    let kind = proptest::sample::select(KINDS.to_vec());
    (kind, 10_u16..=20_000, -120_i16..=120, 10_u16..=q_max)
        .prop_map(|(kind, freq, gain, q)| filter(kind, freq, gain, q))
}

fn any_settings(q_max: u16) -> impl Strategy<Value = EqSettings> {
    let filters = proptest::array::uniform12(proptest::option::of(any_filter(q_max)));
    (-300_i16..=120, filters).prop_map(|(preamp_tenth_db, filters)| EqSettings {
        enabled: true,
        preamp_tenth_db,
        filters,
    })
}

proptest! {
    #[test]
    fn with_the_auto_preamp_no_steady_tone_exceeds_full_scale(
        settings in any_settings(1000),
        rate in proptest::sample::select(RATES.to_vec()),
        extra in proptest::collection::vec(0.0_f64..1.0, 32),
    ) {
        let lowered = auto_preamp(&settings, rate);
        let bank: Vec<Coefficients> = settings
            .filters
            .iter()
            .flatten()
            .map(|f| Coefficients::for_filter(f, rate))
            .collect();
        let preamp = f64::from(i32::from(settings.preamp_tenth_db) - i32::from(lowered)) / 10.0;
        let response = |freq: f64| preamp + bank.iter().map(|c| c.gain_db_at(freq, rate)).sum::<f64>();
        // An independent grid of 300 points per octave from 1 Hz, plus random frequencies.
        let nyquist = f64::from(rate) / 2.0;
        let grid = (0..).map(|k| 2_f64.powf(f64::from(k) / 300.0)).take_while(|f| *f < nyquist);
        let random = extra.iter().map(|u| u * nyquist);
        let highest = grid.chain(random).chain([0.0, nyquist]).map(response).fold(f64::MIN, f64::max);
        prop_assert!(highest <= 1e-6, "{} dB", highest);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn a_swept_sine_with_the_auto_preamp_stays_within_0_5_db_of_full_scale(
        settings in any_settings(1000),
        rate in proptest::sample::select(RATES.to_vec()),
    ) {
        let highest = sweep_peak(&settings, rate);
        prop_assert!(highest <= SWEEP_ALLOWANCE, "{} at {} Hz: {:?}", highest, rate, settings);
    }
}
