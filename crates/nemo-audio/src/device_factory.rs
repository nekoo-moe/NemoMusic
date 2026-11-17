//! The output-device factory for shared mode and device switching.
//!
//! [`DeviceSinkFactory`] holds the [`OutputSelection`] that the player selects and opens that
//! device in shared mode. If the chosen device is absent, it falls back to the system default
//! device and marks `saved_missing = true`. Shared mode fixes the format and is never reported
//! as bit-perfect.
//!
//! Device names and identifiers are never logged at `info` level or above (Criterion 26).

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use nemo_core::audio::AudioFormat;
use nemo_core::output::{DeviceId, OutputDevice, OutputOptions};

use crate::cpal_sink::{CpalSink, list_devices};
use crate::sink::{AudioSink, OpenedSink, OutputSelection, SinkError, SinkFactory};

/// Custom device opener for tests.
pub type DeviceOpener =
    Box<dyn Fn(Option<&DeviceId>) -> Result<Box<dyn AudioSink>, SinkError> + Send + Sync>;

/// Opens output sinks on the selected audio device in shared mode.
pub struct DeviceSinkFactory {
    selection: Mutex<OutputSelection>,
    saved_missing: AtomicBool,
    opener: Option<DeviceOpener>,
}

impl DeviceSinkFactory {
    /// Create a factory with the specified output selection.
    #[must_use]
    pub fn new(selection: OutputSelection) -> Self {
        Self {
            selection: Mutex::new(selection),
            saved_missing: AtomicBool::new(false),
            opener: None,
        }
    }

    /// Create a factory for the device with the specified [`DeviceId`], or default device for `None`.
    #[must_use]
    pub fn for_device(device: Option<DeviceId>) -> Self {
        Self::new(OutputSelection {
            device,
            options: OutputOptions::default(),
        })
    }

    /// Create a factory with a custom device opener (for tests).
    #[must_use]
    pub fn with_opener(selection: OutputSelection, opener: DeviceOpener) -> Self {
        Self {
            selection: Mutex::new(selection),
            saved_missing: AtomicBool::new(false),
            opener: Some(opener),
        }
    }

    /// The selection the factory will open next.
    #[must_use]
    pub fn selection(&self) -> OutputSelection {
        self.selection
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Whether the last open had to fall back to the default device because the chosen device
    /// was absent.
    #[must_use]
    pub fn saved_missing(&self) -> bool {
        self.saved_missing.load(Ordering::Relaxed)
    }

    /// Convenience alias for [`saved_missing`](Self::saved_missing).
    #[must_use]
    pub fn is_saved_missing(&self) -> bool {
        self.saved_missing()
    }

    fn open_internal(&self, id: Option<&DeviceId>) -> Result<Box<dyn AudioSink>, SinkError> {
        if let Some(ref opener) = self.opener {
            opener(id)
        } else {
            let sink = CpalSink::open_device(id)?;
            Ok(Box::new(sink))
        }
    }
}

impl Default for DeviceSinkFactory {
    fn default() -> Self {
        Self::new(OutputSelection::default())
    }
}

impl fmt::Debug for DeviceSinkFactory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceSinkFactory")
            .field("selection", &self.selection)
            .field("saved_missing", &self.saved_missing)
            .finish_non_exhaustive()
    }
}

impl SinkFactory for DeviceSinkFactory {
    /// Open the selected device in shared mode.
    ///
    /// If no device is chosen, the system default device is opened. If a chosen device is
    /// absent, the factory opens the system default device and sets `saved_missing = true`.
    ///
    /// # Errors
    ///
    /// Returns [`SinkError`] when the device or the fallback default device cannot be opened.
    fn open(&self, _source: AudioFormat) -> Result<Box<dyn AudioSink>, SinkError> {
        let selection = self
            .selection
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();

        match selection.device {
            None => {
                tracing::debug!("opening default output device in shared mode");
                let sink = self.open_internal(None)?;
                self.saved_missing.store(false, Ordering::Relaxed);
                Ok(sink)
            }
            Some(ref wanted) => {
                tracing::debug!(device = ?wanted, "opening chosen output device in shared mode");
                match self.open_internal(Some(wanted)) {
                    Ok(sink) => {
                        self.saved_missing.store(false, Ordering::Relaxed);
                        Ok(sink)
                    }
                    Err(SinkError::DeviceNotFound(_)) => {
                        tracing::debug!(
                            device = ?wanted,
                            "chosen output device absent; falling back to system default"
                        );
                        let sink = self.open_internal(None)?;
                        self.saved_missing.store(true, Ordering::Relaxed);
                        Ok(sink)
                    }
                    Err(err) => Err(err),
                }
            }
        }
    }

    /// Shared mode fixes the format and resamples rather than reopening the device.
    fn can_follow_source(&self) -> bool {
        false
    }

    /// Open the sink and report fallback status (always `None` in shared mode).
    ///
    /// # Errors
    ///
    /// Returns [`SinkError`] when the device cannot be opened.
    fn open_reported(&self, source: AudioFormat) -> Result<OpenedSink, SinkError> {
        let sink = self.open(source)?;
        Ok(OpenedSink {
            sink,
            fallback: None,
        })
    }

