//! Equalizer filters: RBJ biquads and the automatic preamp.
//!
//! Coefficients follow the RBJ "Audio EQ Cookbook" for peak, low-shelf and high-shelf filters.
//! Coefficients and filter state are `f64` and samples `f32`, so the 31 Hz band stays stable
//! at 192 kHz. Coefficients and [`auto_preamp`] are computed on the control thread; only
//! [`Biquad::process`] and the setters it needs run in the audio callback.

use std::f64::consts::PI;

use nemo_core::effects::{EQ_MAX_FILTERS, EqFilter, EqSettings, FilterKind};

/// The ISO octave centres of the ten-band equalizer, in Hz.
pub const TEN_BAND_HZ: [u16; 10] = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16_000];

/// The Q of every ten-band filter, in hundredths: Q 1.41, about one octave wide.
pub const TEN_BAND_Q_HUNDREDTHS: u16 = 141;

/// Lowest Q a coefficient uses, so a zero Q cannot divide by zero.
const Q_MIN: f64 = 0.1;
/// Highest filter frequency as a share of the rate, so a filter stays below Nyquist.
const FREQ_MAX_OF_RATE: f64 = 0.49;
/// Lowest filter frequency in Hz, so a filter never sits at DC.
const FREQ_MIN_HZ: f64 = 1.0;
/// Two output samples in a row below this size (-600 dB, far below any `f32` sample) end the
/// ringing: the output state is cleared, so a decaying tail never reaches the slow subnormal
/// range of `f64`.
const STATE_FLOOR: f64 = 1e-30;
/// Probe points per octave of [`auto_preamp`] before it refines each local peak.
const PROBES_PER_OCTAVE: f64 = 48.0;
/// Lowest probe above 0 Hz, in Hz.
const PROBE_MIN_HZ: f64 = 5.0;
/// Golden-section steps per local peak; 40 steps narrow the interval by about 10^8.
const REFINE_STEPS: usize = 40;
/// Slack, in tenths of a dB, before the peak is rounded up, so a band of exactly +4.0 dB
/// lowers the preamp by 4.0 dB and not 4.1 dB because of rounding noise (1e-7 dB is far
/// below one `f32` step at full scale).
const ROUNDING_SLACK_TENTHS: f64 = 1e-6;

/// The ten-band equalizer: ten peaks at [`TEN_BAND_HZ`] with Q 1.41 and the given gains in
/// tenths of a dB, in the first ten slots. The other slots stay empty.
#[must_use]
pub fn ten_band(gains_tenth_db: [i16; 10]) -> [Option<EqFilter>; EQ_MAX_FILTERS] {
    let mut filters = [None; EQ_MAX_FILTERS];
    let bands = TEN_BAND_HZ.into_iter().zip(gains_tenth_db);
    for (slot, (freq_hz, gain_tenth_db)) in filters.iter_mut().zip(bands) {
        *slot = Some(EqFilter {
            kind: FilterKind::Peak,
            freq_hz,
            gain_tenth_db,
            q_hundredths: TEN_BAND_Q_HUNDREDTHS,
        });
    }
    filters
}

