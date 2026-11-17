//! Device watcher integration tests: polling for device additions and removals,
//! following system default when no device is chosen, returning saved device handling
//! (switch at once when stopped, offer when playing), error recovery and manual refresh.

mod common;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::fakes::{RigOptions, SelectableSinkFactory, rig, video_id, video_track};
use common::fixtures::{tone_i16, wav_bytes};
use nemo_audio::device_watch::DeviceEnumerator;
use nemo_core::output::{DeviceId, OutputDevice, SavedDevice};
use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent};

const LIMIT: Duration = Duration::from_secs(10);

fn test_devices() -> (OutputDevice, OutputDevice) {
    let dev1 = OutputDevice {
        id: DeviceId("dev-1".into()),
        name: "Device One".into(),
        is_default: true,
    };
    let dev2 = OutputDevice {
        id: DeviceId("dev-2".into()),
        name: "Device Two".into(),
        is_default: false,
    };
    (dev1, dev2)
}

#[test]
fn device_added_and_removed_updates_snapshot_and_emits_event() {
    let (dev1, dev2) = test_devices();
    let capture = Arc::new(Mutex::new(Vec::new()));
    let opened = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(SelectableSinkFactory::new(
        vec![dev1],
        Arc::clone(&capture),
        Arc::clone(&opened),
        Duration::from_millis(1),
    ));

    let mut r = rig(
        vec![],
        RigOptions {
            sink_factory: Some(factory.clone()),
            device_watch_interval: Some(Duration::from_millis(10)),
            ..RigOptions::default()
        },
    );

    // Initial snapshot has dev1
    r.wait_event(
        "initial devices",
        LIMIT,
        |e| matches!(e, PlayerEvent::OutputDevicesChanged(d) if d.len() == 1),
    );
    assert_eq!(r.player.snapshot().output.devices.len(), 1);

    // Add dev2
    {
        let mut list = factory.devices.lock().unwrap();
        list.push(dev2);
    }

    // Wait for event reporting 2 devices
    r.wait_event(
        "device added",
        LIMIT,
        |e| matches!(e, PlayerEvent::OutputDevicesChanged(d) if d.len() == 2),
    );
    assert_eq!(r.player.snapshot().output.devices.len(), 2);

    // Remove dev2
    r.seen.clear();
    {
        let mut list = factory.devices.lock().unwrap();
        list.pop();
    }

    // Wait for event reporting 1 device
    r.wait_event(
        "device removed",
        LIMIT,
        |e| matches!(e, PlayerEvent::OutputDevicesChanged(d) if d.len() == 1),
    );
    assert_eq!(r.player.snapshot().output.devices.len(), 1);
}

#[test]
fn default_device_change_switches_playback_when_no_device_chosen() {
    let (dev1, dev2) = test_devices();
    let capture = Arc::new(Mutex::new(Vec::new()));
    let opened = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(SelectableSinkFactory::new(
        vec![dev1.clone(), dev2.clone()],
        Arc::clone(&capture),
        Arc::clone(&opened),
        Duration::from_millis(1),
    ));

    let s1 = tone_i16(48_000, 2, 10 * 48_000, 440.0);
    let mut r = rig(
        vec![(video_id(1), wav_bytes(&s1, 16, 48_000, 2))],
        RigOptions {
            sink_factory: Some(factory.clone()),
            capture: Some(Arc::clone(&capture)),
            opened: Some(Arc::clone(&opened)),
            device_watch_interval: Some(Duration::from_millis(10)),
            ..RigOptions::default()
        },
    );

    r.send(PlayerCommand::Play(video_track(1)));

    r.wait_event("playing", LIMIT, |e| {
        matches!(e, PlayerEvent::StateChanged(PlayState::Playing))
    });
    r.wait_samples(480 * 4, LIMIT);

    // Currently playing on dev1 (default)
    let snap = r.player.snapshot();
    assert_eq!(snap.output.chosen, None);
    assert_eq!(snap.output.active.as_ref().map(|d| &d.id), Some(&dev1.id));

    // Change default to dev2
    {
        let mut list = factory.devices.lock().unwrap();
        list[0].is_default = false;
        list[1].is_default = true;
    }

    // Player detects default change and switches output to dev2
    r.wait_event("output switched to new default", LIMIT, |e| {
        matches!(e, PlayerEvent::OutputChanged(s) if s.active.as_ref().map(|d| &d.id) == Some(&dev2.id))
    });

    let snap = r.player.snapshot();
    assert_eq!(snap.output.active.as_ref().map(|d| &d.id), Some(&dev2.id));
    assert_eq!(snap.output.chosen, None);
    assert_eq!(snap.state, PlayState::Playing);
}

