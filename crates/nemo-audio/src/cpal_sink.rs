//! The device sink (`cpal`, shared mode) and output-device listing.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use nemo_core::audio::{AudioFormat, OutputInfo, OutputMode};
use nemo_core::output::{DeviceId, OutputDevice};

use crate::sink::{AudioSink, RenderCallback, SinkError, SinkFactory, SinkHealth, check_format};

/// Samples converted per pass for the integer formats. Fixed so the callback never allocates.
const CONVERT_CHUNK: usize = 8192;

const HEALTH_OK: u8 = 0;
const HEALTH_LOST: u8 = 1;
const HEALTH_TAKEN: u8 = 2;

/// A listed output device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputDeviceInfo {
    /// Human-readable name, also the key for [`CpalSink::open_named`].
    pub name: String,
    /// `true` for the system default device.
    pub is_default: bool,
}

fn device_name(device: &cpal::Device) -> String {
    device
        .description()
        .map_or_else(|_| "Unknown device".to_owned(), |d| d.name().to_owned())
}

/// List the output devices of the default host with their IDs and default mark.
///
/// # Errors
/// [`SinkError::Backend`] when the host cannot enumerate devices.
pub fn list_devices() -> Result<Vec<OutputDevice>, SinkError> {
    let host = cpal::default_host();
    let default_id = host
        .default_output_device()
        .and_then(|d| d.id().ok())
        .map(|id| id.to_string());
    let default_name = host.default_output_device().map(|d| device_name(&d));
    let devices = host
        .output_devices()
        .map_err(|e| SinkError::Backend(e.to_string()))?;
    let mut result = Vec::new();
    for d in devices {
        let Ok(id) = d.id() else {
            tracing::debug!("skipping output device without valid id");
            continue;
        };
        let id_str = id.to_string();
        let name = device_name(&d);
        let is_default = default_id.as_deref() == Some(&id_str)
            || (default_id.is_none() && default_name.as_deref() == Some(&name));
        tracing::debug!(name = %name, is_default, "enumerated output device");
        result.push(OutputDevice {
            id: DeviceId(id_str),
            name,
            is_default,
        });
    }
    Ok(result)
}

/// List the output devices of the default host.
///
/// # Errors
/// [`SinkError::Backend`] when the host cannot enumerate devices.
pub fn list_output_devices() -> Result<Vec<OutputDeviceInfo>, SinkError> {
    Ok(list_devices()?
        .into_iter()
        .map(|d| OutputDeviceInfo {
            name: d.name,
            is_default: d.is_default,
        })
        .collect())
}

/// An output device in shared mode, opened with its default configuration.
///
/// Shared mode means the operating system mixer sits between this app and the hardware, so the
/// path is never reported as bit-perfect. The sink reports the device's own rate and channel
/// count; the player adds resampling or channel mixing when the source differs. Devices that
/// default to `f32`, `i16` or `i32` samples are supported; the conversion to integers only
/// happens when the device needs it.
pub struct CpalSink {
    output: OutputInfo,
    device: cpal::Device,
    config: cpal::StreamConfig,
    sample_format: SampleFormat,
    stream: Option<cpal::Stream>,
    stream_failed: Arc<AtomicBool>,
    health: Arc<AtomicU8>,
    latency_frames: Arc<AtomicU64>,
}

impl CpalSink {
    /// Open the default output device and require it to match `source` in rate and channels.
    ///
    /// # Errors
    ///
    /// [`SinkError::NoDevice`], [`SinkError::Backend`], [`SinkError::UnsupportedSampleFormat`]
    /// or [`SinkError::FormatMismatch`].
    pub fn open(source: AudioFormat) -> Result<Self, SinkError> {
        let sink = Self::open_device(None)?;
        check_format(source, sink.output.format)?;
        Ok(sink)
    }

    /// Open an output device by its [`DeviceId`], or the default one for `None`, with its default format.
    ///
    /// # Errors
    ///
    /// [`SinkError::NoDevice`], [`SinkError::DeviceNotFound`], [`SinkError::Backend`] or
    /// [`SinkError::UnsupportedSampleFormat`].
    pub fn open_device(id: Option<&DeviceId>) -> Result<Self, SinkError> {
        let host = cpal::default_host();
        let device = match id {
            None => host.default_output_device().ok_or(SinkError::NoDevice)?,
            Some(wanted) => {
                let mut matched = None;
                for d in host
                    .output_devices()
                    .map_err(|e| SinkError::Backend(e.to_string()))?
                {
                    if d.id().is_ok_and(|dev_id| dev_id.to_string() == wanted.0) {
                        matched = Some(d);
                        break;
                    }
                }
                matched.ok_or_else(|| SinkError::DeviceNotFound(wanted.0.clone()))?
            }
        };
        Self::from_device(device)
    }

