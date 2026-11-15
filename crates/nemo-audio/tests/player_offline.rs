//! Saved songs in the streaming player (offline-cache): played from the cache with no stream
//! lookup, a badge from the file's measured facts, and a damaged copy marked unsaved. No
//! device, no network.
#![allow(
    clippy::float_cmp,
    reason = "playback is checked bit for bit, so exact float equality is the point"
)]

mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use common::fakes::{FakeOffline, Rig, RigOptions, rig, video_id, video_track};
use common::fixtures::{tone_i16, wav_bytes};
use nemo_core::audio::{Codec, Origin, QualityClass, StreamInfo};
use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent};

const LIMIT: Duration = Duration::from_secs(20);

fn temp_file(tag: &str, bytes: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nemo-offline-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(tag);
    std::fs::write(&path, bytes).expect("write fixture");
    path
}

fn as_f32(ints: &[i32]) -> Vec<f32> {
    ints.iter()
        .map(|v| f32::from(i16::try_from(*v).expect("16-bit")) / 32_768.0)
        .collect()
}

fn saved_rig(store: &Arc<FakeOffline>, streams: Vec<(String, Vec<u8>)>) -> Rig {
    rig(
        streams,
        RigOptions {
            offline: Some(Arc::clone(store) as _),
            ..RigOptions::default()
        },
    )
}

/// The source of the last signal path the player reported.
fn last_source(r: &mut Rig) -> StreamInfo {
    r.drain()
        .iter()
        .rev()
        .find_map(|e| match e {
            PlayerEvent::SignalPathChanged(p) => Some(p.source.clone()),
            _ => None,
        })
        .expect("a signal path event")
}

fn wait_stopped(r: &mut Rig) {
    r.wait_event("the end of the queue", LIMIT, |e| {
        *e == PlayerEvent::StateChanged(PlayState::Stopped)
    });
}

#[test]
fn saved_songs_play_from_the_cache_with_no_stream_lookup() {
    let songs: Vec<Vec<i32>> = [440.0, 550.0, 660.0]
        .iter()
        .map(|f| tone_i16(48_000, 2, 48_000, *f))
        .collect();
    let saved: Vec<(String, PathBuf)> = songs
        .iter()
        .zip(1..)
        .map(|(ints, n)| {
            let path = temp_file(&format!("song-{n}"), &wav_bytes(ints, 16, 48_000, 2));
            (video_id(n), path)
        })
        .collect();
    let store = FakeOffline::new(saved.clone());
    let mut r = saved_rig(&store, Vec::new());
    r.send(PlayerCommand::SetQueue {
        tracks: (1..=3).map(video_track).collect(),
        start: 0,
    });
    wait_stopped(&mut r);
    let want: Vec<f32> = songs.iter().flat_map(|s| as_f32(s)).collect();
    let out = r.captured();
    assert!(out.len() >= want.len(), "{} of {}", out.len(), want.len());
    assert_eq!(out[..want.len()], want[..], "the three saved files, joined");
    assert_eq!(
        r.resolver_calls.load(Ordering::SeqCst),
        0,
        "no resolve and no warm hint for a saved song"
    );
    assert!(store.unsaved.lock().expect("lock").is_empty());
    for (_, path) in saved {
        std::fs::remove_file(path).expect("cleanup");
    }
}

#[test]
fn the_badge_shows_the_probed_codec_and_measured_bitrate_and_stays_lossy() {
    let ints = tone_i16(48_000, 2, 48_000, 440.0);
    let bytes = wav_bytes(&ints, 16, 48_000, 2);
    let path = temp_file("badge", &bytes);
    let store = FakeOffline::new(vec![(video_id(1), path.clone())]);
    let mut r = saved_rig(&store, Vec::new());
    r.send(PlayerCommand::Play(video_track(1)));
    wait_stopped(&mut r);
    let source = last_source(&mut r);
    // The store states Opus at 160 kbps; the badge follows the file: PCM, bytes * 8 / 1 s.
    let kbps = u32::try_from((bytes.len() * 8 + 500) / 1000).expect("small");
    assert_eq!(source.origin, Origin::YoutubeMusic);
    assert_eq!(source.codec, Codec::Pcm);
    assert_eq!(source.bitrate_kbps, Some(kbps));
    assert_eq!(
        source.quality_class(),
        QualityClass::Lossy,
        "never lossless"
    );
    let badge = source.badge();
    assert!(
        !badge.contains("Lossless") && !badge.contains("Hi-Res"),
        "{badge}"
    );
    std::fs::remove_file(path).expect("cleanup");
}

#[test]
fn a_damaged_saved_copy_is_marked_unsaved_and_the_song_streams() {
    let ints = tone_i16(48_000, 2, 48_000, 330.0);
    let path = temp_file("damaged", b"not an audio file at all");
    let store = FakeOffline::new(vec![(video_id(1), path.clone())]);
    let streams = vec![(video_id(1), wav_bytes(&ints, 16, 48_000, 2))];
    let mut r = saved_rig(&store, streams);
    r.send(PlayerCommand::Play(video_track(1)));
    wait_stopped(&mut r);
    let want = as_f32(&ints);
    assert_eq!(
        r.captured()[..want.len()],
        want[..],
        "the stream plays instead"
    );
    assert_eq!(*store.unsaved.lock().expect("lock"), vec![video_id(1)]);
    assert!(
        r.resolver_calls.load(Ordering::SeqCst) > 0,
        "it was streamed"
    );
    std::fs::remove_file(path).expect("cleanup");
}

#[test]
fn a_missing_saved_copy_with_no_stream_fails_with_a_message_and_no_path() {
    let path = std::env::temp_dir().join("nemo-offline-missing-Secret Title.opus");
    let store = FakeOffline::new(vec![(video_id(1), path)]);
    let mut r = saved_rig(&store, Vec::new());
    r.send(PlayerCommand::Play(video_track(1)));
    r.wait_event("a failure", LIMIT, |e| {
        matches!(e, PlayerEvent::Failed { .. })
    });
    let message = r
        .drain()
        .iter()
        .find_map(|e| match e {
            PlayerEvent::Failed { message } => Some(message.clone()),
            _ => None,
        })
        .expect("a failure event");
    assert!(
        !message.is_empty() && !message.contains("Secret Title"),
        "{message}"
    );
    assert_eq!(*store.unsaved.lock().expect("lock"), vec![video_id(1)]);
    assert!(r.captured().is_empty(), "nothing plays");
}
