//! The loudness of a whole file, for the opt-in `ReplayGain` scan of untagged files: decode it
//! with [`StreamDecoder`] and measure the EBU R128 (ITU-R BS.1770) integrated loudness and true
//! peak with `ebur128`.
//!
//! Runs on the library's low-priority scan thread, never in the audio callback. Nothing is
//! written to the file; the library stores the result.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use ebur128::{Channel, EbuR128, Mode};
use nemo_core::audio::AudioFormat;
use nemo_core::library::{Loudness, LoudnessError, LoudnessMeter};

use crate::decoder::DecodeError;
use crate::source::FileSource;
use crate::stream_decoder::StreamDecoder;

/// Measures files with `ebur128`. `nemo-app` hands it to the library's loudness scan.
#[derive(Debug, Clone, Copy, Default)]
pub struct AudioMeter;

impl LoudnessMeter for AudioMeter {
    /// Decode the file at `path` and measure it, checking `cancel` before every block.
    ///
    /// A mono file is measured as dual mono, the way the player plays it on two speakers.
    ///
    /// # Errors
    /// [`LoudnessError::Cancelled`] when `cancel` is set, [`LoudnessError::Unreadable`] when the
    /// file cannot be opened, [`LoudnessError::Decode`] when it cannot be decoded, and
    /// [`LoudnessError::Measure`] when the meter refuses the format or the file has no
    /// measurable loudness (silence, or under 400 ms of audio).
    fn measure(&self, path: &Path, cancel: &AtomicBool) -> Result<Loudness, LoudnessError> {
        if cancel.load(Ordering::Relaxed) {
            return Err(LoudnessError::Cancelled);
        }
        let source = FileSource::open(path).map_err(|_| LoudnessError::Unreadable)?;
        let extension = path.extension().and_then(|e| e.to_str());
        let mut decoder =
            StreamDecoder::open(Box::new(source), extension).map_err(|e| match e {
                DecodeError::Open(_) => LoudnessError::Unreadable,
                _ => LoudnessError::Decode,
            })?;
        let format = decoder.info().format;
        measure_blocks(format, cancel, |block| {
            decoder.next_block(block).map_err(|_| LoudnessError::Decode)
        })
    }
}

/// Feed every block `next` writes into one meter and read the result. `next` returns `false`
/// at the end. `cancel` is checked before each block, so a stop costs at most one block.
fn measure_blocks(
    format: AudioFormat,
    cancel: &AtomicBool,
    mut next: impl FnMut(&mut Vec<f32>) -> Result<bool, LoudnessError>,
) -> Result<Loudness, LoudnessError> {
    let refused = |_| LoudnessError::Measure;
    let channels = u32::from(format.channels);
    let mut meter =
        EbuR128::new(channels, format.sample_rate, Mode::I | Mode::TRUE_PEAK).map_err(refused)?;
    if channels == 1 {
        // The player duplicates a mono file onto both speakers (channel mix 1 to 2), so it is
        // measured as that signal on two channels.
        meter.set_channel(0, Channel::DualMono).map_err(refused)?;
    }
    let mut block = Vec::new();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(LoudnessError::Cancelled);
        }
        if !next(&mut block)? {
            break;
        }
        meter.add_frames_f32(&block).map_err(refused)?;
    }
    let integrated = meter.loudness_global().map_err(refused)?;
    let mut true_peak = 0.0_f64;
    for channel in 0..channels {
        true_peak = true_peak.max(meter.true_peak(channel).map_err(refused)?);
    }
    if !integrated.is_finite() {
        return Err(LoudnessError::Measure);
    }
    Ok(loudness(integrated, true_peak))
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "loudness in LUFS and a peak near 1.0 fit f32 with room to spare"
)]
fn loudness(integrated: f64, true_peak: f64) -> Loudness {
    Loudness {
        integrated_lufs: integrated as f32,
        true_peak: true_peak as f32,
    }
}

#[cfg(test)]
mod tests;
