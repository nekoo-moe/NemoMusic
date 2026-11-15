//! The effects chain the audio callback runs: equalizer, delay, reverb, then a clamp that
//! counts clipping.
//!
//! The control thread turns [`EffectsSettings`] into a `Copy` [`EffectsUpdate`] (coefficients,
//! automatic preamp, stage parameters) and pushes it into a ring of
//! [`EFFECTS_RING_CAPACITY`]. [`EffectsChain::new`] sizes every buffer when the pipeline starts;
//! after that the chain allocates nothing, takes no lock and logs nothing.
//!
//! - One `bool` guards the whole chain: with every effect off it returns before touching a
//!   sample, so the output stays bit-exact.
//! - Turning a stage on or off ramps it over 10 ms ([`RAMP_MS`](super::fade::RAMP_MS)). New
//!   equalizer coefficients or preamp crossfade from the old filters over 10 ms. New delay or
//!   reverb parameters fade that stage out, switch, and fade it back in, 10 ms each way.
//! - An update is taken only once every ramp has finished; the newest waiting update wins, as
//!   each one carries the whole state.
//! - Samples past full scale are clamped and counted; nothing wraps. A NaN or infinite input
//!   sample becomes 0 before any filter sees it and counts as clipped. A non-finite sample a
//!   stage produces becomes 0, counts, and clears the state of every stage.

use nemo_core::effects::{DelaySettings, EQ_MAX_FILTERS, EffectsSettings, ReverbSettings};
use rtrb::{Consumer, Producer, RingBuffer};

use super::biquad::{Coefficients, auto_preamp};
use super::delay::{Delay, DelayParams};
use super::equalizer::Equalizer;
use super::reverb::{Reverb, ReverbParams};
use super::stage::Stage;

/// Updates the ring between the control thread and the chain holds.
pub const EFFECTS_RING_CAPACITY: usize = 8;

/// Frames per processing step; the dry copy for a ramp holds this many frames.
const SCRATCH_FRAMES: usize = 256;
/// Lowest preamp the chain applies, in tenths of a dB (-30 dB).
const PREAMP_MIN_TENTH_DB: i32 = -300;
/// Highest preamp the chain applies, in tenths of a dB (+12 dB).
const PREAMP_MAX_TENTH_DB: i32 = 120;

/// The ring that carries [`EffectsUpdate`]s from the control thread to the chain. Allocates:
/// call it when the pipeline starts.
#[must_use]
pub fn effects_ring() -> (Producer<EffectsUpdate>, Consumer<EffectsUpdate>) {
    RingBuffer::new(EFFECTS_RING_CAPACITY)
}

/// The whole effects state for one rate, ready for the audio callback. `Copy`, so it crosses
/// threads through the ring with no allocation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectsUpdate {
    pub(super) eq_on: bool,
    /// Linear preamp gain, after the automatic lowering.
    pub(super) preamp: f32,
    /// The filters that change the sound, in order; the first `filter_count` are used.
    pub(super) filters: [Coefficients; EQ_MAX_FILTERS],
    pub(super) filter_count: usize,
    lowered_by_tenth_db: u16,
    delay_on: bool,
    delay: DelayParams,
    reverb_on: bool,
    reverb: ReverbParams,
}

impl EffectsUpdate {
    /// The update for `settings` at `rate` Hz: biquad coefficients of every filter that is
    /// not 0 dB, the preamp lowered by [`auto_preamp`] and limited to -30 to +12 dB, and the
    /// clamped delay and reverb parameters. Runs on the control thread (it allocates while it
    /// probes the response).
    #[must_use]
    pub fn new(settings: &EffectsSettings, rate: u32) -> Self {
        let eq = &settings.equalizer;
        let lowered = auto_preamp(eq, rate);
        let preamp_tenth_db = (i32::from(eq.preamp_tenth_db) - i32::from(lowered))
            .clamp(PREAMP_MIN_TENTH_DB, PREAMP_MAX_TENTH_DB);
        let mut filters = [Coefficients::IDENTITY; EQ_MAX_FILTERS];
        let used = eq
            .filters
            .iter()
            .flatten()
            .map(|f| Coefficients::for_filter(f, rate))
            .filter(|c| *c != Coefficients::IDENTITY);
        let mut filter_count = 0;
        for (slot, c) in filters.iter_mut().zip(used) {
            *slot = c;
            filter_count += 1;
        }
        Self {
            eq_on: eq.enabled,
            preamp: tenth_db_to_gain(preamp_tenth_db),
            filters,
            filter_count,
            lowered_by_tenth_db: lowered,
            delay_on: settings.delay.enabled,
            delay: DelayParams::new(&settings.delay, rate),
            reverb_on: settings.reverb.enabled,
            reverb: ReverbParams::new(&settings.reverb, rate),
        }
    }

