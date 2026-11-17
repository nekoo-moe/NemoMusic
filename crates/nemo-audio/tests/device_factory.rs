//! Integration tests for [`DeviceSinkFactory`].
//!
//! Checks device selection, fallback when a chosen device is absent, and asserts that no device
//! name or identifier is logged at `info` level or above (Criterion 26).

use std::sync::{Arc, Mutex};

use nemo_audio::sink::OutputSelection;
use nemo_audio::sink::{CaptureSink, DeviceOpener, DeviceSinkFactory, SinkError, SinkFactory};
use nemo_core::audio::AudioFormat;
use nemo_core::output::{DeviceId, OutputOptions};
use tracing::Level;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::*;

fn test_format() -> AudioFormat {
    AudioFormat {
        sample_rate: 48000,
        channels: 2,
        bit_depth: Some(16),
    }
}

#[derive(Default, Clone)]
struct LogCapture {
    entries: Arc<Mutex<Vec<(Level, String)>>>,
}

struct FieldVisitor<'a>(&'a mut String);

impl tracing::field::Visit for FieldVisitor<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        use std::fmt::Write;
        let _ = write!(self.0, " {}={:?}", field.name(), value);
    }
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        use std::fmt::Write;
        let _ = write!(self.0, " {}={}", field.name(), value);
    }
}

impl<S: tracing::Subscriber> Layer<S> for LogCapture {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut msg = format!("{}: ", event.metadata().target());
        let mut visitor = FieldVisitor(&mut msg);
        event.record(&mut visitor);
        if let Ok(mut entries) = self.entries.lock() {
            entries.push((*event.metadata().level(), msg));
        }
    }
}

#[test]
fn criterion_26_no_device_name_or_id_logged_at_info_or_above() {
    let capture = LogCapture::default();
    let subscriber = tracing_subscriber::registry().with(capture.clone());

    let canary_id_str = "CanaryDeviceId-Secret-12345";
    let canary_name_str = "CanaryHeadphonesSuperSecret";
    let canary_id = DeviceId(canary_id_str.to_owned());

    tracing::subscriber::with_default(subscriber, || {
        let canary_clone = canary_id.clone();
        let opener: DeviceOpener = Box::new(move |id| {
            if id == Some(&canary_clone) {
                Err(SinkError::DeviceNotFound(canary_clone.0.clone()))
            } else {
                Ok(Box::new(CaptureSink::new(test_format())))
            }
        });

        let factory = DeviceSinkFactory::with_opener(
            OutputSelection {
                device: Some(canary_id.clone()),
                options: OutputOptions::default(),
            },
            opener,
        );

        // Open with absent device; triggers fallback to default and sets saved_missing
        let reported = factory
            .open_reported(test_format())
            .expect("should fall back to default");
        assert!(reported.fallback.is_none());
        assert!(factory.saved_missing());

        // Select a new device
        factory.select(OutputSelection {
            device: Some(DeviceId("CanaryDevice2".to_owned())),
            options: OutputOptions::default(),
        });
        assert!(!factory.saved_missing());
    });

    let entries = capture.entries.lock().expect("lock log capture").clone();
    assert!(
        !entries.is_empty(),
        "expected debug logs from factory operations"
    );

    let mut debug_count = 0;
    for (level, msg) in entries {
        if level <= Level::INFO {
            assert!(
                !msg.contains(canary_id_str),
                "device ID leaked at level {level:?}: {msg}"
            );
            assert!(
                !msg.contains(canary_name_str),
                "device name leaked at level {level:?}: {msg}"
            );
        } else if level == Level::DEBUG {
            debug_count += 1;
            // Even at debug level, DeviceId's Debug format hashes the ID, so the raw string does not appear.
            assert!(
                !msg.contains(canary_id_str),
                "raw device ID leaked at debug level: {msg}"
            );
        }
    }
    assert!(
        debug_count > 0,
        "expected at least one debug log from device factory"
    );
}

#[test]
fn device_factory_open_reported_wraps_sink_without_fallback() {
    let opener: DeviceOpener = Box::new(|_| Ok(Box::new(CaptureSink::new(test_format()))));
    let factory = DeviceSinkFactory::with_opener(OutputSelection::default(), opener);

    let reported = factory
        .open_reported(test_format())
        .expect("open_reported succeeds");
    assert!(reported.fallback.is_none());
    assert_eq!(reported.sink.output_info().format, test_format());
    assert!(!factory.can_follow_source());
}

#[test]
#[ignore = "requires real audio hardware/host"]
fn real_hardware_device_factory_open() {
    let factory = DeviceSinkFactory::default();
    let res = factory.open(test_format());
    // May succeed or return NoDevice on machine without endpoints
    if let Ok(sink) = res {
        assert_eq!(
            sink.output_info().mode,
            nemo_core::audio::OutputMode::Shared
        );
    }
}
