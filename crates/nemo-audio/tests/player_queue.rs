//! Queue behaviour of the streaming player: next, previous, remove, auto-advance, gapless
//! change, format change, failures and events. No device, no network.

mod common;

use std::sync::atomic::Ordering;
use std::time::Duration;

use common::fakes::{Rig, RigOptions, rig, video_id, video_track};
use common::fixtures::level_wav;
use nemo_core::audio::AudioFormat;
use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent};
use nemo_core::track::TrackId;

const LIMIT: Duration = Duration::from_secs(20);

/// Three tracks of `secs` seconds each, with constant levels 1000, 2000, 3000.
fn three_tracks(secs: usize, rate: u32) -> Vec<(String, Vec<u8>)> {
    (1..=3)
        .map(|n| {
            let level = i32::try_from(n).expect("small") * 1000;
            (video_id(n), level_wav(level, rate, secs * rate as usize))
        })
        .collect()
}

fn queue_of(n: u32) -> PlayerCommand {
    PlayerCommand::SetQueue {
        tracks: (1..=n).map(video_track).collect(),
        start: 0,
    }
}

fn index_of(r: &Rig) -> Option<usize> {
    r.player.snapshot().queue_index
}

fn wait_index(r: &mut Rig, want: usize) {
    r.wait("queue index", LIMIT, |r| index_of(r) == Some(want));
}

fn level(sample: f32) -> i32 {
    #[allow(clippy::cast_possible_truncation)] // the value is a small integer
    let v = (f64::from(sample) * 32768.0).round() as i32;
    v
}

#[test]
fn next_and_previous_move_through_the_queue() {
    let mut r = rig(three_tracks(30, 48_000), RigOptions::default());
    r.send(queue_of(3));
    wait_index(&mut r, 0);
    r.wait_event("playing", LIMIT, |e| {
        *e == PlayerEvent::StateChanged(PlayState::Playing)
    });
    r.send(PlayerCommand::Next);
    wait_index(&mut r, 1);
    assert_eq!(
        r.player.snapshot().current.expect("current").id,
        TrackId("t2".into())
    );
    r.send(PlayerCommand::Previous); // well under 3 s in: goes back
    wait_index(&mut r, 0);
    assert!(
        r.drain()
            .contains(&PlayerEvent::TrackChanged(TrackId("t1".into())))
    );
}

#[test]
fn previous_after_three_seconds_restarts_the_track() {
    let mut r = rig(three_tracks(30, 48_000), RigOptions::default());
    r.send(PlayerCommand::SetQueue {
        tracks: (1..=3).map(video_track).collect(),
        start: 1,
    });
    wait_index(&mut r, 1);
    r.wait_event("playing", LIMIT, |e| {
        *e == PlayerEvent::StateChanged(PlayState::Playing)
    });
    r.send(PlayerCommand::Seek(Duration::from_secs(10)));
    r.wait("position past 3 s", LIMIT, |r| {
        r.player.position() > Duration::from_secs(4)
    });
    r.send(PlayerCommand::Previous);
    r.wait("restart", LIMIT, |r| {
        r.player.position() < Duration::from_secs(1)
    });
    assert_eq!(index_of(&r), Some(1), "same track, not the previous one");
}

#[test]
fn remove_enqueue_and_play_next_edit_the_queue() {
    let mut r = rig(three_tracks(30, 48_000), RigOptions::default());
    r.send(queue_of(3));
    wait_index(&mut r, 0);
    r.send(PlayerCommand::RemoveAt(1)); // not the current one
    r.send(PlayerCommand::Enqueue(video_track(2)));
    r.send(PlayerCommand::PlayNext(video_track(3)));
    r.wait("queue edits", LIMIT, |r| {
        r.player.snapshot().queue.len() == 4
    });
    let ids: Vec<String> = r
        .player
        .snapshot()
        .queue
        .iter()
        .map(|t| t.id.0.clone())
        .collect();
    assert_eq!(ids, ["t1", "t3", "t3", "t2"]);
    assert_eq!(index_of(&r), Some(0));
    // Removing the current track plays what follows it.
    r.send(PlayerCommand::RemoveAt(0));
    r.wait("next track after removal", LIMIT, |r| {
        r.player.snapshot().current.is_some_and(|t| t.id.0 == "t3")
    });
    assert_eq!(r.player.snapshot().queue.len(), 3);
    assert!(r.drain().contains(&PlayerEvent::QueueChanged));
}

