//! `ReplayGain` in the streaming player: off by default with no arithmetic, the stored gain
//! applied by the feeder when it is on, and a change taking effect from the next song. No
//! device, no network.
#![allow(
    clippy::float_cmp,
    reason = "the off path is checked bit for bit, so exact float equality is the point"
)]

mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use common::fakes::{Rig, RigOptions, local_track, rig};
use common::fixtures::{tone_i16, wav_bytes};
use nemo_audio::replay_gain::{ReplayGain, ReplayGainSettings, ReplayGainSwitch};
use nemo_core::audio::ProcessingStage;
use nemo_core::library::{LocalPlayback, LocalTrackLookup, ReplayGainMode, ReplayGainValues};
use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent};
use nemo_core::track::Track;

const LIMIT: Duration = Duration::from_secs(20);

/// Every local track carries a track gain of -6 dB.
struct Tagged;

impl LocalTrackLookup for Tagged {
    fn playback(&self, _track: &Track) -> Option<LocalPlayback> {
        Some(LocalPlayback {
            gain: tag(),
            ..LocalPlayback::default()
        })
    }
}

fn tag() -> ReplayGainValues {
    ReplayGainValues {
        track_gain_db: Some(-6.0),
        ..ReplayGainValues::default()
    }
}

fn on() -> ReplayGainSettings {
    ReplayGainSettings {
        enabled: true,
        mode: ReplayGainMode::Track,
        pre_gain_db: 0.0,
    }
}

fn as_f32(ints: &[i32]) -> Vec<f32> {
    ints.iter()
        .map(|v| f32::from(i16::try_from(*v).expect("16-bit")) / 32_768.0)
        .collect()
}

fn temp_file(tag: &str, bytes: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nemo-replay-gain-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(tag);
    std::fs::write(&path, bytes).expect("write fixture");
    path
}

fn tagged_rig(switch: &ReplayGainSwitch) -> Rig {
    rig(
        vec![],
        RigOptions {
            local: Some(Arc::new(Tagged)),
            replay_gain: Some(switch.clone()),
            ..RigOptions::default()
        },
    )
}

/// The stages of the last signal path the player reported.
fn last_stages(r: &mut Rig) -> Vec<ProcessingStage> {
    r.drain()
        .iter()
        .rev()
        .find_map(|e| match e {
            PlayerEvent::SignalPathChanged(p) => Some(p.processing.clone()),
            _ => None,
        })
        .expect("a signal path event")
}

fn play_to_end(r: &mut Rig, tracks: Vec<Track>) {
    r.send(PlayerCommand::SetQueue { tracks, start: 0 });
    r.wait_event("the end of the queue", LIMIT, |e| {
        *e == PlayerEvent::StateChanged(PlayState::Stopped)
    });
}

#[test]
fn off_by_default_a_tagged_song_plays_bit_exact_with_no_stage() {
    let ints = tone_i16(48_000, 2, 2 * 48_000, 440.0);
    let path = temp_file("off.wav", &wav_bytes(&ints, 16, 48_000, 2));
    let mut r = tagged_rig(&ReplayGainSwitch::default());
    play_to_end(&mut r, vec![local_track(1, path.clone())]);
    let want = as_f32(&ints);
    assert_eq!(r.captured()[..want.len()], want[..], "no arithmetic");
    assert_eq!(last_stages(&mut r), Vec::new());
    std::fs::remove_file(path).expect("cleanup");
}

#[test]
fn on_the_feeder_scales_by_the_stored_gain_and_the_path_shows_it() {
    let ints = tone_i16(48_000, 2, 2 * 48_000, 440.0);
    let path = temp_file("on.wav", &wav_bytes(&ints, 16, 48_000, 2));
    let switch = ReplayGainSwitch::default();
    switch.set(on());
    let gain = ReplayGain::plan(&on(), &tag()).expect("a stage");
    let mut r = tagged_rig(&switch);
    play_to_end(&mut r, vec![local_track(1, path.clone())]);
    let want: Vec<f32> = as_f32(&ints).iter().map(|s| s * gain.scalar()).collect();
    assert_eq!(
        r.captured()[..want.len()],
        want[..],
        "one scalar on every sample"
    );
    let stages = last_stages(&mut r);
    assert_eq!(stages, vec![ProcessingStage::ReplayGain { db: gain.db() }]);
    assert!((gain.db() + 6.0).abs() < 1e-4);
    std::fs::remove_file(path).expect("cleanup");
}

#[test]
fn a_change_applies_from_the_next_song() {
    let first = tone_i16(48_000, 2, 4 * 48_000, 440.0);
    let second = tone_i16(48_000, 2, 48_000, 660.0);
    let one = temp_file("one.wav", &wav_bytes(&first, 16, 48_000, 2));
    let two = temp_file("two.wav", &wav_bytes(&second, 16, 48_000, 2));
    let switch = ReplayGainSwitch::default();
    let mut r = tagged_rig(&switch);
    r.send(PlayerCommand::SetQueue {
        tracks: vec![local_track(1, one.clone()), local_track(2, two.clone())],
        start: 0,
    });
    r.wait_event("the first song to play", LIMIT, |e| {
        *e == PlayerEvent::StateChanged(PlayState::Playing)
    });
    // The first song is already with the feeder: the change reaches the second only.
    switch.set(on());
    r.wait_event("the end of the queue", LIMIT, |e| {
        *e == PlayerEvent::StateChanged(PlayState::Stopped)
    });
    let scalar = ReplayGain::plan(&on(), &tag()).expect("a stage").scalar();
    let want: Vec<f32> = as_f32(&first)
        .into_iter()
        .chain(as_f32(&second).into_iter().map(|s| s * scalar))
        .collect();
    let out = r.captured();
    assert!(out.len() >= want.len(), "{} of {}", out.len(), want.len());
    assert_eq!(
        out.iter().zip(&want).position(|(got, want)| got != want),
        None,
        "the first song untouched, the second scaled, joined without a gap"
    );
    std::fs::remove_file(one).expect("cleanup");
    std::fs::remove_file(two).expect("cleanup");
}
