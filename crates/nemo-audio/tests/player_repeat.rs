//! Repeat modes of the streaming player: All wraps gaplessly, One replays the current track,
//! and a queue of unplayable tracks still ends. No device, no network.

mod common;

use std::time::Duration;

use common::fakes::{Rig, RigOptions, rig, video_id, video_track};
use common::fixtures::level_wav;
use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent, RepeatMode};

const LIMIT: Duration = Duration::from_secs(20);

fn level(sample: f32) -> i32 {
    #[allow(clippy::cast_possible_truncation)] // the value is a small integer
    let v = (f64::from(sample) * 32768.0).round() as i32;
    v
}

/// Run lengths of the left-channel level between the first and the last non-zero sample.
/// Panics when a zero sample (a gap) lies inside.
fn runs(out: &[f32]) -> Vec<(i32, usize)> {
    let first = out.iter().position(|s| *s != 0.0).expect("audio");
    let last = out.iter().rposition(|s| *s != 0.0).expect("audio");
    let body = &out[first..=last];
    assert!(body.iter().all(|s| *s != 0.0), "a gap between tracks");
    let mut runs: Vec<(i32, usize)> = Vec::new();
    for f in body.as_chunks::<2>().0 {
        let l = level(f[0]);
        match runs.last_mut() {
            Some((v, n)) if *v == l => *n += 1,
            _ => runs.push((l, 1)),
        }
    }
    runs
}

fn one_second_tracks(n: u32) -> Vec<(String, Vec<u8>)> {
    (1..=n)
        .map(|k| {
            let lv = i32::try_from(k).expect("small") * 1000;
            (video_id(k), level_wav(lv, 48_000, 48_000))
        })
        .collect()
}

fn queue_of(n: u32) -> PlayerCommand {
    PlayerCommand::SetQueue {
        tracks: (1..=n).map(video_track).collect(),
        start: 0,
    }
}

fn wait_for_audio(r: &mut Rig, seconds: usize) {
    r.wait_samples(2 * 48_000 * seconds + 2_000, LIMIT);
}

#[test]
fn the_mode_is_in_the_snapshot_and_an_event_reports_it() {
    let mut r = rig(one_second_tracks(1), RigOptions::default());
    assert_eq!(r.player.snapshot().repeat, RepeatMode::Off);
    r.send(PlayerCommand::SetRepeat(RepeatMode::One));
    r.wait_event("repeat event", LIMIT, |e| {
        *e == PlayerEvent::RepeatChanged(RepeatMode::One)
    });
    assert_eq!(r.player.snapshot().repeat, RepeatMode::One);
}

#[test]
fn repeat_all_wraps_to_the_first_track_without_a_gap() {
    let mut r = rig(one_second_tracks(3), RigOptions::default());
    r.send(PlayerCommand::SetRepeat(RepeatMode::All));
    r.send(queue_of(3));
    wait_for_audio(&mut r, 4);
    let out = r.captured();
    let seq: Vec<i32> = runs(&out).iter().map(|r| r.0).collect();
    assert!(
        seq.starts_with(&[1000, 2000, 3000, 1000]),
        "the queue wraps: {seq:?}"
    );
    assert_ne!(r.player.snapshot().state, PlayState::Stopped);
}

#[test]
fn repeat_one_replays_the_current_track_without_a_gap() {
    let mut r = rig(one_second_tracks(2), RigOptions::default());
    r.send(PlayerCommand::SetRepeat(RepeatMode::One));
    r.send(queue_of(2));
    wait_for_audio(&mut r, 3);
    let out = r.captured();
    let seq = runs(&out);
    assert!(seq.iter().all(|r| r.0 == 1000), "only track 1: {seq:?}");
    let frames: usize = seq.iter().map(|r| r.1).sum();
    assert!(frames > 2 * 48_000, "played more than twice: {frames}");
    assert_eq!(r.player.snapshot().queue_index, Some(0));
}

#[test]
fn next_still_moves_on_in_repeat_one_and_wraps_in_repeat_all() {
    let mut r = rig(one_second_tracks(2), RigOptions::default());
    r.send(PlayerCommand::SetRepeat(RepeatMode::One));
    r.send(queue_of(2));
    r.wait("playing", LIMIT, |r| {
        r.player.snapshot().state == PlayState::Playing
    });
    r.send(PlayerCommand::Next);
    r.wait("track 2", LIMIT, |r| {
        r.player.snapshot().queue_index == Some(1)
    });
    r.send(PlayerCommand::SetRepeat(RepeatMode::All));
    r.send(PlayerCommand::Next); // past the end: wraps
    r.wait("track 1 again", LIMIT, |r| {
        r.player.snapshot().queue_index == Some(0)
            && r.player.snapshot().state != PlayState::Stopped
    });
}

#[test]
fn turning_repeat_off_lets_the_queue_end() {
    let mut r = rig(one_second_tracks(1), RigOptions::default());
    r.send(PlayerCommand::SetRepeat(RepeatMode::One));
    r.send(queue_of(1));
    wait_for_audio(&mut r, 2);
    r.send(PlayerCommand::SetRepeat(RepeatMode::Off));
    r.wait_event("stopped", LIMIT, |e| {
        *e == PlayerEvent::StateChanged(PlayState::Stopped)
    });
}

#[test]
fn a_queue_of_unplayable_tracks_ends_even_with_repeat_all() {
    let mut r = rig(Vec::new(), RigOptions::default()); // no stream exists
    r.send(PlayerCommand::SetRepeat(RepeatMode::All));
    r.send(queue_of(2));
    r.wait_event("stopped", LIMIT, |e| {
        *e == PlayerEvent::StateChanged(PlayState::Stopped)
    });
}
