//! Real-network smoke test for LRCLIB. Development only, run by hand:
//! `cargo test -p nemo-lyrics --test lrclib_smoke -- --ignored --nocapture`
//!
//! At most 3 requests (get, then search). Asserts on shape only and prints the shape; the lyrics
//! text is never printed or stored.

use std::time::Instant;

use nemo_core::lyrics::{Granularity, LyricsProvider, LyricsQuery};
use nemo_lyrics::LrclibProvider;

#[tokio::test]
#[ignore = "talks to lrclib.net"]
async fn lrclib_finds_a_very_well_known_track() {
    let provider = LrclibProvider::new().expect("client");
    let query = LyricsQuery {
        title: "Bohemian Rhapsody".into(),
        artists: vec!["Queen".into()],
        album: Some("A Night at the Opera".into()),
        duration_ms: Some(355_000),
        ytm_video_id: None,
        local_path: None,
    };
    let start = Instant::now();
    let result = provider.fetch(&query).await;
    let latency = start.elapsed();
    match &result {
        Ok(Some(doc)) => {
            let timed = doc.lines.iter().filter(|l| l.start_ms.is_some()).count();
            let last = doc.lines.iter().filter_map(|l| l.start_ms).max();
            eprintln!(
                "SMOKE ok: granularity={:?} lines={} timed={} last_ms={:?} instrumental={} matched_duration_ms={:?} latency_ms={}",
                doc.granularity,
                doc.lines.len(),
                timed,
                last,
                doc.instrumental,
                doc.matched.as_ref().and_then(|m| m.duration_ms),
                latency.as_millis()
            );
            assert_eq!(doc.provider.as_str(), "LRCLIB");
            assert!(doc.instrumental || !doc.lines.is_empty());
            if doc.granularity != Granularity::Unsynced {
                assert_eq!(timed, doc.lines.len());
            }
        }
        other => {
            eprintln!("SMOKE result: {other:?} latency_ms={}", latency.as_millis());
            // Not found or rate limited is a valid shape; a parse error is not.
            assert!(!matches!(
                other,
                Err(nemo_core::lyrics::LyricsError::Parse(_))
            ));
        }
    }
}