    /// Update the output selection for the next open.
    fn select(&self, selection: OutputSelection) {
        *self
            .selection
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = selection;
        self.saved_missing.store(false, Ordering::Relaxed);
    }

    /// List the available output devices.
    ///
    /// # Errors
    ///
    /// Returns [`SinkError::Backend`] when the audio host fails to enumerate devices.
    fn list_devices(&self) -> Result<Vec<OutputDevice>, SinkError> {
        list_devices()
    }

    fn is_saved_missing(&self) -> bool {
        self.saved_missing()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;

    use super::*;
    use crate::sink::CaptureSink;

    fn test_format() -> AudioFormat {
        AudioFormat {
            sample_rate: 44100,
            channels: 2,
            bit_depth: Some(16),
        }
    }

    #[test]
    fn factory_defaults_are_unselected_and_not_missing() {
        let factory = DeviceSinkFactory::default();
        let selection = factory.selection();
        assert_eq!(selection.device, None);
        assert_eq!(selection.options, OutputOptions::default());
        assert!(!factory.saved_missing());
        assert!(!factory.is_saved_missing());
        assert!(!factory.can_follow_source());
    }

    #[test]
    fn factory_for_device_initializes_selection() {
        let id = DeviceId("test-device-id".to_owned());
        let factory = DeviceSinkFactory::for_device(Some(id.clone()));
        assert_eq!(factory.selection().device, Some(id));
        assert!(!factory.saved_missing());
    }

    #[test]
    fn select_updates_selection_and_resets_saved_missing() {
        let factory = DeviceSinkFactory::default();
        factory.saved_missing.store(true, Ordering::Relaxed);
        assert!(factory.saved_missing());

        let new_id = DeviceId("speakers".to_owned());
        factory.select(OutputSelection {
            device: Some(new_id.clone()),
            options: OutputOptions::default(),
        });
        assert_eq!(factory.selection().device, Some(new_id));
        assert!(!factory.saved_missing());
    }

    #[test]
    fn open_default_device_via_custom_opener() {
        let called = Arc::new(AtomicUsize::new(0));
        let called_clone = Arc::clone(&called);
        let opener: DeviceOpener = Box::new(move |id| {
            assert!(id.is_none());
            called_clone.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(CaptureSink::new(test_format())))
        });

        let factory = DeviceSinkFactory::with_opener(OutputSelection::default(), opener);
        let sink = factory.open(test_format()).expect("open should succeed");
        assert_eq!(sink.output_info().format, test_format());
        assert_eq!(called.load(Ordering::SeqCst), 1);
        assert!(!factory.saved_missing());
    }

    #[test]
    fn open_chosen_device_that_exists() {
        let dev_id = DeviceId("headphones-1".to_owned());
        let dev_id_clone = dev_id.clone();
        let called = Arc::new(AtomicUsize::new(0));
        let called_clone = Arc::clone(&called);
        let opener: DeviceOpener = Box::new(move |id| {
            assert_eq!(id, Some(&dev_id_clone));
            called_clone.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(CaptureSink::new(test_format())))
        });

        let factory = DeviceSinkFactory::with_opener(
            OutputSelection {
                device: Some(dev_id),
                options: OutputOptions::default(),
            },
            opener,
        );
        let reported = factory
            .open_reported(test_format())
            .expect("open should succeed");
        assert!(reported.fallback.is_none());
        assert_eq!(called.load(Ordering::SeqCst), 1);
        assert!(!factory.saved_missing());
    }

    #[test]
    fn open_chosen_device_absent_falls_back_to_default_and_sets_saved_missing() {
        let dev_id = DeviceId("missing-usb-dac".to_owned());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let calls_clone = Arc::clone(&calls);
        let opener: DeviceOpener = Box::new(move |id| {
            let mut guard = calls_clone.lock().unwrap();
            guard.push(id.cloned());
            if let Some(wanted) = id {
                Err(SinkError::DeviceNotFound(wanted.0.clone()))
            } else {
                Ok(Box::new(CaptureSink::new(test_format())))
            }
        });

        let factory = DeviceSinkFactory::with_opener(
            OutputSelection {
                device: Some(dev_id.clone()),
                options: OutputOptions::default(),
            },
            opener,
        );

        let reported = factory
            .open_reported(test_format())
            .expect("fallback to default should succeed");
        assert!(reported.fallback.is_none());
        assert!(factory.saved_missing());

        let recorded = calls.lock().unwrap().clone();
        assert_eq!(recorded, vec![Some(dev_id), None]);
    }

    #[test]
    fn open_propagates_other_errors() {
        let opener: DeviceOpener = Box::new(|_| Err(SinkError::AlreadyStarted));
        let factory = DeviceSinkFactory::with_opener(OutputSelection::default(), opener);
        let res = factory.open(test_format());
        assert!(matches!(res, Err(SinkError::AlreadyStarted)));
    }
}