/// Biquad coefficients, normalised so that `a0` is 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coefficients {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl Coefficients {
    /// The filter that passes every sample unchanged.
    pub const IDENTITY: Self = Self {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };

    /// The RBJ coefficients of `filter` at `rate` Hz. A 0 dB filter is
    /// [`IDENTITY`](Self::IDENTITY). Out-of-range values are limited first, so the filter is
    /// always stable: the gain to -12 to +12 dB, the Q to at least 0.1, and the frequency to
    /// 1 Hz to 0.49 times the rate. Runs on the control thread.
    #[must_use]
    pub fn for_filter(filter: &EqFilter, rate: u32) -> Self {
        let gain_tenth_db = filter
            .gain_tenth_db
            .clamp(EqFilter::GAIN_MIN_TENTH_DB, EqFilter::GAIN_MAX_TENTH_DB);
        if gain_tenth_db == 0 {
            return Self::IDENTITY;
        }
        let rate = f64::from(rate.max(1));
        let freq = f64::from(filter.freq_hz)
            .min(rate * FREQ_MAX_OF_RATE)
            .max(FREQ_MIN_HZ);
        let (sin, cos) = (2.0 * PI * freq / rate).sin_cos();
        let q = (f64::from(filter.q_hundredths) / 100.0).max(Q_MIN);
        let alpha = sin / (2.0 * q);
        // A = 10^(dB / 40); the gain arrives in tenths of a dB.
        let amp = 10_f64.powf(f64::from(gain_tenth_db) / 400.0);
        let raw = match filter.kind {
            FilterKind::Peak => [
                1.0 + alpha * amp,
                -2.0 * cos,
                1.0 - alpha * amp,
                1.0 + alpha / amp,
                -2.0 * cos,
                1.0 - alpha / amp,
            ],
            FilterKind::LowShelf => low_shelf(amp, cos, alpha),
            FilterKind::HighShelf => high_shelf(amp, cos, alpha),
        };
        Self::normalised(raw)
    }

    /// `[b0, b1, b2, a0, a1, a2]` divided by `a0`.
    fn normalised([b0, b1, b2, a0, a1, a2]: [f64; 6]) -> Self {
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    /// The gain of the filter at `freq_hz` for a stream at `rate` Hz, in dB.
    ///
    /// Uses the RBJ form in `sin^2(w/2)`, which keeps its precision at low frequencies where
    /// the plain `cos w` form cancels.
    #[must_use]
    pub fn gain_db_at(&self, freq_hz: f64, rate: u32) -> f64 {
        let half = PI * freq_hz / f64::from(rate.max(1));
        let phi = half.sin() * half.sin();
        let Self { b0, b1, b2, a1, a2 } = *self;
        let num = (b0 + b1 + b2).powi(2) - 4.0 * (b0 * b1 + 4.0 * b0 * b2 + b1 * b2) * phi
            + 16.0 * b0 * b2 * phi * phi;
        let den =
            (1.0 + a1 + a2).powi(2) - 4.0 * (a1 + 4.0 * a2 + a1 * a2) * phi + 16.0 * a2 * phi * phi;
        10.0 * (num.max(f64::MIN_POSITIVE) / den.max(f64::MIN_POSITIVE)).log10()
    }
}

/// RBJ low shelf, `[b0, b1, b2, a0, a1, a2]`.
fn low_shelf(amp: f64, cos: f64, alpha: f64) -> [f64; 6] {
    let k = 2.0 * amp.sqrt() * alpha;
    [
        amp * ((amp + 1.0) - (amp - 1.0) * cos + k),
        2.0 * amp * ((amp - 1.0) - (amp + 1.0) * cos),
        amp * ((amp + 1.0) - (amp - 1.0) * cos - k),
        (amp + 1.0) + (amp - 1.0) * cos + k,
        -2.0 * ((amp - 1.0) + (amp + 1.0) * cos),
        (amp + 1.0) + (amp - 1.0) * cos - k,
    ]
}

/// RBJ high shelf, `[b0, b1, b2, a0, a1, a2]`.
fn high_shelf(amp: f64, cos: f64, alpha: f64) -> [f64; 6] {
    let k = 2.0 * amp.sqrt() * alpha;
    [
        amp * ((amp + 1.0) + (amp - 1.0) * cos + k),
        -2.0 * amp * ((amp - 1.0) + (amp + 1.0) * cos),
        amp * ((amp + 1.0) + (amp - 1.0) * cos - k),
        (amp + 1.0) - (amp - 1.0) * cos + k,
        2.0 * ((amp - 1.0) - (amp + 1.0) * cos),
        (amp + 1.0) - (amp - 1.0) * cos - k,
    ]
}

/// One biquad filter for one channel: Direct Form I with `f64` state, which keeps a
/// coefficient change free of state jumps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Biquad {
    coefficients: Coefficients,
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl Default for Biquad {
    /// An identity filter with clear state.
    fn default() -> Self {
        Self::new(Coefficients::IDENTITY)
    }
}

