//! Song transitions in the feeder: a crossfade, a silence trim or a gapless join.
//!
//! [`TransitionMixer`] runs on the feeder thread only, never in the audio callback. It holds
//! the last L frames of the playing song, already in the output format, in a tail window that
//! is allocated when the song starts. At the song's end it scans that tail and the head of the
//! next song for silence, picks the transition, and mixes the two before anything reaches the
//! ring. It never changes tempo or aligns beats.

use std::collections::VecDeque;
use std::f64::consts::FRAC_PI_2;

use nemo_core::audio::AudioFormat;
use nemo_core::effects::{TransitionKind, TransitionMode};

use crate::units::f64_of;

/// Samples below this size (-60 dBFS) are silence. A frame is silent when all its samples are.
pub const SILENCE_FLOOR: f32 = 1e-3;
/// How much of the next song is scanned for leading silence, in ms.
pub const HEAD_SCAN_MS: u64 = 10_000;
/// The overlap of a silence trim, in ms.
pub const TRIM_OVERLAP_MS: u64 = 300;
/// Silence at an end up to this length (ms) counts as a clean end; longer silence is trimmed.
pub const CLEAN_END_MS: u64 = 50;

/// The transition control attaches to a song: the mode and the crossfade length L.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionPlan {
    /// `Crossfade` always crossfades, `Smart` scans each pair and picks, `Off` joins gaplessly.
    pub kind_hint: TransitionMode,
    /// The crossfade length L, in frames of the output format.
    pub frames: u64,
}

/// What [`TransitionMixer::mix_into`] wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transition {
    /// The transition that ran. A crossfade with nothing to overlap is reported as `Gapless`.
    pub kind: TransitionKind,
    /// Frames written before the overlap starts.
    pub overlap_start: usize,
    /// Frames in which both songs sound.
    pub overlap_frames: usize,
    /// Frames at the start of the next song's head dropped as silence.
    pub head_skipped: usize,
}

impl Transition {
    /// Frames of the next song's head the transition used (dropped silence and overlap). The
    /// feeder goes on with the next song from there.
    #[must_use]
    pub const fn head_used(&self) -> usize {
        self.head_skipped + self.overlap_frames
    }

    /// The frame of the written output where the next song takes over: the middle of the
    /// overlap, where its `Started` event belongs.
    #[must_use]
    pub const fn handover_frame(&self) -> usize {
        self.overlap_start + self.overlap_frames / 2
    }

    /// The next song's own position at the hand-over, in frames: L/2 for a full crossfade.
    #[must_use]
    pub const fn handover_position(&self) -> usize {
        self.head_skipped + self.overlap_frames / 2
    }
}

/// The tail window of the playing song and the transition into the next one. Feeder only.
#[derive(Debug, Clone)]
pub struct TransitionMixer {
    /// The last frames of the song, interleaved, at most `capacity` samples.
    window: VecDeque<f32>,
    capacity: usize,
    channels: usize,
    rate: u32,
    hint: TransitionMode,
}

impl TransitionMixer {
    /// A mixer for a song in the output `format` with `plan`. Allocates the tail window of L
    /// frames, so call it when the song starts. With the mode `Off` there is no window.
    #[must_use]
    pub fn new(format: AudioFormat, plan: TransitionPlan) -> Self {
        let channels = usize::from(format.channels.max(1));
        let frames = if plan.kind_hint == TransitionMode::Off {
            0
        } else {
            usize::try_from(plan.frames).unwrap_or(usize::MAX)
        };
        let capacity = frames.saturating_mul(channels);
        Self {
            window: VecDeque::with_capacity(capacity),
            capacity,
            channels,
            rate: format.sample_rate,
            hint: plan.kind_hint,
        }
    }

    /// Pass an interleaved `block` of the playing song through the tail window. The frames
    /// that leave the window, the oldest first, are appended to `out` for the ring.
    pub fn push(&mut self, block: &[f32], out: &mut Vec<f32>) {
        if self.capacity == 0 {
            out.extend_from_slice(block);
            return;
        }
        for &sample in block {
            if self.window.len() == self.capacity
                && let Some(oldest) = self.window.pop_front()
            {
                out.push(oldest);
            }
            self.window.push_back(sample);
        }
    }

    /// Frames in the tail window. Less than L when the song was shorter or has not filled it
    /// yet; the transition then shrinks to what is there.
    #[must_use]
    pub fn window_frames(&self) -> usize {
        self.window.len() / self.channels
    }

