//! The reverb stage: a Schroeder-Moorer reverb in the Freeverb layout (the public-domain
//! algorithm by Jezar at Dreampoint).
//!
//! Per channel, eight lowpass-feedback comb filters in parallel feed four all-pass filters in
//! series. The delay tables are Freeverb's 44.1 kHz lengths scaled to the device rate, and
//! [`Reverb::new`] allocates them when the pipeline starts; nothing allocates in the callback.
//! Size sets the comb feedback (0.70 to 0.97) and damping the lowpass in the loop (0 to 0.4),
//! so the loop gain stays below 1 and the tail always ends.

use nemo_core::effects::ReverbSettings;

/// Most channels the reverb processes. Channels past the eighth pass through unchanged.
pub const REVERB_MAX_CHANNELS: usize = 8;

/// The rate the delay tables are written for.
const TUNING_RATE: u64 = 44_100;
/// Comb lengths at 44.1 kHz (Freeverb).
const COMB_TUNING: [u64; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
/// All-pass lengths at 44.1 kHz (Freeverb).
const ALLPASS_TUNING: [u64; 4] = [556, 441, 341, 225];
/// Extra length per channel at 44.1 kHz, so the channels do not ring in step.
const SPREAD: u64 = 23;
/// The spread of the eighth channel, the longest tables.
const WIDEST_SPREAD: u64 = SPREAD * 7;
/// Input gain into the combs (Freeverb's fixed gain).
const INPUT_GAIN: f32 = 0.015;
/// Feedback of the all-pass filters.
const ALLPASS_FEEDBACK: f32 = 0.5;
/// Comb feedback at size 0%.
const FEEDBACK_MIN: f32 = 0.70;
/// Added comb feedback at size 100%: 0.97 at most, which keeps the tail under 10 s.
const FEEDBACK_SPAN: f32 = 0.27;
/// Lowpass coefficient in the comb loop at damping 100%.
const DAMP_SPAN: f32 = 0.4;
/// How far the tail must fall below the sound that made it: -60 dB.
const TAIL_FLOOR: f64 = 1e-3;
/// The longest tail, in seconds.
const TAIL_MAX_S: u64 = 10;
/// Values below this size (-600 dB) are stored as zero, so a dying tail never reaches the
/// slow subnormal range of `f32`.
const FLOOR: f32 = 1e-30;

/// A table length at 44.1 kHz scaled to `rate`, at least one frame.
fn scaled(len: u64, rate: u32) -> usize {
    let frames = (len * u64::from(rate) + TUNING_RATE / 2) / TUNING_RATE;
    usize::try_from(frames).unwrap_or(usize::MAX).max(1)
}

/// `value`, or 0 when it is below [`FLOOR`].
#[inline]
fn flushed(value: f32) -> f32 {
    if value.abs() < FLOOR { 0.0 } else { value }
}

/// Reverb settings turned into gains and a tail length for one rate. `Copy`, so the control
/// thread can hand them to the callback through a ring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReverbParams {
    feedback: f32,
    damp: f32,
    wet: f32,
    dry: f32,
    tail_frames: u64,
}

impl ReverbParams {
    /// The parameters of `settings` at `rate` Hz. Size, damping and mix are clamped to 100%
    /// first. `enabled` is the chain's business and is not read here. Runs on the control
    /// thread.
    #[must_use]
    pub fn new(settings: &ReverbSettings, rate: u32) -> Self {
        let share = |pct: u8| f32::from(pct.min(100)) / 100.0;
        let feedback = FEEDBACK_MIN + FEEDBACK_SPAN * share(settings.size_pct);
        let wet = share(settings.mix_pct);
        Self {
            feedback,
            damp: DAMP_SPAN * share(settings.damping_pct),
            wet,
            dry: 1.0 - wet,
            tail_frames: tail_frames(feedback, rate),
        }
    }

    /// How long the reverb rings after the input stops, in frames: until the slowest comb has
    /// fallen by 60 dB and the all-pass filters have rung out, capped at 10 s. It follows the
    /// size; damping only shortens the real tail.
    #[must_use]
    pub const fn tail_frames(&self) -> u64 {
        self.tail_frames
    }
}

/// Loops of the longest comb (with the widest channel spread) until its feedback has taken
/// it 60 dB down, plus ten loops of each all-pass (0.5^10 is below -60 dB), capped at 10 s.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the loop count is positive and at most 227 (feedback 0.97)"
)]
fn tail_frames(feedback: f32, rate: u32) -> u64 {
    let longest = COMB_TUNING.iter().max().copied().unwrap_or(0) + WIDEST_SPREAD;
    let loops = (TAIL_FLOOR.ln() / f64::from(feedback).ln()).ceil() as u64;
    let allpass: u64 = ALLPASS_TUNING
        .iter()
        .map(|&len| 10 * (len + WIDEST_SPREAD))
        .sum();
    let at_tuning = loops * longest + allpass;
    let frames = (at_tuning * u64::from(rate)).div_ceil(TUNING_RATE);
    frames.min(TAIL_MAX_S * u64::from(rate))
}