impl Biquad {
    /// A filter with `coefficients` and clear state.
    #[must_use]
    pub const fn new(coefficients: Coefficients) -> Self {
        Self {
            coefficients,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    /// The coefficients in use.
    #[must_use]
    pub const fn coefficients(&self) -> Coefficients {
        self.coefficients
    }

    /// Use `coefficients` from the next sample on, keeping the state.
    ///
    /// RT-SAFE: a copy of five numbers. No allocation, lock or logging.
    pub const fn set_coefficients(&mut self, coefficients: Coefficients) {
        self.coefficients = coefficients;
    }

    /// Clear the state, as after a seek or a non-finite sample.
    ///
    /// RT-SAFE: four stores. No allocation, lock or logging.
    pub const fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }

    /// Filter one sample.
    ///
    /// RT-SAFE: five multiplies and four adds in `f64`. No allocation, lock or logging.
    #[inline]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the result is a sample; f32 is the output format"
    )]
    pub fn process(&mut self, sample: f32) -> f32 {
        let x = f64::from(sample);
        let c = self.coefficients;
        let y = c.b0 * x + c.b1 * self.x1 + c.b2 * self.x2 - c.a1 * self.y1 - c.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        if y.abs() < STATE_FLOOR && self.y1.abs() < STATE_FLOOR {
            // Two outputs in a row below the floor: the ringing is gone. Clearing only one of
            // them at a zero crossing would feed the resonance and keep it alive.
            self.y2 = 0.0;
            self.y1 = 0.0;
        } else {
            self.y2 = self.y1;
            self.y1 = y;
        }
        y as f32
    }
}

/// How far the preamp must be lowered, in tenths of a dB, so the combined response of the
/// preamp and every filter peaks at 0 dB or below: its peak boost, rounded up. 0 when the
/// equalizer is off or nothing rises above 0 dB.
///
/// Runs on the control thread, never in the callback: it probes the response from 0 Hz to
/// the Nyquist frequency on a log grid of 48 points per octave and refines each local peak.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is clamped to 0..=u16::MAX before the cast"
)]
pub fn auto_preamp(settings: &EqSettings, rate: u32) -> u16 {
    if !settings.enabled {
        return 0;
    }
    let bank: Vec<Coefficients> = settings
        .filters
        .iter()
        .flatten()
        .map(|f| Coefficients::for_filter(f, rate))
        .collect();
    let preamp_db = f64::from(settings.preamp_tenth_db) / 10.0;
    let response =
        |freq: f64| preamp_db + bank.iter().map(|c| c.gain_db_at(freq, rate)).sum::<f64>();
    let peak = peak_db(&response, f64::from(rate.max(1)) / 2.0);
    let tenths = (peak * 10.0 - ROUNDING_SLACK_TENTHS).ceil();
    tenths.clamp(0.0, f64::from(u16::MAX)) as u16
}

/// The highest value of `response` between 0 Hz and `nyquist`.
fn peak_db(response: &dyn Fn(f64) -> f64, nyquist: f64) -> f64 {
    let grid = probe_grid(nyquist);
    let values: Vec<f64> = grid.iter().map(|&f| response(f)).collect();
    let mut peak = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    for (freqs, near) in grid.windows(3).zip(values.windows(3)) {
        if let ([lo, _, hi], [left, mid, right]) = (freqs, near)
            && mid >= left
            && mid >= right
        {
            peak = peak.max(refine(response, *lo, *hi));
        }
    }
    peak
}

/// 0 Hz, a log grid from [`PROBE_MIN_HZ`], and `nyquist`.
fn probe_grid(nyquist: f64) -> Vec<f64> {
    let step = 2_f64.powf(1.0 / PROBES_PER_OCTAVE);
    let mut grid = vec![0.0];
    let mut freq = PROBE_MIN_HZ;
    while freq < nyquist {
        grid.push(freq);
        freq *= step;
    }
    grid.push(nyquist);
    grid
}

/// Golden-section search for the highest value of `response` between `lo` and `hi`.
fn refine(response: &dyn Fn(f64) -> f64, mut lo: f64, mut hi: f64) -> f64 {
    const INV_PHI: f64 = 0.618_033_988_749_894_9;
    let mut best = f64::NEG_INFINITY;
    for _ in 0..REFINE_STEPS {
        let left = hi - INV_PHI * (hi - lo);
        let right = lo + INV_PHI * (hi - lo);
        let (at_left, at_right) = (response(left), response(right));
        best = best.max(at_left).max(at_right);
        if at_left > at_right {
            hi = right;
        } else {
            lo = left;
        }
    }
    best
}

#[cfg(test)]
mod tests;
