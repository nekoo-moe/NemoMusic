//! Device switching tests: seamless switch during playback, no re-resolution, queue
//! preservation, click-free continuity, failure rollback, and switching while stopped.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::fakes::{RigOptions, SelectableSinkFactory, rig, video_id, video_track};
use common::fixtures::{tone_i16, wav_bytes};
use nemo_core::output::{DeviceId, OutputDevice, SavedDevice};
use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent};

const LIMIT: Duration = Duration::from_secs(10);

fn test_devices() -> (OutputDevice, OutputDevice, OutputDevice) {
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
    let dev_fail = OutputDevice {
        id: DeviceId("dev-fail".into()),
        name: "Failing Device".into(),
        is_default: false,
    };
    (dev1, dev2, dev_fail)
}

#[test]
fn switch_device_during_playback_preserves_position_and_queue_without_reresolve() {
    let (dev1, dev2, _dev_fail) = test_devices();
    let capture = Arc::new(Mutex::new(Vec::new()));
    let opened = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(SelectableSinkFactory::new(
        vec![dev1.clone(), dev2.clone()],
        Arc::clone(&capture),
        Arc::clone(&opened),
        Duration::from_millis(1),
    ));

    let s1 = tone_i16(48_000, 2, 10 * 48_000, 440.0);
    let s2 = tone_i16(48_000, 2, 10 * 48_000, 880.0);
    let mut r = rig(
        vec![
            (video_id(1), wav_bytes(&s1, 16, 48_000, 2)),
            (video_id(2), wav_bytes(&s2, 16, 48_000, 2)),
        ],
        RigOptions {
            sink_factory: Some(factory),
            capture: Some(capture),
            opened: Some(opened),
            ..RigOptions::default()
        },
    );

    r.send(PlayerCommand::SetQueue {
        tracks: vec![video_track(1), video_track(2)],
        start: 0,
    });

    // Wait until playback reaches Playing state and audio is captured
    r.wait_event("state playing", LIMIT, |e| {
        matches!(e, PlayerEvent::StateChanged(PlayState::Playing))
    });
    r.wait_samples(480 * 4, LIMIT);

    let resolves_before = r.resolver_calls.load(Ordering::SeqCst);
    let pos_before = r.player.position();

    // Switch device
    r.send(PlayerCommand::SetOutputDevice(Some(SavedDevice {
        id: dev2.id.clone(),
        name: dev2.name.clone(),
    })));

    // Wait for OutputChanged event with dev2 chosen
    r.wait_event("output changed to dev-2", LIMIT, |e| {
        matches!(e, PlayerEvent::OutputChanged(s) if s.chosen.as_ref().map(|d| &d.id) == Some(&dev2.id))
    });

    // Check resolver was not called again
    let resolves_after = r.resolver_calls.load(Ordering::SeqCst);
    assert_eq!(
        resolves_before, resolves_after,
        "decoder must be carried over without resolving again"
    );

    // Queue and state are preserved
    let snap = r.player.snapshot();
    assert_eq!(snap.queue.len(), 2);
    assert_eq!(snap.queue_index, Some(0));
    assert_eq!(snap.state, PlayState::Playing);
    assert_eq!(snap.output.chosen.as_ref().map(|d| &d.id), Some(&dev2.id));
    assert_eq!(snap.output.active.as_ref().map(|d| &d.id), Some(&dev2.id));

    // Position moves forward across the switch
    let pos_after = r.player.position();
    assert!(
        pos_after >= pos_before,
        "position moves forward: before={pos_before:?}, after={pos_after:?}"
    );

    // Audio continues playing on new device
    let samples_now = r.captured().len();
    r.wait_samples(samples_now + 480 * 4, LIMIT);
}

#[test]
fn failed_open_keeps_old_device_and_saved_choice() {
    let (dev1, dev2, dev_fail) = test_devices();
    let capture = Arc::new(Mutex::new(Vec::new()));
    let opened = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(SelectableSinkFactory::new(
        vec![dev1, dev2, dev_fail.clone()],
        Arc::clone(&capture),
        Arc::clone(&opened),
        Duration::from_millis(1),
    ));
    *factory.fail_id.lock().unwrap() = Some(dev_fail.id.clone());

    let s1 = tone_i16(48_000, 2, 10 * 48_000, 440.0);
    let mut r = rig(
        vec![(video_id(1), wav_bytes(&s1, 16, 48_000, 2))],
        RigOptions {
            sink_factory: Some(factory),
            capture: Some(capture),
            opened: Some(opened),
            ..RigOptions::default()
        },
    );

    r.send(PlayerCommand::Play(video_track(1)));
    r.wait_samples(480 * 4, LIMIT);

    // Try to switch to failing device
    r.send(PlayerCommand::SetOutputDevice(Some(SavedDevice {
        id: dev_fail.id.clone(),
        name: dev_fail.name,
    })));

    // OutputOpenFailed is emitted
    r.wait_event("output open failed", LIMIT, |e| {
        matches!(e, PlayerEvent::OutputOpenFailed { .. })
    });

    // Old device and saved choice are kept
    let snap = r.player.snapshot();
    assert_eq!(snap.output.chosen, None);
    assert_eq!(snap.state, PlayState::Playing);
    assert_eq!(snap.queue_index, Some(0));

    // Playback continues uninterrupted on old device
    let samples_now = r.captured().len();
    r.wait_samples(samples_now + 480 * 4, LIMIT);
}

