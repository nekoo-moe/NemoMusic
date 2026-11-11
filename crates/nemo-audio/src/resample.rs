//! Sample-rate conversion with a windowed-sinc resampler (`rubato`). Created only when the
//! source rate differs from the device rate.

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{
    Async, FixedAsync, Indexing, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

/// Input frames per resampler call.
const CHUNK_FRAMES: usize = 1024;
/// Upper bound on flush rounds, so a faulty resampler cannot loop forever.
const MAX_FLUSH_ROUNDS: usize = 16;

/// Why resampling failed.
#[derive(Debug, thiserror::Error)]
#[error("resampling failed: {0}")]
pub struct ResampleFailure(String);

fn fail(err: impl std::fmt::Display) -> ResampleFailure {
    ResampleFailure(err.to_string())
}

/// Converts interleaved `f32` frames from one sample rate to another, chunk by chunk.
///
/// The filter is a 256-tap windowed sinc (Blackman-Harris) with cubic interpolation between
/// 256 filter phases. The start-up delay of the filter is removed and [`finish`](Self::finish)
/// flushes the tail, so the output length is `round(input_frames * to / from)`.
pub struct StreamResampler {
    inner: Async<f32>,
    channels: usize,
    ratio: f64,
    pending: Vec<f32>,
    scratch: Vec<f32>,
    delay_left: usize,
    in_frames: u64,
    out_frames: u64,
}

impl StreamResampler {
    /// A resampler from `from_hz` to `to_hz` for `channels` interleaved channels.
    ///
    /// # Errors
    /// [`ResampleFailure`] when the rates or channel count are rejected.
    pub fn new(from_hz: u32, to_hz: u32, channels: u16) -> Result<Self, ResampleFailure> {
        let ratio = f64::from(to_hz) / f64::from(from_hz);
        let params = SincInterpolationParameters::new(256, WindowFunction::BlackmanHarris2)
            .oversampling_factor(256)
            .interpolation(SincInterpolationType::Cubic);
        let channels = usize::from(channels);
        let inner = Async::<f32>::new_sinc(
            ratio,
            1.0,
            &params,
            CHUNK_FRAMES,
            channels,
            FixedAsync::Input,
        )
        .map_err(fail)?;
        let scratch = vec![0.0; inner.output_frames_max() * channels];
        let delay_left = inner.output_delay();
        Ok(Self {
            inner,
            channels,
            ratio,
            pending: Vec::new(),
            scratch,
            delay_left,
            in_frames: 0,
            out_frames: 0,
        })
    }

    /// Feed interleaved frames; converted frames are appended to `out`.
    ///
    /// # Errors
    /// [`ResampleFailure`] if the resampler rejects a chunk.
    pub fn push(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<(), ResampleFailure> {
        self.in_frames += (input.len() / self.channels) as u64;
        self.pending.extend_from_slice(input);
        loop {
            let need = self.inner.input_frames_next();
            if self.pending.len() < need * self.channels {
                return Ok(());
            }
            self.run_chunk(need, None, out)?;
        }
    }

    /// Process `need` pending frames, optionally only `partial` of them being real input.
    fn run_chunk(
        &mut self,
        need: usize,
        partial: Option<usize>,
        out: &mut Vec<f32>,
    ) -> Result<(), ResampleFailure> {
        let ch = self.channels;
        let max_out = self.scratch.len() / ch;
        let indexing = partial.map(|n| Indexing::new().partial_len(n));
        let input = InterleavedSlice::new(&self.pending[..need * ch], ch, need).map_err(fail)?;
        let mut output = InterleavedSlice::new_mut(&mut self.scratch, ch, max_out).map_err(fail)?;
        let (used, made) = self
            .inner
            .process_into_buffer(&input, &mut output, indexing.as_ref())
            .map_err(fail)?;
        self.pending.drain(..used.min(need) * ch);
        let skip = self.delay_left.min(made);
        self.delay_left -= skip;
        out.extend_from_slice(&self.scratch[skip * ch..made * ch]);
        self.out_frames += (made - skip) as u64;
        Ok(())
    }

    /// Flush the filter tail into `out`, so the total output length is exact. Call at the end
    /// of a stream, then [`reset`](Self::reset) before reusing the resampler.
    ///
    /// # Errors
    /// [`ResampleFailure`] if the resampler rejects a chunk.
    pub fn finish(&mut self, out: &mut Vec<f32>) -> Result<(), ResampleFailure> {
        if self.in_frames == 0 {
            return Ok(());
        }
        // Rounding to the nearest whole frame is the definition of the expected length.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let expected = (crate::units::f64_of(self.in_frames) * self.ratio).round() as u64;
        let start_len = out.len();
        let need = self.inner.input_frames_next();
        let real = self.pending.len() / self.channels;
        self.pending.resize(need * self.channels, 0.0);
        self.run_chunk(need, Some(real), out)?;
        for _ in 0..MAX_FLUSH_ROUNDS {
            if self.out_frames >= expected {
                break;
            }
            let need = self.inner.input_frames_next();
            self.pending.clear();
            self.pending.resize(need * self.channels, 0.0);
            self.run_chunk(need, None, out)?;
        }
        let surplus = usize::try_from(self.out_frames.saturating_sub(expected)).unwrap_or(0);
        let keep = out
            .len()
            .saturating_sub(surplus * self.channels)
            .max(start_len);
        out.truncate(keep);
        self.out_frames = expected;
        Ok(())
    }

    /// Forget all state, for example after a seek.
    pub fn reset(&mut self) {
        self.inner.reset();
        self.pending.clear();
        self.delay_left = self.inner.output_delay();
        self.in_frames = 0;
        self.out_frames = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(rate: u32, freq: f32, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| {
                let t = f64::from(u32::try_from(i).expect("small")) / f64::from(rate);
                #[allow(clippy::cast_possible_truncation)] // a sample value
                let v = (0.5 * (2.0 * std::f64::consts::PI * f64::from(freq) * t).sin()) as f32;
                [v, v]
            })
            .collect()
    }

    fn run(chunk_frames: usize, seconds: usize) -> usize {
        let mut r = StreamResampler::new(44_100, 48_000, 2).expect("resampler");
        let input = sine(44_100, 1000.0, 44_100 * seconds);
        let mut out = Vec::new();
        for chunk in input.chunks(2 * chunk_frames) {
            r.push(chunk, &mut out).expect("push");
        }
        r.finish(&mut out).expect("finish");
        out.len() / 2
    }

    #[test]
    fn output_length_is_the_ratio_within_one_frame() {
        for chunk in [777, 4096] {
            assert_eq!(run(chunk, 1), 48_000, "chunk {chunk}");
        }
    }
}
