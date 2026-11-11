//! SponsorBlock skipping on the decode side, with a fake segment source.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use common::fakes::{FakeSegments, RigOptions, rig, video_id, video_track};
use common::fixtures::{counter_index, counter_wav};
use nemo_core::player::{PlayerCommand, PlayerEvent};
use nemo_core::segment::{Segment, SegmentCategory};

const LIMIT: Duration = Duration::from_secs(20);

fn segment() -> Segment {
    Segment {
        start: Duration::from_secs(2),
        end: Duration::from_secs(3),
        category: SegmentCategory::Sponsor,
        id: "seg-1".into(),
    }
}

fn indices(out: &[f32]) -> Vec<i64> {
    out.as_chunks::<2>()
        .0
        .iter()
        .map(|f| counter_index(f[0]))
        .collect()
}

fn options(source: &Arc<FakeSegments>) -> RigOptions {
    RigOptions {
        interval: Duration::from_millis(5), // 2x real time
        segments: Some(Arc::clone(source)),
        ..RigOptions::default()
    }
}

fn track_with_duration() -> nemo_core::track::Track {
    let mut t = video_track(1);
    t.duration = Some(Duration::from_secs(6));
    t
}

#[test]
fn a_segment_is_cut_out_and_reported_then_undo_plays_it() {
    let source = Arc::new(FakeSegments {
        segments: vec![segment()],
        calls: AtomicUsize::new(0),
    });
    let mut r = rig(
        vec![(video_id(1), counter_wav(48_000, 288_000))],
        options(&source),
    );
    r.send(PlayerCommand::SetSkipSegments(true));
    r.send(PlayerCommand::Play(track_with_duration()));
    r.wait_event("segment skipped", LIMIT, |e| {
        *e == PlayerEvent::SegmentSkipped(segment())
    });
    assert_eq!(source.calls.load(Ordering::SeqCst), 1);
    // The event fires when playback reaches the cut; the frames after it may not be captured
    // yet. Wait for 0.1 s more audio so the jump is visible (timing fix, not a behaviour change).
    let now = r.captured().len();
    r.wait_samples(now + 9_600, LIMIT);
    let idx = indices(&r.captured());
    let cut = idx
        .windows(2)
        .position(|w| w[1] != w[0] + 1)
        .expect("a jump in the frame numbers");
    assert_eq!(idx[cut], 95_999, "audio stops at the segment start");
    assert_eq!(idx[cut + 1], 144_000, "audio resumes at the segment end");

    // Undo seeks back to the segment start and does not skip it again.
    let before = r.captured().len();
    r.send(PlayerCommand::UndoSkip);
    r.wait_samples(before + 2 * 60_000, LIMIT);
    let idx = indices(&r.captured()[before..]);
    let back = idx
        .iter()
        .position(|i| *i == 96_000)
        .expect("the segment start plays again");
    assert!(
        idx[back..back + 50_000]
            .iter()
            .enumerate()
            .all(|(k, i)| *i == 96_000 + i64::try_from(k).expect("small")),
        "the whole segment plays after the undo"
    );
    let skipped = r
        .drain()
        .iter()
        .filter(|e| matches!(e, PlayerEvent::SegmentSkipped(_)))
        .count();
    assert_eq!(skipped, 1, "the ignored segment is not skipped again");
}

#[test]
fn skipping_off_plays_everything_and_asks_for_no_segments() {
    let source = Arc::new(FakeSegments {
        segments: vec![segment()],
        calls: AtomicUsize::new(0),
    });
    let mut r = rig(
        vec![(video_id(1), counter_wav(48_000, 96_000))],
        options(&source),
    );
    r.send(PlayerCommand::Play(track_with_duration()));
    r.wait_samples(2 * 96_000, LIMIT);
    let idx = indices(&r.captured()[..2 * 96_000]);
    assert!(idx.windows(2).all(|w| w[1] == w[0] + 1), "no jump");
    assert_eq!(source.calls.load(Ordering::SeqCst), 0);
}