    /// Open the named output device, or the default one for `None`, with its default format.
    ///
    /// # Errors
    ///
    /// [`SinkError::NoDevice`], [`SinkError::DeviceNotFound`], [`SinkError::Backend`] or
    /// [`SinkError::UnsupportedSampleFormat`].
    pub fn open_named(name: Option<&str>) -> Result<Self, SinkError> {
        let host = cpal::default_host();
        let device = match name {
            None => host.default_output_device().ok_or(SinkError::NoDevice)?,
            Some(wanted) => host
                .output_devices()
                .map_err(|e| SinkError::Backend(e.to_string()))?
                .find(|d| device_name(d) == wanted)
                .ok_or_else(|| SinkError::DeviceNotFound(wanted.to_owned()))?,
        };
        Self::from_device(device)
    }

    fn from_device(device: cpal::Device) -> Result<Self, SinkError> {
        let supported = device
            .default_output_config()
            .map_err(|e| SinkError::Backend(e.to_string()))?;
        let sample_format = supported.sample_format();
        if !matches!(
            sample_format,
            SampleFormat::F32 | SampleFormat::I16 | SampleFormat::I32
        ) {
            return Err(SinkError::UnsupportedSampleFormat(
                sample_format.to_string(),
            ));
        }
        let format = AudioFormat {
            sample_rate: supported.sample_rate(),
            channels: supported.channels(),
            bit_depth: None,
        };
        let name = device_name(&device);
        tracing::debug!(name = %name, "opened cpal output device");
        Ok(Self {
            output: OutputInfo {
                device_name: name,
                mode: OutputMode::Shared,
                format,
            },
            config: supported.into(),
            sample_format,
            device,
            stream: None,
            stream_failed: Arc::new(AtomicBool::new(false)),
            health: Arc::new(AtomicU8::new(HEALTH_OK)),
            latency_frames: Arc::new(AtomicU64::new(0)),
        })
    }

    /// `true` if the backend reported a stream error since the sink started.
    #[must_use]
    pub fn stream_failed(&self) -> bool {
        self.stream_failed.load(Ordering::Relaxed)
    }

    fn build<T>(&self, mut callback: RenderCallback) -> Result<cpal::Stream, SinkError>
    where
        T: SizedSample + FromSample<f32> + 'static,
    {
        let failed = Arc::clone(&self.stream_failed);
        let health = Arc::clone(&self.health);
        let latency = Arc::clone(&self.latency_frames);
        let rate = f64::from(self.output.format.sample_rate);
        let mut scratch = vec![0.0_f32; CONVERT_CHUNK];
        self.device
            .build_output_stream(
                self.config,
                move |data: &mut [T], info: &cpal::OutputCallbackInfo| {
                    // RT-SAFE: atomics, a float multiply and the render callback only.
                    let ts = info.timestamp();
                    let delay = ts.playback.saturating_duration_since(ts.callback);
                    // Truncation to whole frames is intended.
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    latency.store((delay.as_secs_f64() * rate) as u64, Ordering::Relaxed);
                    render_converted(data, &mut scratch, &mut callback);
                },
                move |_err: cpal::Error| {
                    health.store(HEALTH_LOST, Ordering::Relaxed);
                    failed.store(true, Ordering::Relaxed);
                },
                None,
            )
            .map_err(|e| SinkError::Backend(e.to_string()))
    }
}

/// Fill `data` from the render callback, converting `f32` to the device sample type.
///
/// RT-SAFE: works in fixed chunks of the preallocated `scratch`; no allocation.
fn render_converted<T>(data: &mut [T], scratch: &mut [f32], callback: &mut RenderCallback)
where
    T: SizedSample + FromSample<f32>,
{
    for chunk in data.chunks_mut(scratch.len()) {
        let part = &mut scratch[..chunk.len()];
        callback(part);
        for (dst, src) in chunk.iter_mut().zip(part.iter()) {
            *dst = T::from_sample_(*src);
        }
    }
}

impl AudioSink for CpalSink {
    fn output_info(&self) -> &OutputInfo {
        &self.output
    }

