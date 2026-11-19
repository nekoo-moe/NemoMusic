//! The "off for a day" marker and the chain's hook: fakes and temp folders, no network.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime};

use nemo_core::audio::Codec;
use nemo_core::stream::StreamResolver;
use nemo_core::track::VideoId;

use super::fakes::{ANSWER, FakeRequest, FakeSolver, FakeSource};
use crate::cipher::{CipherResolver, OFF_FOR, OffMarker, PlayerStore};
use crate::resolver::{FallbackResolver, PRIMARY_FAILURE_LIMIT};

fn folder(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("nemo-off-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir); // A leftover of an earlier run is fine to drop.
    dir
}

#[test]
fn the_marker_rests_for_a_day_across_restarts_and_then_expires() {
    let dir = folder("rest");
    let path = dir.join("cache").join("off.json");
    let now = SystemTime::now();
    assert!(!OffMarker::new(Some(path.clone())).resting(now));
    OffMarker::new(Some(path.clone())).switch_off(now);
    // A new run reads the same file.
    let next_run = OffMarker::new(Some(path.clone()));
    assert!(next_run.resting(now + Duration::from_hours(23)));
    assert!(!next_run.resting(now + OFF_FOR + Duration::from_secs(1)));
    assert!(!path.exists(), "an expired marker is deleted");
    let _ = std::fs::remove_dir_all(dir); // Cleanup only.
}

#[test]
fn the_marker_holds_one_time_stamp_and_nothing_else() {
    let dir = folder("content");
    let path = dir.join("off.json");
    let marker = OffMarker::new(Some(path.clone()));
    marker.switch_off(SystemTime::now());
    let text = std::fs::read_to_string(&path).expect("file");
    let value: serde_json::Value = serde_json::from_str(&text).expect("json");
    let keys: Vec<_> = value.as_object().expect("object").keys().cloned().collect();
    assert_eq!(keys, ["until"]);
    assert!(value["until"].is_u64());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).expect("meta").permissions().mode() & 0o777,
            0o600
        );
    }
    marker.clear();
    assert!(!path.exists());
    let _ = std::fs::remove_dir_all(dir); // Cleanup only.
}

#[test]
fn a_damaged_marker_or_no_file_means_the_tier_is_on() {
    let dir = folder("damaged");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("off.json");
    std::fs::write(&path, "not json").expect("write");
    assert!(!OffMarker::new(Some(path)).resting(SystemTime::now()));
    assert!(!OffMarker::new(None).resting(SystemTime::now()));
    OffMarker::new(None).switch_off(SystemTime::now()); // Nothing to write, nothing to fail.
    let _ = std::fs::remove_dir_all(dir); // Cleanup only.
}

#[test]
fn three_refused_streams_switch_the_tier_off_once_and_the_marker_survives_a_restart() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_time()
        .build()
        .expect("runtime");
    let dir = folder("chain");
    let path = dir.join("off.json");
    let source = FakeSource::new("aaaaaaaa", Duration::ZERO);
    let solver = Arc::new(FakeSolver::default());
    let store = PlayerStore::new(source, solver.clone(), rt.handle().clone(), None);
    let cipher = Arc::new(
        CipherResolver::with_parts(
            store,
            solver,
            FakeRequest::new(ANSWER),
            &[Codec::Opus, Codec::Aac],
        )
        .with_off_marker(Arc::new(OffMarker::new(Some(path.clone())))),
    );
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&calls);
    let hook = cipher.off_hook().expect("a marker is kept");
    let hook = Arc::new(move || {
        counted.fetch_add(1, Ordering::SeqCst);
        hook();
    });
    let tier = FallbackResolver::new(cipher, Arc::new(NeverAsked)).with_switched_off(hook);
    for n in 0..PRIMARY_FAILURE_LIMIT {
        let video = VideoId::parse(&format!("{n:011}")).expect("valid");
        assert!(rt.block_on(tier.resolve(&video)).is_ok());
        assert!(
            !path.exists() || n + 1 == PRIMARY_FAILURE_LIMIT,
            "not before the third"
        );
        tier.stream_failed(&video);
    }
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "told once, on the third refused stream"
    );
    // One more refused stream (another video) does not tell again.
    let extra = VideoId::parse("abcdefghijk").expect("valid");
    let _ = rt.block_on(tier.resolve(&extra));
    tier.stream_failed(&extra);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    // A restart: a new marker on the same file rests for a day.
    assert!(OffMarker::new(Some(path)).resting(SystemTime::now()));
    let _ = std::fs::remove_dir_all(dir); // Cleanup only.
}

/// A fallback that must not be asked while the tier answers.
struct NeverAsked;

impl StreamResolver for NeverAsked {
    fn resolve<'a>(
        &'a self,
        _video: &'a VideoId,
    ) -> nemo_core::BoxFuture<
        'a,
        Result<nemo_core::stream::ResolvedStream, nemo_core::stream::StreamError>,
    > {
        Box::pin(async { Err(nemo_core::stream::StreamError::NotAvailable) })
    }
}
