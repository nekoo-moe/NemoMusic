//! The equalizer stage of the effects chain: a preamp and a bank of biquads per channel, with
//! an on/off ramp and a crossfade from the old filters when the coefficients change.

use nemo_core::effects::EQ_MAX_FILTERS;

use super::biquad::{Biquad, Coefficients};
use super::chain::EffectsUpdate;
use super::fade::Fade;

/// Most channels the equalizer filters. Channels past the eighth pass through unchanged.
const EQ_MAX_CHANNELS: usize = 8;

/// The filters of one channel.
type Bank = [Biquad; EQ_MAX_FILTERS];

/// The equalizer for one stream.
#[derive(Debug)]
pub(super) struct Equalizer {
    /// On/off ramp.
    fade: Fade,
    /// Crossfade from `old` (0) to `bank` (1) after a coefficient change.
    morph: Fade,
    morphing: bool,
    /// Channels that are filtered: the stream's up to [`EQ_MAX_CHANNELS`].
    lanes: usize,
    bank: Vec<Bank>,
    old: Vec<Bank>,
    coefficients: [Coefficients; EQ_MAX_FILTERS],
    count: usize,
    old_count: usize,
    preamp: f32,
    old_preamp: f32,
}

impl Equalizer {
    /// A silent, flat equalizer. Allocates the banks: call it when the pipeline starts.
    pub(super) fn new(channels: usize, rate: u32) -> Self {
        let lanes = channels.clamp(1, EQ_MAX_CHANNELS);
        let bank = [Biquad::default(); EQ_MAX_FILTERS];
        Self {
            fade: Fade::new(rate),
            morph: Fade::new(rate),
            morphing: false,
            lanes,
            bank: vec![bank; lanes],
            old: vec![bank; lanes],
            coefficients: [Coefficients::IDENTITY; EQ_MAX_FILTERS],
            count: 0,
            old_count: 0,
            preamp: 1.0,
            old_preamp: 1.0,
        }
    }

    /// Take the equalizer part of `update`. A silent equalizer loads the filters with clear
    /// state; a sounding one crossfades to them from its current filters.
    ///
    /// RT-SAFE: copies of preallocated banks. No allocation, lock or logging.
    pub(super) fn apply(&mut self, update: &EffectsUpdate) {
        let changed = update.preamp.to_bits() != self.preamp.to_bits()
            || update.filter_count != self.count
            || update.filters != self.coefficients;
        if self.fade.is_off() {
            self.load(update);
        } else if update.eq_on && changed {
            for (old, now) in self.old.iter_mut().zip(&self.bank) {
                *old = *now;
            }
            self.old_count = self.count;
            self.old_preamp = self.preamp;
            for bank in &mut self.bank {
                for (filter, c) in bank.iter_mut().zip(update.filters) {
                    filter.set_coefficients(c);
                }
            }
            self.set_targets(update);
            self.morph.jump(false);
            self.morph.set(true);
            self.morphing = true;
        }
        self.fade.set(update.eq_on);
    }

    /// Load the filters of `update` with clear state.
    ///
    /// RT-SAFE: stores into the preallocated banks. No allocation, lock or logging.
    fn load(&mut self, update: &EffectsUpdate) {
        for bank in &mut self.bank {
            for (filter, c) in bank.iter_mut().zip(update.filters) {
                *filter = Biquad::new(c);
            }
        }
        self.set_targets(update);
        self.morphing = false;
    }

    /// Remember the coefficients, filter count and preamp of `update`.
    ///
    /// RT-SAFE: three copies. No allocation, lock or logging.
    const fn set_targets(&mut self, update: &EffectsUpdate) {
        self.coefficients = update.filters;
        self.count = update.filter_count;
        self.preamp = update.preamp;
    }

    /// Filter the interleaved `chunk` in place, ramped against the dry signal while the
    /// equalizer turns on or off.
    ///
    /// RT-SAFE: per-sample arithmetic on the preallocated banks. No allocation, lock or
    /// logging.
    pub(super) fn process(&mut self, chunk: &mut [f32], channels: usize) {
        if self.fade.is_off() {
            self.morphing = false;
            return;
        }
        let full = self.fade.is_full();
        for frame in chunk.chunks_exact_mut(channels) {
            let gain = if full { 1.0 } else { self.fade.next_gain() };
            let mix = if self.morphing {
                self.morph.next_gain()
            } else {
                1.0
            };
            let lanes = frame.iter_mut().take(self.lanes);
            for ((sample, bank), old) in lanes.zip(&mut self.bank).zip(&mut self.old) {
                let x = *sample;
                let mut y = run(bank, self.count, self.preamp, x);
                if self.morphing {
                    // The old filters keep running to the end of the crossfade, so their state
                    // stays current.
                    let before = run(old, self.old_count, self.old_preamp, x);
                    if mix < 1.0 {
                        y = before + mix * (y - before);
                    }
                }
                *sample = if gain >= 1.0 { y } else { x + gain * (y - x) };
            }
            if self.morphing && self.morph.is_full() {
                self.morphing = false;
            }
        }
    }

    /// Clear the filter state, as after a seek or a non-finite sample.
    ///
    /// RT-SAFE: stores into the preallocated banks. No allocation, lock or logging.
    pub(super) fn reset(&mut self) {
        for filter in self.bank.iter_mut().chain(&mut self.old).flatten() {
            filter.reset();
        }
    }

    /// The equalizer is silent.
    ///
    /// RT-SAFE: reads only. No allocation, lock or logging.
    pub(super) const fn is_idle(&self) -> bool {
        self.fade.is_off()
    }

    /// No ramp or crossfade runs.
    ///
    /// RT-SAFE: reads only. No allocation, lock or logging.
    pub(super) const fn is_settled(&self) -> bool {
        self.fade.is_settled() && !self.morphing
    }
}

/// `x` through the preamp and the first `count` filters of `bank`.
///
/// RT-SAFE: per-sample arithmetic. No allocation, lock or logging.
#[inline]
fn run(bank: &mut Bank, count: usize, preamp: f32, x: f32) -> f32 {
    bank.iter_mut()
        .take(count)
        .fold(x * preamp, |s, filter| filter.process(s))
}
