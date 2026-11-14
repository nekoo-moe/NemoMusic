//! CUE tracks: each plays one span of a single FLAC file. Three tracks in a row equal the whole
//! file frame for frame, and seeking counts from the track's own start. No device, no network.
#![allow(
    clippy::float_cmp,
    reason = "frame-exact checks compare samples exactly, so exact float equality is the point"
)]

mod common;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::fakes::{RigOptions, local_track, rig};
use common::fixtures::flac_bytes;
use nemo_core::library::{LocalPlayback, LocalTrackLookup, TrackSpan};
use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent};
use nemo_core::track::Track;

const LIMIT: Duration = Duration::from_secs(20);
const RATE: u32 = 48_000;

/// Spans by track id, as `nemo-library` reads them from a CUE sheet. Records the thread of
/// every lookup.
struct Cue {
    spans: HashMap<String, TrackSpan>,
    threads: Mutex<Vec<Option<String>>>,
}

impl LocalTrackLookup for Cue {
    fn playback(&self, track: &Track) -> Option<LocalPlayback> {
        let thread = std::thread::current().name().map(str::to_owned);
        self.threads.lock().expect("threads").push(thread);
        self.spans.get(&track.id.0).map(|span| LocalPlayback {
            span: Some(*span),
            ..LocalPlayback::default()
        })
    }
}

/// A stereo 16-bit FLAC whose left channel holds `frame % 30 000 + 1` and right the negative,
/// so every played frame can be told apart.
fn counter_flac(frames: usize) -> Vec<u8> {
    let mut samples = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let v = i16::try_from(i % 30_000 + 1).expect("invariant: below 30 001");
        samples.extend([v, -v]);
    }
    flac_bytes(&samples, RATE, 2)
}

/// The decoded value of a counter sample.
fn sample(frame: usize) -> f32 {
    f32::from(i16::try_from(frame % 30_000 + 1).expect("invariant: below 30 001")) / 32_768.0
}

fn temp_file(tag: &str, bytes: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nemo-player-cue-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(tag);
    std::fs::write(&path, bytes).expect("write fixture");
    path
}

/// A CUE time `mm:ss:ff` (75 frames per second) with no minutes, truncated to whole nanoseconds
/// as a parser that keeps a `Duration` gets it.
fn cue_time(seconds: u64, frames: u64) -> Duration {
    Duration::from_nanos((seconds * 75 + frames) * 1_000_000_000 / 75)
}

fn lookup(spans: &[(u32, TrackSpan)]) -> Arc<Cue> {
    Arc::new(Cue {
        spans: spans
            .iter()
            .map(|(n, span)| (format!("t{n}"), *span))
            .collect(),
        threads: Mutex::new(Vec::new()),
    })
}

#[test]
fn three_cue_tracks_in_a_row_equal_the_whole_file_frame_for_frame() {
    // 5.12 s, 245 760 frames (60 FLAC blocks). Each track is longer than the 200 ms prefill.
    // The sheet:
    //   TRACK 01 AUDIO  INDEX 01 00:00:00
    //   TRACK 02 AUDIO  INDEX 01 00:01:13   (frame 56 320, inside a FLAC block)
    //   TRACK 03 AUDIO  INDEX 01 00:03:30   (frame 163 200, inside a FLAC block)
    let frames = 4096 * 60;
    let path = temp_file("album.flac", &counter_flac(frames));
    let (two, three) = (cue_time(1, 13), cue_time(3, 30));
    let cue = lookup(&[
        (
            1,
            TrackSpan {
                start: Duration::ZERO,
                end: Some(two),
            },
        ),
        (
            2,
            TrackSpan {
                start: two,
                end: Some(three),
            },
        ),
        (
            3,
            TrackSpan {
                start: three,
                end: None,
            },
        ),
    ]);
    let mut r = rig(
        vec![],
        RigOptions {
            local: Some(Arc::clone(&cue) as Arc<dyn LocalTrackLookup>),
            ..RigOptions::default()
        },
    );
    r.send(PlayerCommand::SetQueue {
        tracks: (1..=3).map(|n| local_track(n, path.clone())).collect(),
        start: 0,
    });
    r.wait_event("end of the third track", LIMIT, |e| {
        *e == PlayerEvent::StateChanged(PlayState::Stopped)
    });
    let want: Vec<f32> = (0..frames).flat_map(|f| [sample(f), -sample(f)]).collect();
    let out = r.captured();
    assert!(
        out.len() >= want.len(),
        "{} of {} samples",
        out.len(),
        want.len()
    );
    assert_eq!(
        out.iter().zip(&want).position(|(got, want)| got != want),
        None,
        "first wrong sample: no frame may be lost, doubled or moved"
    );
    assert!(out[want.len()..].iter().all(|s| *s == 0.0), "nothing after");
    let seen = r.drain();
    for d in [two, three.saturating_sub(two)] {
        assert!(seen.contains(&PlayerEvent::DurationKnown(d)), "{d:?}");
    }
    let threads = cue.threads.lock().expect("threads").clone();
    assert_eq!(threads.len(), 3, "one lookup per track");
    assert!(
        threads.iter().all(|t| t.as_deref() == Some("nemo-prepare")),
        "looked up on the prepare thread only: {threads:?}"
    );
    std::fs::remove_file(path).expect("cleanup");
}

#[test]
fn a_seek_in_a_cue_track_counts_from_its_start_and_stops_at_its_end() {
    // 2.56 s of audio; the track is 0.5 s to 2.0 s (frames 24 000 to 96 000).
    let frames = 4096 * 30;
    let path = temp_file("long.flac", &counter_flac(frames));
    let cue = lookup(&[(
        1,
        TrackSpan {
            start: Duration::from_millis(500),
            end: Some(Duration::from_secs(2)),
        },
    )]);
    let mut r = rig(
        vec![],
        RigOptions {
            interval: Duration::from_millis(10), // real time, so the seek comes before the end
            local: Some(cue as Arc<dyn LocalTrackLookup>),
            ..RigOptions::default()
        },
    );
    r.send(PlayerCommand::Play(local_track(1, path.clone())));
    r.wait_samples(2 * 4800, LIMIT);
    assert_eq!(
        r.captured()[0],
        sample(24_000),
        "the track starts at its span"
    );
    r.send(PlayerCommand::Seek(Duration::from_millis(500)));
    r.wait_event("end of track", LIMIT, |e| {
        *e == PlayerEvent::StateChanged(PlayState::Stopped)
    });
    let out = r.captured();
    let left: Vec<f32> = out.as_chunks::<2>().0.iter().map(|f| f[0]).collect();
    // Track position 0.5 s is file frame 48 000; the track ends before file frame 96 000.
    let want: Vec<f32> = (48_000..96_000).map(sample).collect();
    let at = left
        .windows(2000)
        .position(|w| w == &want[..2000])
        .expect("file frame 48 000 plays after the seek");
    let rest = &left[at..];
    assert!(
        rest.len() >= want.len(),
        "{} frames after the seek",
        rest.len()
    );
    assert_eq!(
        rest.iter().zip(&want).position(|(got, want)| got != want),
        None,
        "first wrong frame of the rest of the track"
    );
    assert!(
        rest[want.len()..].iter().all(|s| *s == 0.0),
        "nothing after the span end"
    );
    std::fs::remove_file(path).expect("cleanup");
}