    fn start(&mut self, callback: RenderCallback) -> Result<(), SinkError> {
        if self.stream.is_some() {
            return Err(SinkError::AlreadyStarted);
        }
        let stream = match self.sample_format {
            SampleFormat::F32 => self.build_f32(callback)?,
            SampleFormat::I16 => self.build::<i16>(callback)?,
            SampleFormat::I32 => self.build::<i32>(callback)?,
            other => return Err(SinkError::UnsupportedSampleFormat(other.to_string())),
        };
        stream
            .play()
            .map_err(|e| SinkError::Backend(e.to_string()))?;
        self.stream = Some(stream);
        Ok(())
    }

    fn stop(&mut self) {
        self.stream = None;
    }

    fn latency_frames(&self) -> u64 {
        self.latency_frames.load(Ordering::Relaxed)
    }

    fn health(&self) -> SinkHealth {
        match self.health.load(Ordering::Relaxed) {
            HEALTH_LOST => SinkHealth::Lost,
            HEALTH_TAKEN => SinkHealth::Taken,
            _ => SinkHealth::Ok,
        }
    }
}

impl CpalSink {
    /// Build an `f32` stream: the callback writes straight into the device buffer, so the
    /// samples are never converted or copied.
    fn build_f32(&self, mut callback: RenderCallback) -> Result<cpal::Stream, SinkError> {
        let failed = Arc::clone(&self.stream_failed);
        let health = Arc::clone(&self.health);
        let latency = Arc::clone(&self.latency_frames);
        let rate = f64::from(self.output.format.sample_rate);
        self.device
            .build_output_stream(
                self.config,
                move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
                    // RT-SAFE: atomics, a float multiply and the render callback only.
                    let ts = info.timestamp();
                    let delay = ts.playback.saturating_duration_since(ts.callback);
                    // Truncation to whole frames is intended.
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    latency.store((delay.as_secs_f64() * rate) as u64, Ordering::Relaxed);
                    callback(data);
                },
                move |_err: cpal::Error| {
                    health.store(HEALTH_LOST, Ordering::Relaxed);
                    failed.store(true, Ordering::Relaxed);
                },
                None,
            )
            .map_err(|e| SinkError::Backend(e.to_string()))
    }
}

/// Opens [`CpalSink`]s on one output device (the default device for `None`). Shared mode fixes
/// the format, so the player resamples instead of reopening the device.
#[derive(Debug, Clone, Default)]
pub struct CpalSinkFactory {
    device: Option<DeviceId>,
    named: Option<String>,
}

impl CpalSinkFactory {
    /// A factory for the device with the specified [`DeviceId`], or the default device for `None`.
    #[must_use]
    pub fn for_device(device: Option<DeviceId>) -> Self {
        Self {
            device,
            named: None,
        }
    }

    /// A factory for the named device, or the default device for `None`.
    #[must_use]
    pub fn new(device: Option<String>) -> Self {
        Self {
            device: None,
            named: device,
        }
    }
}

impl SinkFactory for CpalSinkFactory {
    fn open(&self, _source: AudioFormat) -> Result<Box<dyn AudioSink>, SinkError> {
        if let Some(ref id) = self.device {
            Ok(Box::new(CpalSink::open_device(Some(id))?))
        } else if let Some(ref name) = self.named {
            Ok(Box::new(CpalSink::open_named(Some(name))?))
        } else {
            Ok(Box::new(CpalSink::open_device(None)?))
        }
    }

    fn list_devices(&self) -> Result<Vec<OutputDevice>, SinkError> {
        list_devices()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires real audio hardware/host"]
    fn test_list_devices_runs() {
        let res = list_devices();
        assert!(res.is_ok(), "list_devices should succeed: {res:?}");
    }

    #[test]
    #[ignore = "requires real audio hardware/host"]
    fn test_list_output_devices_backward_compat() {
        let res = list_output_devices();
        assert!(res.is_ok(), "list_output_devices should succeed: {res:?}");
    }

    #[test]
    fn test_cpal_sink_factory_defaults() {
        let factory = CpalSinkFactory::default();
        assert!(factory.device.is_none());
        assert!(factory.named.is_none());
    }

    #[test]
    fn test_health_constants() {
        assert_eq!(HEALTH_OK, 0);
        assert_eq!(HEALTH_LOST, 1);
        assert_eq!(HEALTH_TAKEN, 2);
    }
}
