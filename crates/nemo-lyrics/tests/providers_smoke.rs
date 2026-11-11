//! Real-network smoke test for the web providers. Development only, run by hand:
//! `cargo test -p nemo-lyrics --test providers_smoke -- --ignored --nocapture`
//!
//! Two well-known songs (one English, one Vietnamese), one provider at a time, at least 1 s
//! between requests, no loops. It prints the shape of each answer (hit or miss, granularity,
//! counts, latency) and never the lyrics. The result is checked only for shape.
//!
//! `SimpMusic` needs a YouTube video id. Pass one at run time with `NEMO_SMOKE_VIDEO_ID`; none
//! is stored in the repo. Without it that provider is skipped.

use std::time::{Duration, Instant};

use nemo_core::lyrics::{LyricsError, LyricsProvider, LyricsQuery};
use nemo_core::track::VideoId;
use nemo_lyrics::LrclibProvider;
use nemo_lyrics::blapi::BetterLyricsProvider;
use nemo_lyrics::netease::NeteaseProvider;
use nemo_lyrics::simpmusic::SimpMusicProvider;
use nemo_lyrics::unison::UnisonProvider;

fn query(title: &str, artist: &str, duration_ms: Option<u64>, video: Option<&str>) -> LyricsQuery {
    LyricsQuery {
        title: title.into(),
        artists: vec![artist.into()],
        album: None,
        duration_ms,
        ytm_video_id: video.and_then(|v| VideoId::parse(v).ok()),
        local_path: None,
    }
}

async fn probe(label: &str, provider: &dyn LyricsProvider, q: &LyricsQuery) {
    let start = Instant::now();
    let result = provider.fetch_credited(q).await;
    let ms = start.elapsed().as_millis();
    let id = provider.id().as_str();
    match result {
        Ok(Some((doc, credit))) => {
            let timed = doc.lines.iter().filter(|l| l.start_ms.is_some()).count();
            let words: usize = doc.lines.iter().map(|l| l.words.len()).sum();
            eprintln!(
                "SMOKE {id} / {label}: HIT granularity={:?} lines={} timed={timed} words={words} credit={} latency_ms={ms}",
                doc.granularity,
                doc.lines.len(),
                credit.is_some(),
            );
            assert_eq!(doc.provider.as_str(), id);
        }
        Ok(None) => eprintln!("SMOKE {id} / {label}: MISS latency_ms={ms}"),
        Err(e) => {
            eprintln!("SMOKE {id} / {label}: ERROR {e} latency_ms={ms}");
            assert!(
                !matches!(e, LyricsError::Parse(_)),
                "unexpected parse error for {id}"
            );
        }
    }
    tokio::time::sleep(Duration::from_millis(1100)).await;
}

#[tokio::test]
#[ignore = "talks to unison.betterlyrics.org, lrclib.net, api.betterlyrics.org, music.163.com"]
async fn providers_answer_with_the_documented_shapes() {
    let english = query("Blinding Lights", "The Weeknd", Some(200_000), None);
    // The duration is unknown to us, so it is not sent.
    let vietnamese = query("Nàng Ly", "Desmond Vu", None, None);

    let unison = UnisonProvider::new().expect("client");
    let lrclib = LrclibProvider::new().expect("client");
    let better = BetterLyricsProvider::new().expect("client");
    let netease = NeteaseProvider::new().expect("client");

    // The lookup example from Unison's own README, to prove the happy path.
    let readme_example = query("Explorer", "Justice", None, None);
    probe("unison readme example", &unison, &readme_example).await;

    for (label, q) in [("english", &english), ("vietnamese", &vietnamese)] {
        probe(label, &unison, q).await;
        probe(label, &lrclib, q).await;
        probe(label, &better, q).await;
        probe(label, &netease, q).await;
    }

    if let Ok(video) = std::env::var("NEMO_SMOKE_VIDEO_ID") {
        let simp = SimpMusicProvider::new().expect("client");
        let q = query("Bohemian Rhapsody", "Queen", Some(355_000), Some(&video));
        probe("english (video id from env)", &simp, &q).await;
    }
}