    /// How far the automatic preamp lowered the user's preamp, in tenths of a dB.
    #[must_use]
    pub const fn preamp_lowered_by_tenth_db(&self) -> u16 {
        self.lowered_by_tenth_db
    }
}

/// `10^(tenths / 200)`: a level in tenths of a dB as a linear gain.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the gain lies between -30 and +12 dB, well inside f32"
)]
fn tenth_db_to_gain(tenth_db: i32) -> f32 {
    10_f64.powf(f64::from(tenth_db) / 200.0) as f32
}

/// The equalizer, delay and reverb for one stream, with the clip counter.
#[derive(Debug)]
pub struct EffectsChain {
    channels: usize,
    updates: Consumer<EffectsUpdate>,
    /// Some stage sounds or ramps. `false` means the chain touches no sample.
    active: bool,
    eq: Equalizer,
    delay: Stage<Delay>,
    reverb: Stage<Reverb>,
    /// The dry copy of one step while a stage ramps.
    dry: Vec<f32>,
    clipped: u64,
    /// Frames left to ring out after the input stopped.
    tail_left: u64,
}

impl EffectsChain {
    /// A chain for an interleaved stream of `channels` channels at `rate` Hz, with every
    /// effect off, fed by `updates` (see [`effects_ring`]). Allocates the delay line, the
    /// reverb tables and the filter banks: call it when the pipeline starts, never in the
    /// callback. Channels past the eighth pass through unchanged.
    #[must_use]
    pub fn new(channels: usize, rate: u32, updates: Consumer<EffectsUpdate>) -> Self {
        let channels = channels.max(1);
        let delay = DelayParams::new(&DelaySettings::default(), rate);
        let reverb = ReverbParams::new(&ReverbSettings::default(), rate);
        Self {
            channels,
            updates,
            active: false,
            eq: Equalizer::new(channels, rate),
            delay: Stage::new(Delay::new(channels, rate, delay), rate),
            reverb: Stage::new(Reverb::new(channels, rate, reverb), rate),
            dry: vec![0.0; SCRATCH_FRAMES * channels],
            clipped: 0,
            tail_left: 0,
        }
    }

    /// Run the chain on the interleaved `block` in place. Takes the newest waiting update
    /// first, once every ramp has finished. With every effect off the samples are not touched.
    ///
    /// RT-SAFE: at most [`EFFECTS_RING_CAPACITY`] lock-free pops, then arithmetic on
    /// preallocated buffers. No allocation, lock, blocking call or logging.
    pub fn process(&mut self, block: &mut [f32]) {
        self.take_update();
        if !self.active {
            return;
        }
        self.run(block);
        self.tail_left = self.delay.tail_frames().max(self.reverb.tail_frames());
    }

    /// Fill `block` with the ring-out of the delay and the reverb after the input stopped:
    /// the chain run on silence. Returns `false`, with `block` silent, once the tail of the
    /// last [`process`](Self::process) call has played or when no stage rings.
    ///
    /// RT-SAFE: as [`process`](Self::process). No allocation, lock, blocking call or logging.
    pub fn process_tail(&mut self, block: &mut [f32]) -> bool {
        block.fill(0.0);
        self.take_update();
        if !self.active || self.tail_left == 0 {
            return false;
        }
        self.run(block);
        let frames = u64::try_from(block.len() / self.channels).unwrap_or(u64::MAX);
        self.tail_left = self.tail_left.saturating_sub(frames);
        true
    }