/// One delay line of a comb or an all-pass filter.
#[derive(Debug, Clone)]
struct Line {
    buf: Vec<f32>,
    pos: usize,
}

impl Line {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; len],
            pos: 0,
        }
    }

    /// The sample written one line length ago, or silence while `since_reset` frames do not
    /// yet reach back that far.
    ///
    /// RT-SAFE: one read.
    #[inline]
    fn read(&self, since_reset: usize) -> f32 {
        if since_reset >= self.buf.len() {
            self.buf.get(self.pos).copied().unwrap_or(0.0)
        } else {
            0.0
        }
    }

    /// Store `value` at the read position and step on.
    ///
    /// RT-SAFE: one write.
    #[inline]
    fn write_and_step(&mut self, value: f32) {
        if let Some(slot) = self.buf.get_mut(self.pos) {
            *slot = flushed(value);
        }
        self.pos = if self.pos + 1 >= self.buf.len() {
            0
        } else {
            self.pos + 1
        };
    }
}

/// A lowpass-feedback comb filter.
#[derive(Debug, Clone)]
struct Comb {
    line: Line,
    /// The lowpass state in the loop.
    store: f32,
}

/// The reverb of one channel.
#[derive(Debug, Clone)]
struct Lane {
    combs: [Comb; 8],
    allpasses: [Line; 4],
}

impl Lane {
    fn new(rate: u32, spread: u64) -> Self {
        Self {
            combs: COMB_TUNING.map(|len| Comb {
                line: Line::new(scaled(len + spread, rate)),
                store: 0.0,
            }),
            allpasses: ALLPASS_TUNING.map(|len| Line::new(scaled(len + spread, rate))),
        }
    }

    /// The wet output for one input sample.
    ///
    /// RT-SAFE: eight combs and four all-passes on preallocated lines.
    #[inline]
    fn process(&mut self, sample: f32, p: &ReverbParams, since_reset: usize) -> f32 {
        let input = sample * INPUT_GAIN;
        let mut sum = 0.0;
        for comb in &mut self.combs {
            let out = comb.line.read(since_reset);
            comb.store = flushed(out * (1.0 - p.damp) + comb.store * p.damp);
            comb.line.write_and_step(input + comb.store * p.feedback);
            sum += out;
        }
        for allpass in &mut self.allpasses {
            let delayed = allpass.read(since_reset);
            allpass.write_and_step(sum + delayed * ALLPASS_FEEDBACK);
            sum = delayed - sum;
        }
        sum
    }
}

/// The reverb for one stream.
#[derive(Debug, Clone)]
pub struct Reverb {
    lanes: Vec<Lane>,
    /// Samples per frame of the stream.
    channels: usize,
    /// Frames processed since the last reset; lines read as silence until it reaches them.
    since_reset: usize,
    params: ReverbParams,
}

impl Reverb {
    /// A silent reverb for an interleaved stream of `channels` channels at `rate` Hz, with the
    /// tables scaled to that rate. Allocates (about 190 KB per channel at 192 kHz): call it
    /// when the pipeline starts, never in the callback.
    #[must_use]
    pub fn new(channels: usize, rate: u32, params: ReverbParams) -> Self {
        let channels = channels.max(1);
        let lanes = (0..channels.min(REVERB_MAX_CHANNELS))
            .map(|k| Lane::new(rate, SPREAD * u64::try_from(k).unwrap_or(0)))
            .collect();
        Self {
            lanes,
            channels,
            since_reset: 0,
            params,
        }
    }

    /// The parameters in use.
    #[must_use]
    pub const fn params(&self) -> ReverbParams {
        self.params
    }

    /// Use `params` from the next frame on, keeping the tail in the lines.
    ///
    /// RT-SAFE: a copy. No allocation, lock or logging.
    pub const fn set_params(&mut self, params: ReverbParams) {
        self.params = params;
    }

    /// Forget the tail, as after a seek or a non-finite sample.
    ///
    /// RT-SAFE: one store per comb; the old line content then reads as silence. No
    /// allocation, lock or logging.
    pub fn reset(&mut self) {
        self.since_reset = 0;
        for lane in &mut self.lanes {
            for comb in &mut lane.combs {
                comb.store = 0.0;
            }
        }
    }

    /// Add reverb to the interleaved `block` in place. A trailing partial frame is left
    /// unchanged.
    ///
    /// RT-SAFE: runs the preallocated lines. No allocation, lock or logging.
    pub fn process(&mut self, block: &mut [f32]) {
        let p = self.params;
        for frame in block.chunks_exact_mut(self.channels) {
            for (sample, lane) in frame.iter_mut().zip(self.lanes.iter_mut()) {
                let wet = lane.process(*sample, &p, self.since_reset);
                *sample = p.dry * *sample + p.wet * wet;
            }
            self.since_reset = self.since_reset.saturating_add(1);
        }
    }
}

#[cfg(test)]
mod tests;
