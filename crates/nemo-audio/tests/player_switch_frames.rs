//! Device switch continuity, read as frame numbers. The track is a counter (frame `n` holds the
//! value `n + 1`), so the capture of both devices tells exactly which decoded frames each one
//! played: a repeat or a hole across the switch is an extra run, whatever the timing.

mod common;

use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::fakes::{BLOCK, Rig, RigOptions, SelectableSinkFactory, rig, video_id, video_track};
use common::fixtures::{FrameRun, counter_runs, counter_wav};
use nemo_core::output::{DeviceId, OutputDevice, SavedDevice};
use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent};

const LIMIT: Duration = Duration::from_secs(10);
const RATE: usize = 48_000;
/// 500 ms of frames: the longest silence allowed between the two devices.
const MAX_GAP_FRAMES: usize = RATE / 2;

/// Counter tracks of the given lengths in frames (video ids 1, 2, ...) on a player with two
/// devices. `interval` is the real time of one callback of `block` frames.
fn setup(frames: &[usize], interval: Duration, block: usize) -> (Rig, OutputDevice) {
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
    let capture = Arc::new(Mutex::new(Vec::new()));
    let opened = Arc::new(AtomicUsize::new(0));
    let mut factory = SelectableSinkFactory::new(
        vec![dev1, dev2.clone()],
        Arc::clone(&capture),
        Arc::clone(&opened),
        interval,
    );
    factory.block = block;
    let streams = frames
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let id = u32::try_from(i + 1).expect("invariant: few tracks");
            (video_id(id), counter_wav(48_000, *n))
        })
        .collect();
    let r = rig(
        streams,
        RigOptions {
            sink_factory: Some(Arc::new(factory)),
            capture: Some(capture),
            opened: Some(opened),
            ..RigOptions::default()
        },
    );
    let tracks = (1..=frames.len())
        .map(|i| video_track(u32::try_from(i).expect("invariant: few tracks")))
        .collect();
    r.send(PlayerCommand::SetQueue { tracks, start: 0 });
    (r, dev2)
}

/// Frames in the capture so far, silence included.
fn captured_frames(r: &Rig) -> usize {
    r.captured().len() / 2
}

/// Wait until the capture holds `frames` frames.
fn wait_frames(r: &mut Rig, frames: usize) {
    r.wait_samples(frames * 2, LIMIT);
}

/// Switch to `dev2` and wait until the player says it did.
fn switch_to(r: &mut Rig, dev2: &OutputDevice) {
    r.send(PlayerCommand::SetOutputDevice(Some(SavedDevice {
        id: dev2.id.clone(),
        name: dev2.name.clone(),
    })));
    let id = dev2.id.clone();
    r.wait_event("output changed", LIMIT, move |e| {
        matches!(e, PlayerEvent::OutputChanged(s) if s.chosen.as_ref().map(|d| &d.id) == Some(&id))
    });
}

/// The one join of a seamless switch: two runs, the second starting at the frame after the first
/// ended, with at most 500 ms of silence between them.
fn assert_seamless(runs: &[FrameRun]) {
    assert_eq!(
        runs.len(),
        2,
        "one run per device, no hole or repeat: {runs:?}"
    );
    assert_eq!(
        runs[1].first,
        runs[0].last + 1,
        "the new device must continue at the next frame: {runs:?}"
    );
    assert!(
        runs[1].silence_before <= MAX_GAP_FRAMES,
        "silence between the devices must be at most 500 ms: {runs:?}"
    );
}

/// Switch a counter track after `lead` frames and read back what both devices played.
fn switch_and_read(interval: Duration, block: usize, lead: usize) -> Vec<FrameRun> {
    let (mut r, dev2) = setup(&[10 * RATE], interval, block);
    wait_frames(&mut r, lead);
    switch_to(&mut r, &dev2);
    let after = captured_frames(&r) + 60 * BLOCK;
    wait_frames(&mut r, after);
    counter_runs(&r.captured())
}

#[test]
fn switch_device_repeats_and_skips_no_frame() {
    assert_seamless(&switch_and_read(Duration::from_millis(1), BLOCK, 8 * BLOCK));
}

#[test]
fn switch_device_right_after_the_song_starts_repeats_no_frame() {
    // The first frames are out before the control thread has read the song's start report.
    assert_seamless(&switch_and_read(Duration::from_millis(1), BLOCK, BLOCK));
}

#[test]
fn switch_device_with_a_fast_old_device_repeats_no_frame() {
    // The old device pulls 20 blocks in the 2 ms a stopping feeder can take.
    assert_seamless(&switch_and_read(
        Duration::from_micros(100),
        BLOCK,
        8 * BLOCK,
    ));
}

#[test]
fn switch_device_on_a_block_size_that_is_no_frame_multiple_repeats_no_frame() {
    // 441 frames is not a whole number of microseconds, so the position falls between frames.
    assert_seamless(&switch_and_read(Duration::from_millis(1), 441, 9 * 441));
}

#[test]
fn switch_device_in_the_last_second_plays_the_end_and_the_next_song_from_its_start() {
    // 2 times real time: the 3 s song has been decoded to its end when 2.3 s have played.
    let (mut r, dev2) = setup(&[3 * RATE, 2 * RATE], Duration::from_millis(5), BLOCK);
    wait_frames(&mut r, 115_200);
    switch_to(&mut r, &dev2);
    r.wait_event("stopped at the end of the queue", LIMIT, |e| {
        matches!(e, PlayerEvent::StateChanged(PlayState::Stopped))
    });
    let runs = counter_runs(&r.captured());
    assert_eq!(runs.len(), 3, "first song, its end, next song: {runs:?}");
    assert_eq!(runs[1].first, runs[0].last + 1, "{runs:?}");
    assert_eq!(
        runs[1].last,
        i64::try_from(3 * RATE - 1).expect("small"),
        "{runs:?}"
    );
    assert!(runs[1].silence_before <= MAX_GAP_FRAMES, "{runs:?}");
    assert_eq!(
        runs[2].first, 0,
        "the next song starts at its start: {runs:?}"
    );
    assert_eq!(
        runs[2].last,
        i64::try_from(2 * RATE - 1).expect("small"),
        "{runs:?}"
    );
}

#[test]
fn switch_device_while_paused_in_the_last_second_stays_paused_and_resumes_the_end() {
    let (mut r, dev2) = setup(&[2 * RATE], Duration::from_millis(5), BLOCK);
    wait_frames(&mut r, 72_000);
    r.send(PlayerCommand::Pause);
    r.wait_event("paused", LIMIT, |e| {
        matches!(e, PlayerEvent::StateChanged(PlayState::Paused))
    });
    // The feeder writes the rest of the song into the ring while the device is paused.
    std::thread::sleep(Duration::from_millis(100));
    switch_to(&mut r, &dev2);
    assert_eq!(r.player.snapshot().state, PlayState::Paused);
    let heard = counter_runs(&r.captured());
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        counter_runs(&r.captured()),
        heard,
        "nothing plays while paused"
    );
    r.send(PlayerCommand::Resume);
    r.wait_event("stopped at the end", LIMIT, |e| {
        matches!(e, PlayerEvent::StateChanged(PlayState::Stopped))
    });
    let runs = counter_runs(&r.captured());
    assert_eq!(runs.len(), 2, "{runs:?}");
    assert_eq!(runs[1].first, runs[0].last + 1, "{runs:?}");
    assert_eq!(
        runs[1].last,
        i64::try_from(2 * RATE - 1).expect("small"),
        "{runs:?}"
    );
}