    /// Some effect sounds or ramps, so [`process`](Self::process) changes samples.
    ///
    /// RT-SAFE: reads only. No allocation, lock or logging.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }

    /// Samples clamped at full scale, or replaced because they were not finite, since the chain
    /// was made.
    ///
    /// RT-SAFE: reads only. No allocation, lock or logging.
    #[must_use]
    pub const fn clipped_samples(&self) -> u64 {
        self.clipped
    }

    /// Clear the state of every stage, as after a seek. The settings stay.
    ///
    /// RT-SAFE: stores into preallocated state. No allocation, lock or logging.
    pub fn reset(&mut self) {
        self.reset_state();
        self.tail_left = 0;
    }

    /// Every ramp has finished, so a new update can be taken.
    ///
    /// RT-SAFE: reads only. No allocation, lock or logging.
    const fn is_settled(&self) -> bool {
        self.eq.is_settled() && self.delay.is_settled() && self.reverb.is_settled()
    }

    /// Apply the newest waiting update once every ramp has finished.
    ///
    /// RT-SAFE: bounded lock-free pops and copies. No allocation, lock or logging.
    fn take_update(&mut self) {
        if !self.is_settled() {
            return;
        }
        let mut newest = None;
        while let Ok(update) = self.updates.pop() {
            newest = Some(update);
        }
        if let Some(u) = newest {
            self.eq.apply(&u);
            self.delay.apply(u.delay_on, u.delay);
            self.reverb.apply(u.reverb_on, u.reverb);
            self.active = !self.is_idle();
        }
    }

    /// Every stage is silent with nothing waiting.
    ///
    /// RT-SAFE: reads only. No allocation, lock or logging.
    const fn is_idle(&self) -> bool {
        self.eq.is_idle() && self.delay.is_idle() && self.reverb.is_idle()
    }

    /// Equalizer, delay, reverb and clamp, in steps of [`SCRATCH_FRAMES`].
    ///
    /// RT-SAFE: arithmetic on preallocated buffers. No allocation, lock or logging.
    fn run(&mut self, block: &mut [f32]) {
        let step = SCRATCH_FRAMES * self.channels;
        for chunk in block.chunks_mut(step) {
            self.sanitize(chunk);
            self.eq.process(chunk, self.channels);
            self.delay.process(chunk, &mut self.dry, self.channels);
            self.reverb.process(chunk, &mut self.dry, self.channels);
            self.clamp(chunk);
        }
        self.active = !self.is_idle();
    }

    /// Replace a non-finite input sample with 0 and count it, so it never reaches a filter.
    ///
    /// RT-SAFE: a compare per sample. No allocation, lock or logging.
    fn sanitize(&mut self, chunk: &mut [f32]) {
        for s in chunk.iter_mut().filter(|s| !s.is_finite()) {
            *s = 0.0;
            self.clipped = self.clipped.saturating_add(1);
        }
    }

    /// Clamp to full scale and count; a non-finite sample becomes 0 and clears every state.
    ///
    /// RT-SAFE: a compare per sample. No allocation, lock or logging.
    fn clamp(&mut self, chunk: &mut [f32]) {
        let mut broken = false;
        for s in chunk.iter_mut() {
            if !s.is_finite() {
                *s = 0.0;
                broken = true;
                self.clipped = self.clipped.saturating_add(1);
            } else if s.abs() > 1.0 {
                *s = s.clamp(-1.0, 1.0);
                self.clipped = self.clipped.saturating_add(1);
            }
        }
        if broken {
            self.reset_state();
        }
    }

    /// Clear the state of every stage.
    ///
    /// RT-SAFE: stores into preallocated state. No allocation, lock or logging.
    fn reset_state(&mut self) {
        self.eq.reset();
        self.delay.effect.reset();
        self.reverb.effect.reset();
    }
}

#[cfg(test)]
mod tests;