#[test]
fn switch_device_no_click_step() {
    let (dev1, dev2, _dev_fail) = test_devices();
    let capture = Arc::new(Mutex::new(Vec::new()));
    let opened = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(SelectableSinkFactory::new(
        vec![dev1, dev2.clone()],
        Arc::clone(&capture),
        Arc::clone(&opened),
        Duration::from_millis(1),
    ));

    // Continuous 440 Hz sine tone (amplitude ~0.5)
    let s1 = tone_i16(48_000, 2, 10 * 48_000, 440.0);
    let mut r = rig(
        vec![(video_id(1), wav_bytes(&s1, 16, 48_000, 2))],
        RigOptions {
            sink_factory: Some(factory),
            capture: Some(capture),
            opened: Some(opened),
            ..RigOptions::default()
        },
    );

    r.send(PlayerCommand::Play(video_track(1)));
    r.wait_samples(480 * 8, LIMIT);

    // Switch device during continuous tone
    r.send(PlayerCommand::SetOutputDevice(Some(SavedDevice {
        id: dev2.id.clone(),
        name: dev2.name,
    })));

    r.wait_event("output changed", LIMIT, |e| {
        matches!(e, PlayerEvent::OutputChanged(s) if s.chosen.as_ref().map(|d| &d.id) == Some(&dev2.id))
    });

    // Capture more samples across the switch (covering prefill gap + resumption)
    let samples_after = r.captured().len() + 480 * 60;
    r.wait_samples(samples_after, LIMIT);

    let captured = r.captured();
    assert!(captured.len() >= 480 * 10);

    // Find the end of device 1 playback (before the silence gap) and the start of device 2 playback.
    let gap_start = captured
        .iter()
        .position(|s| *s == 0.0)
        .expect("prefill gap between devices");
    let resume = captured[gap_start..]
        .iter()
        .position(|s| *s != 0.0)
        .map(|p| gap_start + p)
        .expect("device 2 resumes playback");

    // Check gap duration: at 48 kHz stereo, gap frames must be <= 500 ms (24000 frames)
    let gap_frames = (resume - gap_start) / 2;
    let gap_ms = gap_frames as u64 * 1000 / 48_000;
    assert!(
        gap_ms <= 500,
        "gap between devices must be <= 500 ms, was {gap_ms} ms"
    );

    // Verify continuity across the switch boundary (dev1 last sample vs dev2 first sample)
    let boundary_step = (captured[resume] - captured[gap_start - 2]).abs();
    assert!(
        boundary_step < 0.1,
        "sample step across switch boundary must be below click threshold (< 0.1), was {boundary_step}"
    );

    // Verify smooth continuity during dev1 playback
    for i in 2..gap_start {
        let diff = (captured[i] - captured[i - 2]).abs();
        assert!(
            diff < 0.1,
            "sample jump too large in dev1 at sample {i}: {diff}"
        );
    }

    // Verify smooth continuity during dev2 playback
    for i in (resume + 2)..captured.len() {
        let diff = (captured[i] - captured[i - 2]).abs();
        assert!(
            diff < 0.1,
            "sample jump too large in dev2 at sample {i}: {diff}"
        );
    }
}

#[test]
fn switch_device_while_stopped() {
    let (dev1, dev2, _dev_fail) = test_devices();
    let capture = Arc::new(Mutex::new(Vec::new()));
    let opened = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(SelectableSinkFactory::new(
        vec![dev1, dev2.clone()],
        Arc::clone(&capture),
        Arc::clone(&opened),
        Duration::from_millis(1),
    ));

    let mut r = rig(
        vec![],
        RigOptions {
            sink_factory: Some(factory),
            capture: Some(capture),
            opened: Some(opened),
            ..RigOptions::default()
        },
    );

    assert_eq!(r.player.snapshot().state, PlayState::Stopped);

    r.send(PlayerCommand::SetOutputDevice(Some(SavedDevice {
        id: dev2.id.clone(),
        name: dev2.name,
    })));

    r.wait_event("output changed while stopped", LIMIT, |e| {
        matches!(e, PlayerEvent::OutputChanged(s) if s.chosen.as_ref().map(|d| &d.id) == Some(&dev2.id))
    });

    let snap = r.player.snapshot();
    assert_eq!(snap.output.chosen.as_ref().map(|d| &d.id), Some(&dev2.id));
    assert_eq!(snap.state, PlayState::Stopped);
}
