//! `LyricsHub` returns lyrics best first: Syllable > Word > Line > Unsynced, then score, then
//! chain order; a stored per-track pick still wins. Fake providers only; no network.

use std::sync::Arc;

use nemo_core::BoxFuture;
use nemo_core::lyrics::{
    Granularity, LyricLine, LyricsDoc, LyricsError, LyricsProvider, LyricsQuery, ProviderId,
};
use nemo_core::services::LyricsService;
use nemo_core::track::VideoId;
use nemo_lyrics::{ChainEntry, ChainOptions, LyricsCache, LyricsHub, track_key};
use tokio::runtime::Handle;

struct Fake {
    id: ProviderId,
    host: &'static [&'static str],
    level: Granularity,
}

impl LyricsProvider for Fake {
    fn id(&self) -> ProviderId {
        self.id
    }
    fn hosts(&self) -> &'static [&'static str] {
        self.host
    }
    fn fetch<'a>(
        &'a self,
        _q: &'a LyricsQuery,
    ) -> BoxFuture<'a, Result<Option<LyricsDoc>, LyricsError>> {
        let timed = self.level != Granularity::Unsynced;
        let doc = LyricsDoc {
            lines: vec![LyricLine {
                start_ms: timed.then_some(1000),
                end_ms: None,
                text: "la".into(),
                words: Vec::new(),
            }],
            granularity: self.level,
            provider: self.id,
            matched: None,
            instrumental: false,
        };
        Box::pin(async move { Ok(Some(doc)) })
    }
    fn best_granularity(&self) -> Granularity {
        self.level
    }
}

struct Dir(std::path::PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A hub whose providers answer in this chain order: plain, line, word, syllable.
fn hub(name: &str) -> (LyricsHub, Dir) {
    let dir = std::env::temp_dir().join(format!(
        "nemo-lyrics-levels-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    let fakes: [(&'static str, &'static [&'static str], Granularity); 4] = [
        ("plain", &["a.example"], Granularity::Unsynced),
        ("line", &["b.example"], Granularity::Line),
        ("word", &["c.example"], Granularity::Word),
        ("syllable", &["d.example"], Granularity::Syllable),
    ];
    let entries = fakes
        .into_iter()
        .map(|(id, host, level)| {
            ChainEntry::new(Arc::new(Fake {
                id: ProviderId::new(id),
                host,
                level,
            }))
        })
        .collect();
    let cache = LyricsCache::open(&dir).expect("cache");
    let hub = LyricsHub::with_chain(cache, Handle::current(), entries, ChainOptions::default());
    (hub, Dir(dir))
}

fn query() -> LyricsQuery {
    LyricsQuery {
        title: "Test Song".into(),
        artists: vec!["Test Artist".into()],
        album: None,
        duration_ms: Some(200_000),
        ytm_video_id: Some(VideoId::parse("abcdefghijk").expect("id")),
        local_path: None,
    }
}

fn provider(doc: Option<&LyricsDoc>) -> Option<&'static str> {
    doc.map(|d| d.provider.as_str())
}

#[tokio::test]
async fn lookup_returns_syllable_lyrics_over_earlier_providers() {
    let (hub, _dir) = hub("lookup");
    let q = query();
    assert_eq!(
        provider(hub.lookup(&q).await.doc.as_ref()),
        Some("syllable")
    );
    // Served again from the cache with the same pick.
    assert_eq!(
        provider(hub.lookup(&q).await.doc.as_ref()),
        Some("syllable")
    );
}

#[tokio::test]
async fn lookup_all_is_best_first() {
    let (hub, _dir) = hub("all");
    let q = query();
    for _ in 0..2 {
        let all = hub.lookup_all(&q).await;
        let levels: Vec<Granularity> = all.iter().map(|c| c.granularity).collect();
        assert_eq!(
            levels,
            [
                Granularity::Syllable,
                Granularity::Word,
                Granularity::Line,
                Granularity::Unsynced
            ]
        );
    }
}

#[tokio::test]
async fn stored_pick_beats_syllable_until_cleared() {
    let (hub, _dir) = hub("pick");
    let q = query();
    let key = track_key(&q);
    hub.choose(&key, ProviderId::new("line"));
    assert_eq!(provider(hub.lookup(&q).await.doc.as_ref()), Some("line"));
    hub.clear_choice(&key);
    assert_eq!(
        provider(hub.lookup(&q).await.doc.as_ref()),
        Some("syllable")
    );
}

#[tokio::test]
async fn providers_report_their_best_level() {
    let (hub, _dir) = hub("best");
    let best: Vec<(&str, Granularity)> = hub
        .providers()
        .into_iter()
        .map(|p| (p.id.as_str(), p.best))
        .collect();
    assert_eq!(best[3], ("syllable", Granularity::Syllable));
    assert_eq!(best[0], ("plain", Granularity::Unsynced));
}