#[test]
fn saved_device_return_while_stopped_switches_at_once() {
    let (dev1, dev2) = test_devices();
    let capture = Arc::new(Mutex::new(Vec::new()));
    let opened = Arc::new(AtomicUsize::new(0));
    // Start with dev1 only, so dev2 is absent
    let factory = Arc::new(SelectableSinkFactory::new(
        vec![dev1.clone()],
        Arc::clone(&capture),
        Arc::clone(&opened),
        Duration::from_millis(1),
    ));

    let mut r = rig(
        vec![],
        RigOptions {
            sink_factory: Some(factory.clone()),
            device_watch_interval: Some(Duration::from_millis(10)),
            ..RigOptions::default()
        },
    );

    // User chooses dev2, which is currently absent
    r.send(PlayerCommand::SetOutputDevice(Some(SavedDevice {
        id: dev2.id.clone(),
        name: dev2.name.clone(),
    })));

    r.wait_event(
        "saved missing",
        LIMIT,
        |e| matches!(e, PlayerEvent::OutputChanged(s) if s.saved_missing),
    );
    let snap = r.player.snapshot();
    assert!(snap.output.saved_missing);
    assert_eq!(snap.output.chosen.as_ref().map(|d| &d.id), Some(&dev2.id));
    assert_eq!(snap.output.active.as_ref().map(|d| &d.id), Some(&dev1.id));
    assert_eq!(snap.state, PlayState::Stopped);

    // dev2 returns while stopped
    {
        let mut list = factory.devices.lock().unwrap();
        list.push(dev2.clone());
    }

    // Player automatically switches to dev2 at once
    r.wait_event("switched to returning saved device", LIMIT, |e| {
        matches!(e, PlayerEvent::OutputChanged(s) if !s.saved_missing && s.active.as_ref().map(|d| &d.id) == Some(&dev2.id))
    });

    let snap = r.player.snapshot();
    assert!(!snap.output.saved_missing);
    assert_eq!(snap.output.chosen.as_ref().map(|d| &d.id), Some(&dev2.id));
    assert_eq!(snap.output.active.as_ref().map(|d| &d.id), Some(&dev2.id));
}

#[test]
fn saved_device_return_while_playing_emits_saved_device_returned() {
    let (dev1, dev2) = test_devices();
    let capture = Arc::new(Mutex::new(Vec::new()));
    let opened = Arc::new(AtomicUsize::new(0));
    // Start with dev1 only
    let factory = Arc::new(SelectableSinkFactory::new(
        vec![dev1.clone()],
        Arc::clone(&capture),
        Arc::clone(&opened),
        Duration::from_millis(1),
    ));

    let s1 = tone_i16(48_000, 2, 10 * 48_000, 440.0);
    let mut r = rig(
        vec![(video_id(1), wav_bytes(&s1, 16, 48_000, 2))],
        RigOptions {
            sink_factory: Some(factory.clone()),
            capture: Some(Arc::clone(&capture)),
            opened: Some(Arc::clone(&opened)),
            device_watch_interval: Some(Duration::from_millis(10)),
            ..RigOptions::default()
        },
    );

    // User chooses dev2, absent
    r.send(PlayerCommand::SetOutputDevice(Some(SavedDevice {
        id: dev2.id.clone(),
        name: dev2.name.clone(),
    })));

    r.wait_event(
        "saved missing",
        LIMIT,
        |e| matches!(e, PlayerEvent::OutputChanged(s) if s.saved_missing),
    );

    // Start playing
    r.send(PlayerCommand::Play(video_track(1)));
    r.wait_event("playing", LIMIT, |e| {
        matches!(e, PlayerEvent::StateChanged(PlayState::Playing))
    });
    r.wait_samples(480 * 4, LIMIT);

    // dev2 returns while playing
    {
        let mut list = factory.devices.lock().unwrap();
        list.push(dev2.clone());
    }

    // Player emits SavedDeviceReturned and OutputChanged with saved_missing = false,
    // but DOES NOT interrupt or switch active playback automatically
    r.wait_event("saved device returned offer", LIMIT, |e| {
        matches!(e, PlayerEvent::SavedDeviceReturned)
    });

    let snap = r.player.snapshot();
    assert!(!snap.output.saved_missing);
    assert_eq!(snap.output.chosen.as_ref().map(|d| &d.id), Some(&dev2.id));
    assert_eq!(snap.output.active.as_ref().map(|d| &d.id), Some(&dev1.id));
    assert_eq!(snap.state, PlayState::Playing);
}

#[test]
fn enumeration_failure_does_not_panic_and_manual_refresh_works() {
    let (dev1, dev2) = test_devices();
    let fails = Arc::new(AtomicBool::new(true));
    let fails_clone = Arc::clone(&fails);
    let current_devs = Arc::new(Mutex::new(vec![dev1]));
    let devs_clone = Arc::clone(&current_devs);

    let enumerator: Arc<dyn DeviceEnumerator> = Arc::new(move || {
        if fails_clone.load(Ordering::SeqCst) {
            Err("mock cpal enumeration error".into())
        } else {
            Ok(devs_clone.lock().unwrap().clone())
        }
    });

    let mut r = rig(
        vec![],
        RigOptions {
            device_enumerator: Some(enumerator),
            device_watch_interval: Some(Duration::from_millis(10)),
            ..RigOptions::default()
        },
    );

    // Wait a short time while failing: player is still alive and didn't panic
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(r.player.snapshot().state, PlayState::Stopped);

    // Recover enumerator with dev1 and dev2
    {
        let mut list = current_devs.lock().unwrap();
        list.push(dev2);
    }
    fails.store(false, Ordering::SeqCst);

    // Watcher will detect it
    r.wait_event(
        "watcher recovered",
        LIMIT,
        |e| matches!(e, PlayerEvent::OutputDevicesChanged(d) if d.len() == 2),
    );

    // Also verify manual refresh via ListOutputDevices
    r.seen.clear();
    r.send(PlayerCommand::ListOutputDevices);
    r.wait_event(
        "manual list output devices",
        LIMIT,
        |e| matches!(e, PlayerEvent::OutputDevicesChanged(d) if d.len() == 2),
    );
}
