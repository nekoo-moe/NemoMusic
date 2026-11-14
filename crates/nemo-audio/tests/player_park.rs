//! The parked mode: a remote device plays, the local player keeps the queue and decodes nothing.

mod common;

use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use common::fakes::{Rig, RigOptions, rig, video_id, video_track};
use common::fixtures::{counter_index, counter_wav};
use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent};
use nemo_core::track::TrackId;

const LIMIT: Duration = Duration::from_secs(20);

/// Track 1 is a 4 s counter; track 2 is known by id only when `second` is true.
fn parked_rig(second: bool) -> Rig {
    let mut streams = vec![(video_id(1), counter_wav(48_000, 4 * 48_000))];
    if second {
        streams.push((video_id(2), counter_wav(48_000, 4 * 48_000)));
    }
    let mut r = rig(streams, RigOptions::default());
    r.send(PlayerCommand::SetQueue {
        tracks: vec![video_track(1), video_track(2)],
        start: 0,
    });
    r.wait_samples(2 * 4800, LIMIT);
    r.send(PlayerCommand::Park);
    r.wait_event("parked", LIMIT, |e| *e == PlayerEvent::ParkedChanged(true));
    r
}

fn index_of(r: &Rig) -> Option<usize> {
    r.player.snapshot().queue_index
}

fn failures(events: &[PlayerEvent]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, PlayerEvent::Failed { .. }))
        .count()
}

#[test]
fn park_releases_the_device_and_keeps_the_queue_position() {
    let mut r = parked_rig(true);
    let still = r.captured().len();
    thread::sleep(Duration::from_millis(100));
    assert_eq!(r.captured().len(), still, "no audio after Park");
    assert_eq!(r.player.snapshot().state, PlayState::Stopped);
    assert_eq!(index_of(&r), Some(0), "the queue position stays");
    assert!(
        r.drain()
            .contains(&PlayerEvent::StateChanged(PlayState::Stopped))
    );
}

#[test]
fn a_parked_next_reports_the_track_and_decodes_nothing() {
    // Track 2 is unknown to the resolver: any decode attempt would report a failure. Its prefetch
    // before Park already failed once; count only the failures after that.
    let mut r = parked_rig(false);
    let opened = r.opened.load(Ordering::SeqCst);
    let still = r.captured().len();
    let failures_before = failures(r.drain());
    r.send(PlayerCommand::Next);
    r.wait_event("track 2 current", LIMIT, |e| {
        *e == PlayerEvent::TrackChanged(TrackId("t2".into()))
    });
    thread::sleep(Duration::from_millis(100));
    assert_eq!(index_of(&r), Some(1));
    assert_eq!(r.opened.load(Ordering::SeqCst), opened, "no device opened");
    assert_eq!(r.captured().len(), still, "no audio while parked");
    assert_eq!(
        failures(r.drain()),
        failures_before,
        "nothing was resolved or decoded"
    );
    r.send(PlayerCommand::Previous);
    r.wait("back on track 1", LIMIT, |r| index_of(r) == Some(0));
    assert_eq!(r.opened.load(Ordering::SeqCst), opened);
}

#[test]
fn unpark_resumes_at_the_given_position() {
    let mut r = parked_rig(true);
    let before = r.captured().len();
    r.send(PlayerCommand::Unpark {
        at: Duration::from_secs(2),
    });
    r.wait_event("unparked", LIMIT, |e| {
        *e == PlayerEvent::ParkedChanged(false)
    });
    r.wait_samples(before + 2 * 24_000, LIMIT);
    let out = r.captured();
    let first = out[before..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|f| counter_index(f[0]))
        .find(|i| *i >= 0)
        .expect("audio after Unpark");
    assert_eq!(first, 96_000, "playback resumes at 2 s");
    assert_eq!(index_of(&r), Some(0));
}

#[test]
fn park_and_unpark_twice_change_nothing_more() {
    let mut r = parked_rig(true);
    r.send(PlayerCommand::Park);
    r.send(PlayerCommand::Unpark { at: Duration::ZERO });
    r.send(PlayerCommand::Unpark { at: Duration::ZERO });
    // Commands run in order: once the volume change is seen, the ones before it ran too.
    r.send(PlayerCommand::SetVolume(50));
    r.wait_event("volume changed", LIMIT, |e| {
        *e == PlayerEvent::VolumeChanged(50)
    });
    let changes = r
        .drain()
        .iter()
        .filter(|e| matches!(e, PlayerEvent::ParkedChanged(_)))
        .count();
    assert_eq!(changes, 2, "one Park and one Unpark took effect");
}