#[test]
fn tracks_change_without_a_gap_and_the_queue_ends_stopped() {
    let mut r = rig(three_tracks(1, 48_000), RigOptions::default());
    r.send(queue_of(3));
    r.wait_event("stopped at the end", LIMIT, |e| {
        *e == PlayerEvent::StateChanged(PlayState::Stopped)
    });
    let out = r.captured();
    let first = out.iter().position(|s| *s != 0.0).expect("audio");
    let last = out.iter().rposition(|s| *s != 0.0).expect("audio");
    let body = &out[first..=last];
    assert!(
        body.iter().all(|s| *s != 0.0),
        "a zero sample means a gap between tracks"
    );
    let levels: Vec<i32> = body
        .as_chunks::<2>()
        .0
        .iter()
        .map(|f| level(f[0]))
        .collect();
    let mut runs: Vec<(i32, usize)> = Vec::new();
    for l in levels {
        match runs.last_mut() {
            Some((v, n)) if *v == l => *n += 1,
            _ => runs.push((l, 1)),
        }
    }
    assert_eq!(
        runs.iter().map(|r| r.0).collect::<Vec<_>>(),
        [1000, 2000, 3000]
    );
    assert!(
        runs.iter().all(|r| r.1 == 48_000),
        "each track is played whole: {runs:?}"
    );
    let snap = r.player.snapshot();
    assert_eq!(snap.state, PlayState::Stopped);
    assert_eq!(
        r.opened.load(Ordering::SeqCst),
        1,
        "one sink for the whole queue"
    );
    let changed = r
        .drain()
        .iter()
        .filter(|e| matches!(e, PlayerEvent::TrackChanged(_)))
        .count();
    assert_eq!(changed, 3);
}

#[test]
fn a_different_format_reopens_the_sink_when_the_device_can_follow() {
    let files = vec![
        (video_id(1), level_wav(1000, 48_000, 24_000)),
        (video_id(2), level_wav(2000, 44_100, 22_050)),
    ];
    let mut r = rig(
        files,
        RigOptions {
            fixed: None, // the device follows each track's format
            ..RigOptions::default()
        },
    );
    r.send(queue_of(2));
    r.wait_event("stopped at the end", LIMIT, |e| {
        *e == PlayerEvent::StateChanged(PlayState::Stopped)
    });
    assert_eq!(
        r.opened.load(Ordering::SeqCst),
        2,
        "sink reopened for 44.1 kHz"
    );
    let out = r.captured();
    let a = out
        .iter()
        .filter(|s| (**s - 1000.0 / 32768.0).abs() < 1e-9)
        .count();
    let b = out
        .iter()
        .filter(|s| (**s - 2000.0 / 32768.0).abs() < 1e-9)
        .count();
    assert_eq!(
        (a, b),
        (2 * 24_000, 2 * 22_050),
        "both tracks played whole, unresampled"
    );
    assert!(
        r.drain()
            .iter()
            .all(|e| !matches!(e, PlayerEvent::Failed { .. }))
    );
}

#[test]
fn an_unavailable_track_fails_and_the_player_moves_on() {
    let files = vec![(video_id(2), level_wav(2000, 48_000, 24_000))];
    let mut r = rig(files, RigOptions::default());
    r.send(queue_of(2)); // track 1 has no stream
    r.wait_event("failure", LIMIT, |e| {
        matches!(e, PlayerEvent::Failed { .. })
    });
    r.wait("track 2 to play", LIMIT, |r| {
        r.player.snapshot().current.is_some_and(|t| t.id.0 == "t2")
            && r.player.snapshot().state == PlayState::Playing
    });
    assert!(
        r.captured().len() > 1000 || {
            r.wait_samples(1000, LIMIT);
            true
        }
    );
}

#[test]
fn every_subscriber_gets_every_event_and_pause_resume_works() {
    let mut r = rig(three_tracks(30, 48_000), RigOptions::default());
    let mut second = r.player.subscribe();
    r.send(queue_of(1));
    r.wait_event("playing", LIMIT, |e| {
        *e == PlayerEvent::StateChanged(PlayState::Playing)
    });
    r.send(PlayerCommand::Pause);
    r.wait("paused", LIMIT, |r| {
        r.player.snapshot().state == PlayState::Paused
    });
    let at = r.player.position();
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(r.player.position(), at, "position holds while paused");
    r.send(PlayerCommand::Resume);
    r.wait("playing again", LIMIT, |r| {
        r.player.snapshot().state == PlayState::Playing
    });
    r.wait("position moves", LIMIT, |r| r.player.position() > at);
    let mut got = Vec::new();
    while let Some(e) = second.try_next() {
        got.push(e);
    }
    assert!(got.contains(&PlayerEvent::TrackChanged(TrackId("t1".into()))));
    assert!(got.contains(&PlayerEvent::StateChanged(PlayState::Paused)));
}

#[test]
fn a_changed_device_format_is_reported_in_the_signal_path() {
    let files = vec![(video_id(1), level_wav(1000, 44_100, 44_100))];
    let mut r = rig(
        files,
        RigOptions {
            fixed: Some(AudioFormat {
                sample_rate: 48_000,
                channels: 2,
                bit_depth: None,
            }),
            ..RigOptions::default()
        },
    );
    r.send(queue_of(1));
    r.wait_event("path", LIMIT, |e| {
        matches!(e, PlayerEvent::SignalPathChanged(_))
    });
    let path = r.player.snapshot().signal_path.expect("path");
    assert_eq!(path.output.format.sample_rate, 48_000);
    assert_eq!(path.processing.len(), 1);
}
