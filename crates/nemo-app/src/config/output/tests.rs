//! Tests for the stored output device and options.

use std::path::PathBuf;

use super::*;
use crate::config::{CONFIG_FILE, Settings};

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nemo-out-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir); // A leftover from an earlier run is fine to drop.
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

fn device(id: &str, name: &str) -> OutputDevice {
    OutputDevice {
        id: DeviceId(id.to_owned()),
        name: name.to_owned(),
        is_default: false,
    }
}

#[test]
fn the_device_and_options_round_trip() {
    let root = temp_root("roundtrip");
    let s = Settings {
        output_device: Some(StoredDevice::from(SavedDevice {
            id: DeviceId("{0.0.0.00000000}.{dac}".to_owned()),
            name: "USB DAC".to_owned(),
        })),
        output_options: StoredOutputOptions {
            exclusive: true,
            idle_release_secs: 30,
            exclusive_buffer_ms: 10,
            auto_resume: true,
        },
        ..Settings::default()
    };
    s.save(&root).expect("save");
    let loaded = Settings::load(&root);
    assert_eq!(loaded, s);
    let saved = loaded.output_device.and_then(|d| d.saved()).expect("saved");
    assert_eq!(saved.id, DeviceId("{0.0.0.00000000}.{dac}".to_owned()));
    assert_eq!(saved.name, "USB DAC");
    assert!(loaded.output_options.to_core().exclusive);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn an_old_plain_name_loads_is_matched_once_and_is_saved_with_the_id() {
    let root = temp_root("upgrade");
    std::fs::write(root.join(CONFIG_FILE), r#"{"output_device": "USB DAC"}"#).expect("write");
    let mut s = Settings::load(&root);
    let stored = s.output_device.as_mut().expect("device");
    assert_eq!(stored.name, "USB DAC");
    assert!(stored.id.is_empty() && stored.saved().is_none());
    let devices = [
        device("speakers-id", "Speakers"),
        device("dac-id", "USB DAC"),
    ];
    assert!(stored.upgrade(&devices));
    assert_eq!(stored.id, "dac-id");
    // Once matched, the ID holds even if another device takes the name later.
    assert!(!stored.upgrade(&[device("other-id", "USB DAC")]));
    s.save(&root).expect("save");
    let reloaded = Settings::load(&root).output_device.expect("device");
    assert_eq!(
        reloaded,
        StoredDevice {
            id: "dac-id".to_owned(),
            name: "USB DAC".to_owned(),
        }
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn a_name_that_matches_no_device_or_several_stays_unmatched() {
    let mut stored = StoredDevice {
        id: String::new(),
        name: "Headphones".to_owned(),
    };
    assert!(!stored.upgrade(&[device("a", "Speakers")]));
    assert!(!stored.upgrade(&[device("a", "Headphones"), device("b", "Headphones")]));
    assert!(stored.id.is_empty());
}

#[test]
fn an_old_file_has_no_device_and_shared_mode() {
    let root = temp_root("old");
    std::fs::write(root.join(CONFIG_FILE), r#"{"volume_percent": 40}"#).expect("write");
    let s = Settings::load(&root);
    assert_eq!(s.output_device, None);
    assert_eq!(s.output_options.to_core(), OutputOptions::default());
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn out_of_range_options_are_clamped() {
    let root = temp_root("clamp");
    std::fs::write(
        root.join(CONFIG_FILE),
        r#"{"output_options": {"idle_release_secs": 0, "exclusive_buffer_ms": 250}}"#,
    )
    .expect("write");
    let o = Settings::load(&root).output_options;
    assert_eq!(
        (o.idle_release_secs, o.exclusive_buffer_ms),
        (
            OutputOptions::IDLE_RELEASE_MIN,
            OutputOptions::BUFFER_MAX_MS
        )
    );
    assert!(!o.exclusive && !o.auto_resume);
    std::fs::write(
        root.join(CONFIG_FILE),
        r#"{"output_options": {"idle_release_secs": 200, "exclusive_buffer_ms": 1}}"#,
    )
    .expect("write");
    let o = Settings::load(&root).output_options;
    assert_eq!(
        (o.idle_release_secs, o.exclusive_buffer_ms),
        (
            OutputOptions::IDLE_RELEASE_MAX,
            OutputOptions::BUFFER_MIN_MS
        )
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}
