//! Audio sinks: where rendered samples go.
//!
//! [`CaptureSink`] is for tests (no device). [`CpalSink`] opens the default output device in
//! shared mode and refuses a device whose rate or channel count differs from the source.

use nemo_core::audio::{AudioFormat, OutputInfo, OutputMode};

pub use crate::cpal_sink::{CpalSink, CpalSinkFactory, OutputDeviceInfo, list_output_devices};

/// The function a sink calls to fetch samples. It runs on the audio thread, so it must be
/// real-time safe.
pub type RenderCallback = Box<dyn FnMut(&mut [f32]) + Send + 'static>;

/// Why a sink could not open or start.
#[derive(Debug, thiserror::Error)]
pub enum SinkError {
    /// There is no default output device.
    #[error("no default output device")]
    NoDevice,
    /// No output device has the requested name.
    #[error("no output device named {0}")]
    DeviceNotFound(String),
    /// The device rate or channel count differs from the source. No resampling is done.
    #[error(
        "device format {device_rate} Hz / {device_channels} ch does not match source \
         {source_rate} Hz / {source_channels} ch"
    )]
    FormatMismatch {
        /// Source sample rate in hertz.
        source_rate: u32,
        /// Source channel count.
        source_channels: u16,
        /// Device sample rate in hertz.
        device_rate: u32,
        /// Device channel count.
        device_channels: u16,
    },
    /// The device default sample format is not 32-bit float.
    #[error("device default sample format is {0}, only f32, i16 and i32 are supported")]
    UnsupportedSampleFormat(String),
    /// The audio backend reported an error.
    #[error("audio backend error: {0}")]
    Backend(String),
    /// The sink was started twice.
    #[error("the sink is already started")]
    AlreadyStarted,
}

/// Check that a device format equals the source format in rate and channels.
///
/// # Errors
///
/// Returns [`SinkError::FormatMismatch`] when the rate or channel count differs.
pub fn check_format(source: AudioFormat, device: AudioFormat) -> Result<(), SinkError> {
    if source.sample_rate == device.sample_rate && source.channels == device.channels {
        Ok(())
    } else {
        Err(SinkError::FormatMismatch {
            source_rate: source.sample_rate,
            source_channels: source.channels,
            device_rate: device.sample_rate,
            device_channels: device.channels,
        })
    }
}

/// A destination for rendered samples.
pub trait AudioSink {
    /// The device end of the signal path.
    fn output_info(&self) -> &OutputInfo;
    /// Begin pulling samples through `callback`.
    ///
    /// # Errors
    ///
    /// Returns [`SinkError`] when the stream cannot be built or started.
    fn start(&mut self, callback: RenderCallback) -> Result<(), SinkError>;
    /// Stop pulling samples.
    fn stop(&mut self);
    /// Frames between the render callback writing a sample and the speakers playing it, as the
    /// device last reported. Zero when the sink does not know.
    fn latency_frames(&self) -> u64 {
        0
    }
}

/// Opens the output sink for a track. Called on the player's control thread.
pub trait SinkFactory: Send + Sync {
    /// Open a sink for audio in the source format. A sink may return another format (a shared
    /// device keeps its own); the player then adds resampling or channel mixing.
    ///
    /// # Errors
    /// Returns [`SinkError`] when no device can be opened.
    fn open(&self, source: AudioFormat) -> Result<Box<dyn AudioSink>, SinkError>;

    /// `true` when [open](Self::open) can return a sink in the source rate and channel count
    /// (an exclusive device). The player then reopens the sink when the next track has another
    /// format. alse when the device fixes the format (shared mode).
    fn can_follow_source(&self) -> bool {
        false
    }
}

/// A sink that records what it is given. The caller drives it with [`CaptureSink::pump`].
pub struct CaptureSink {
    output: OutputInfo,
    callback: Option<RenderCallback>,
    scratch: Vec<f32>,
    captured: Vec<f32>,
}

impl CaptureSink {
    /// Create a capture sink that pretends to be a shared-mode device of `format`.
    #[must_use]
    pub fn new(format: AudioFormat) -> Self {
        Self {
            output: OutputInfo {
                device_name: "Capture (test)".to_owned(),
                mode: OutputMode::Shared,
                format,
            },
            callback: None,
            scratch: Vec::new(),
            captured: Vec::new(),
        }
    }

    /// Run the callback once for `frames` frames and record the result. Does nothing before
    /// [`AudioSink::start`].
    pub fn pump(&mut self, frames: usize) {
        let samples = frames * usize::from(self.output.format.channels);
        if let Some(callback) = self.callback.as_mut() {
            self.scratch.resize(samples, 0.0);
            callback(&mut self.scratch);
            self.captured.extend_from_slice(&self.scratch);
        }
    }

    /// Every sample captured so far, interleaved.
    #[must_use]
    pub fn captured(&self) -> &[f32] {
        &self.captured
    }
}

impl AudioSink for CaptureSink {
    fn output_info(&self) -> &OutputInfo {
        &self.output
    }

    fn start(&mut self, callback: RenderCallback) -> Result<(), SinkError> {
        if self.callback.is_some() {
            return Err(SinkError::AlreadyStarted);
        }
        self.callback = Some(callback);
        Ok(())
    }

    fn stop(&mut self) {
        self.callback = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(rate: u32, channels: u16) -> AudioFormat {
        AudioFormat {
            sample_rate: rate,
            channels,
            bit_depth: None,
        }
    }

    #[test]
    fn matching_formats_pass_and_others_are_typed_errors() {
        assert!(check_format(fmt(48_000, 2), fmt(48_000, 2)).is_ok());
        assert!(matches!(
            check_format(fmt(44_100, 2), fmt(48_000, 2)),
            Err(SinkError::FormatMismatch {
                source_rate: 44_100,
                device_rate: 48_000,
                ..
            })
        ));
        assert!(matches!(
            check_format(fmt(48_000, 1), fmt(48_000, 2)),
            Err(SinkError::FormatMismatch { .. })
        ));
    }
}