    /// The transition into a next song whose first frames, in the output format, are `head`.
    /// `join_without_fade` is the gapless-mark policy's answer for the pair.
    ///
    /// Gapless when the policy says so, the mode is `Off` or there is no tail. The mode
    /// `Crossfade` always crossfades. `Smart` scans the tail for trailing silence and the first
    /// 10 s of `head` for leading silence: more than 50 ms of silence at either end is
    /// trimmed (`TrimSilence`); two clean ends, each reaching silence within 50 ms, join
    /// gaplessly; otherwise the songs crossfade.
    pub fn at_end(&mut self, head: &[f32], join_without_fade: bool) -> TransitionKind {
        if join_without_fade || self.hint == TransitionMode::Off || self.window.is_empty() {
            return TransitionKind::Gapless;
        }
        if self.hint == TransitionMode::Crossfade {
            return TransitionKind::Crossfade;
        }
        let channels = self.channels;
        let trailing = trailing_silence(self.window.make_contiguous(), channels);
        let leading = leading_silence(self.scanned(head), channels);
        let clean = self.ms_frames(CLEAN_END_MS);
        if trailing > clean || leading > clean {
            TransitionKind::TrimSilence
        } else if trailing > 0 && leading > 0 {
            TransitionKind::Gapless
        } else {
            TransitionKind::Crossfade
        }
    }

    /// Write the transition of `kind` into `out` and empty the window: the tail, with its end
    /// mixed into the start of `head` by equal-power gains. `Crossfade` overlaps the whole
    /// tail, `TrimSilence` drops the silence at both ends and overlaps 300 ms, `Gapless`
    /// writes the tail unchanged. The rest of `head`, from [`Transition::head_used`], is the
    /// feeder's to pass on.
    pub fn mix_into(
        &mut self,
        kind: TransitionKind,
        head: &[f32],
        out: &mut Vec<f32>,
    ) -> Transition {
        let channels = self.channels;
        let head_frames = head.len() / channels;
        let skip = if kind == TransitionKind::TrimSilence {
            leading_silence(self.scanned(head), channels)
        } else {
            0
        };
        let trim_overlap = self.ms_frames(TRIM_OVERLAP_MS);
        let tail = self.window.make_contiguous();
        let tail_frames = tail.len() / channels;
        let (keep, overlap) = match kind {
            TransitionKind::Gapless => (tail_frames, 0),
            TransitionKind::Crossfade => (tail_frames, tail_frames.min(head_frames)),
            TransitionKind::TrimSilence => {
                let keep = tail_frames - trailing_silence(tail, channels);
                (keep, trim_overlap.min(keep).min(head_frames - skip))
            }
        };
        let plain = keep - overlap;
        out.extend_from_slice(tail.get(..plain * channels).unwrap_or_default());
        let fading = tail
            .get(plain * channels..keep * channels)
            .unwrap_or_default();
        let rising = head
            .get(skip * channels..(skip + overlap) * channels)
            .unwrap_or_default();
        let frames = fading
            .chunks_exact(channels)
            .zip(rising.chunks_exact(channels));
        for (i, (from, to)) in frames.enumerate() {
            let (out_gain, in_gain) = equal_power(i, overlap);
            out.extend(from.iter().zip(to).map(|(a, b)| a * out_gain + b * in_gain));
        }
        self.window.clear();
        let kind = if kind == TransitionKind::Crossfade && overlap == 0 {
            TransitionKind::Gapless
        } else {
            kind
        };
        Transition {
            kind,
            overlap_start: plain,
            overlap_frames: overlap,
            head_skipped: skip,
        }
    }

    /// The part of `head` scanned for leading silence: its first 10 s.
    fn scanned<'a>(&self, head: &'a [f32]) -> &'a [f32] {
        let samples = self.ms_frames(HEAD_SCAN_MS).saturating_mul(self.channels);
        head.get(..samples).unwrap_or(head)
    }

    fn ms_frames(&self, ms: u64) -> usize {
        usize::try_from(u64::from(self.rate) * ms / 1000).unwrap_or(usize::MAX)
    }
}

/// Gains of frame `i` of an `n`-frame equal-power overlap: `(cos, sin)` of a quarter turn
/// sampled at frame centres, so the two squared always sum to 1.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the gains lie in 0..=1 and are applied to f32 samples"
)]
fn equal_power(i: usize, n: usize) -> (f32, f32) {
    let at = |k: usize| f64_of(u64::try_from(k).unwrap_or(u64::MAX));
    let angle = (at(i) + 0.5) / at(n.max(1)) * FRAC_PI_2;
    (angle.cos() as f32, angle.sin() as f32)
}

fn is_silent(frame: &[f32]) -> bool {
    frame.iter().all(|s| s.abs() < SILENCE_FLOOR)
}

/// Silent frames at the end of the interleaved `samples`.
fn trailing_silence(samples: &[f32], channels: usize) -> usize {
    samples
        .chunks_exact(channels)
        .rev()
        .take_while(|f| is_silent(f))
        .count()
}

/// Silent frames at the start of the interleaved `samples`.
fn leading_silence(samples: &[f32], channels: usize) -> usize {
    samples
        .chunks_exact(channels)
        .take_while(|f| is_silent(f))
        .count()
}

#[cfg(test)]
mod tests;
