//! Integration test verifying CPAL device listing.

use nemo_audio::sink::list_devices;

#[test]
#[ignore = "requires real audio hardware/host"]
fn list_real_cpal_devices() {
    let devices = list_devices().expect("listing devices should succeed on an audio host");
    for d in &devices {
        assert!(!d.name.is_empty(), "device name should not be empty");
        assert!(!d.id.0.is_empty(), "device ID should not be empty");
    }
}
