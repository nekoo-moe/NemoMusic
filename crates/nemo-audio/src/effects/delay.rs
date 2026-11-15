//! The delay (echo) stage.
//!
//! A feedback delay line per channel: time 1 ms to 2 s, feedback at most 85% so every echo
//! decays, and a wet/dry mix. [`Delay::new`] sizes the line for 2 s when the pipeline starts;
//! nothing allocates in the callback. [`DelayParams`] are computed and clamped on the control
//! thread.

use nemo_core::effects::DelaySettings;

/// Most channels the delay processes. Channels past the eighth pass through unchanged.
pub const DELAY_MAX_CHANNELS: usize = 8;

/// How far an echo must fall below the sound that made it before the tail ends: -60 dB.
const TAIL_FLOOR: f64 = 1e-3;
/// The longest tail, in seconds.
const TAIL_MAX_S: u64 = 10;
/// Line values below this size (-600 dB) are stored as zero, so a dying echo never reaches
/// the slow subnormal range of `f32`.
const LINE_FLOOR: f32 = 1e-30;

/// Delay settings turned into frames and gains for one rate. `Copy`, so the control thread
/// can hand them to the callback through a ring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DelayParams {
    frames: usize,
    feedback: f32,
    wet: f32,
    dry: f32,
    tail_frames: u64,
}

impl DelayParams {
    /// The parameters of `settings` at `rate` Hz, clamped first: time to 1 to 2000 ms,
    /// feedback to at most 85%, mix to at most 100%. `enabled` is the chain's business and is
    /// not read here. Runs on the control thread.
    #[must_use]
    pub fn new(settings: &DelaySettings, rate: u32) -> Self {
        let time_ms = settings
            .time_ms
            .clamp(DelaySettings::TIME_MIN_MS, DelaySettings::TIME_MAX_MS);
        let feedback_pct = settings.feedback_pct.min(DelaySettings::FEEDBACK_MAX_PCT);
        let wet = f32::from(settings.mix_pct.min(100)) / 100.0;
        let frames = ((u64::from(time_ms) * u64::from(rate) + 500) / 1000).max(1);
        Self {
            frames: usize::try_from(frames).unwrap_or(usize::MAX),
            feedback: f32::from(feedback_pct) / 100.0,
            wet,
            dry: 1.0 - wet,
            tail_frames: tail_frames(frames, feedback_pct, rate),
        }
    }

    /// The delay time in frames.
    #[must_use]
    pub const fn frames(&self) -> usize {
        self.frames
    }

    /// How long the echoes ring after the input stops, in frames: until the last echo at or
    /// above -60 dB has played, capped at 10 s.
    #[must_use]
    pub const fn tail_frames(&self) -> u64 {
        self.tail_frames
    }
}

/// Echo `k` (from 1) is `feedback^(k-1)` as loud as the sound that made it. The tail lasts
/// until every echo at or above -60 dB has started, capped at [`TAIL_MAX_S`].
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the echo count is positive and at most 43 (feedback 85%)"
)]
fn tail_frames(frames: u64, feedback_pct: u8, rate: u32) -> u64 {
    let echoes = if feedback_pct == 0 {
        1
    } else {
        let feedback = f64::from(feedback_pct) / 100.0;
        // The small bias keeps an echo of exactly -60 dB (feedback 10%, echo 4) inside.
        1 + (TAIL_FLOOR.ln() / feedback.ln() + 1e-9).floor() as u64
    };
    echoes
        .saturating_mul(frames)
        .min(TAIL_MAX_S * u64::from(rate))
}

/// The delay line for one stream.
#[derive(Debug, Clone)]
pub struct Delay {
    /// `capacity` frames of `lanes` samples each.
    line: Vec<f32>,
    /// Samples per frame of the stream.
    channels: usize,
    /// Channels that are delayed: `channels` up to [`DELAY_MAX_CHANNELS`].
    lanes: usize,
    /// Frames the line holds: 2 s plus one.
    capacity: usize,
    /// The frame the next sample is written to.
    write: usize,
    /// Frames written since the last reset, up to `capacity`. A slot written before the reset
    /// reads as silence.
    written: usize,
    params: DelayParams,
}

impl Delay {
    /// A silent 2 s line for an interleaved stream of `channels` channels at `rate` Hz.
    /// Allocates up to 8 x 2 s of samples: call it when the pipeline starts, never in the
    /// callback.
    #[must_use]
    pub fn new(channels: usize, rate: u32, params: DelayParams) -> Self {
        let channels = channels.max(1);
        let lanes = channels.min(DELAY_MAX_CHANNELS);
        let two_seconds = u64::from(DelaySettings::TIME_MAX_MS) * u64::from(rate) / 1000;
        let capacity = usize::try_from(two_seconds).unwrap_or(0).saturating_add(1);
        Self {
            line: vec![0.0; lanes.saturating_mul(capacity)],
            channels,
            lanes,
            capacity,
            write: 0,
            written: 0,
            params,
        }
    }

    /// The parameters in use.
    #[must_use]
    pub const fn params(&self) -> DelayParams {
        self.params
    }

    /// Use `params` from the next frame on, keeping the echoes in the line.
    ///
    /// RT-SAFE: a copy. No allocation, lock or logging.
    pub const fn set_params(&mut self, params: DelayParams) {
        self.params = params;
    }

    /// Forget every echo, as after a seek or a non-finite sample.
    ///
    /// RT-SAFE: one store; the old line content then reads as silence. No allocation, lock or
    /// logging.
    pub const fn reset(&mut self) {
        self.written = 0;
    }

    /// Delay the interleaved `block` in place. A trailing partial frame is left unchanged.
    ///
    /// RT-SAFE: reads and writes the preallocated line. No allocation, lock or logging.
    pub fn process(&mut self, block: &mut [f32]) {
        let DelayParams {
            frames,
            feedback,
            wet,
            dry,
            ..
        } = self.params;
        let delay = frames.min(self.capacity.saturating_sub(1));
        for frame in block.chunks_exact_mut(self.channels) {
            let read = if self.write >= delay {
                self.write - delay
            } else {
                self.write + self.capacity - delay
            };
            let live = self.written >= delay;
            let (from, to) = (read * self.lanes, self.write * self.lanes);
            for (lane, sample) in frame.iter_mut().take(self.lanes).enumerate() {
                let delayed = if live {
                    self.line.get(from + lane).copied().unwrap_or(0.0)
                } else {
                    0.0
                };
                let fed = *sample + feedback * delayed;
                if let Some(slot) = self.line.get_mut(to + lane) {
                    *slot = if fed.abs() < LINE_FLOOR { 0.0 } else { fed };
                }
                *sample = dry * *sample + wet * delayed;
            }
            self.write = if self.write + 1 >= self.capacity {
                0
            } else {
                self.write + 1
            };
            self.written = (self.written + 1).min(self.capacity);
        }
    }
}

#[cfg(test)]
mod tests;
