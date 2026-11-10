//! The sample-altering stages between the decoder and the ring: channel mix and resample.
//!
//! When the source already has the device's rate and channel count the chain is empty and
//! [`Chain::process`] only copies the samples.

use nemo_core::audio::AudioFormat;

use crate::mix::ChannelMixer;
use crate::resample::{ResampleFailure, StreamResampler};

/// Converts decoded blocks to the output format.
pub struct Chain {
    mixer: Option<ChannelMixer>,
    resampler: Option<StreamResampler>,
    /// `true` when mixing runs before resampling (the channel count shrinks).
    mix_first: bool,
    a: Vec<f32>,
    b: Vec<f32>,
}

impl Chain {
    /// The chain from `source` to `output`. Both stages are absent when the formats agree.
    ///
    /// # Errors
    /// [`ResampleFailure`] when the resampler cannot be built.
    pub fn new(source: AudioFormat, output: AudioFormat) -> Result<Self, ResampleFailure> {
        let mix_first = source.channels > output.channels;
        let rate_channels = if mix_first {
            output.channels
        } else {
            source.channels
        };
        let resampler = (source.sample_rate != output.sample_rate)
            .then(|| StreamResampler::new(source.sample_rate, output.sample_rate, rate_channels))
            .transpose()?;
        Ok(Self {
            mixer: ChannelMixer::new(source.channels, output.channels),
            resampler,
            mix_first,
            a: Vec::new(),
            b: Vec::new(),
        })
    }

    /// Convert `input` and append the result to `out`.
    ///
    /// # Errors
    /// [`ResampleFailure`] if the resampler rejects the data.
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<(), ResampleFailure> {
        match (self.mixer, self.resampler.as_mut()) {
            (None, None) => out.extend_from_slice(input),
            (Some(m), None) => {
                m.mix(input, &mut self.a);
                out.extend_from_slice(&self.a);
            }
            (None, Some(r)) => r.push(input, out)?,
            (Some(m), Some(r)) if self.mix_first => {
                m.mix(input, &mut self.a);
                r.push(&self.a, out)?;
            }
            (Some(m), Some(r)) => {
                self.a.clear();
                r.push(input, &mut self.a)?;
                m.mix(&self.a, &mut self.b);
                out.extend_from_slice(&self.b);
            }
        }
        Ok(())
    }

    /// Flush the resampler tail into `out`.
    ///
    /// # Errors
    /// [`ResampleFailure`] if the resampler rejects the flush.
    pub fn finish(&mut self, out: &mut Vec<f32>) -> Result<(), ResampleFailure> {
        let Some(r) = self.resampler.as_mut() else {
            return Ok(());
        };
        if self.mixer.is_some() && !self.mix_first {
            self.a.clear();
            r.finish(&mut self.a)?;
            if let Some(m) = self.mixer {
                m.mix(&self.a, &mut self.b);
            }
            out.extend_from_slice(&self.b);
            Ok(())
        } else {
            r.finish(out)
        }
    }

    /// Forget buffered samples, for a seek.
    pub fn reset(&mut self) {
        if let Some(r) = self.resampler.as_mut() {
            r.reset();
        }
    }
}
