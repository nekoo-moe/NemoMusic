//! Unit tests for the lyrics hub.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use nemo_core::lyrics::{Granularity, LyricLine, LyricsDoc, ProviderInfo};
use nemo_core::track::VideoId;

use crate::local::LOCAL_ID;

use super::*;

type Answer = Result<Option<LyricsDoc>, LyricsError>;

struct Fake {
    id: ProviderId,
    answer: Answer,
    calls: Arc<AtomicUsize>,
    credit: Option<LyricsCredit>,
    info: ProviderInfo,
}

impl LyricsProvider for Fake {
    fn id(&self) -> ProviderId {
        self.id
    }
    fn hosts(&self) -> &'static [&'static str] {
        &["fake.example"]
    }
    fn fetch<'a>(&'a self, _q: &'a LyricsQuery) -> BoxFuture<'a, Answer> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let answer = self.answer.clone();
        Box::pin(async move { answer })
    }
    fn fetch_credited<'a>(
        &'a self,
        _q: &'a LyricsQuery,
    ) -> BoxFuture<'a, Result<Option<nemo_core::lyrics::CreditedDoc>, LyricsError>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let answer = self.answer.clone();
        let credit = self.credit.clone();
        Box::pin(async move { Ok(answer?.map(|d| (d, credit))) })
    }
    fn info(&self) -> ProviderInfo {
        self.info
    }
}

fn doc(name: &'static str) -> LyricsDoc {
    LyricsDoc {
        lines: vec![LyricLine {
            start_ms: Some(1000),
            end_ms: None,
            text: "hello".into(),
            words: vec![],
        }],
        granularity: Granularity::Line,
        provider: ProviderId::new(name),
        matched: None,
        instrumental: false,
    }
}

fn query() -> LyricsQuery {
    LyricsQuery {
        title: "Test Song".into(),
        artists: vec!["Test Artist".into()],
        album: Some("Test Album".into()),
        duration_ms: Some(200_000),
        ytm_video_id: Some(VideoId::parse("abcdefghijk").expect("id")),
        local_path: None,
    }
}

struct Setup {
    hub: LyricsHub,
    calls: Arc<AtomicUsize>,
    dir: std::path::PathBuf,
}

impl Drop for Setup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn setup(name: &str, answer: Answer) -> Setup {
    let dir = std::env::temp_dir().join(format!(
        "nemo-lyrics-hub-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    let calls = Arc::new(AtomicUsize::new(0));
    let fake = Fake {
        id: ProviderId::new("fake"),
        answer,
        calls: Arc::clone(&calls),
        credit: None,
        info: ProviderInfo::OPEN,
    };
    let cache = LyricsCache::open(&dir).expect("cache");
    let hub = LyricsHub::with_chain(
        cache,
        Handle::current(),
        vec![ChainEntry::new(Arc::new(fake))],
        ChainOptions::default(),
    );
    Setup { hub, calls, dir }
}

#[tokio::test]
async fn hit_is_cached_and_served_without_asking_again() {
    let s = setup("hit", Ok(Some(doc("fake"))));
    let q = query();
    let first = s.hub.lookup(&q).await;
    assert!(first.doc.is_some());
    assert_eq!(first.tried, [ProviderId::new("fake")]);
    let second = s.hub.lookup(&q).await;
    assert_eq!(second.doc, first.doc);
    assert_eq!(s.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn miss_and_error_are_negative_cached() {
    let s = setup("miss", Ok(None));
    let q = query();
    assert!(s.hub.lookup(&q).await.doc.is_none());
    assert!(s.hub.lookup(&q).await.doc.is_none());
    assert_eq!(s.calls.load(Ordering::SeqCst), 1);

    let e = setup(
        "err",
        Err(LyricsError::RateLimited {
            retry_after: Some(Duration::from_secs(60)),
        }),
    );
    let lookup = e.hub.lookup(&q).await;
    assert!(lookup.doc.is_none());
    assert_eq!(lookup.tried.len(), 1);
    let _ = e.hub.lookup(&q).await;
    assert_eq!(e.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn refetch_and_clear_ask_the_provider_again() {
    let s = setup("refetch", Ok(Some(doc("fake"))));
    let q = query();
    let _ = s.hub.lookup(&q).await;
    s.hub.refetch(&track_key(&q));
    let _ = s.hub.lookup(&q).await;
    assert_eq!(s.calls.load(Ordering::SeqCst), 2);
    s.hub.clear_cache();
    let _ = s.hub.lookup(&q).await;
    assert_eq!(s.calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn disabled_provider_is_not_asked_or_listed() {
    let s = setup("disabled", Ok(Some(doc("fake"))));
    assert!(s.hub.set_enabled(ProviderId::new("fake"), false));
    assert!(!s.hub.set_enabled(ProviderId::new("nope"), false));
    let lookup = s.hub.lookup(&query()).await;
    assert!(lookup.doc.is_none() && lookup.tried.is_empty());
    assert_eq!(s.calls.load(Ordering::SeqCst), 0);
    assert_eq!(s.hub.chain_config(), [(ProviderId::new("fake"), false)]);
}

#[tokio::test]
async fn order_is_editable() {
    let s = setup("order", Ok(None));
    s.hub
        .set_order(&[ProviderId::new("unknown"), ProviderId::new("fake")]);
    assert_eq!(s.hub.chain_config().len(), 1);
}

#[tokio::test]
async fn offsets_are_stored_and_clamped() {
    let s = setup("offset", Ok(None));
    assert_eq!(s.hub.offset_ms("abc"), 0);
    s.hub.set_offset_ms("abc", 350);
    assert_eq!(s.hub.offset_ms("abc"), 350);
    s.hub.set_offset_ms("abc", -999_999);
    assert_eq!(s.hub.offset_ms("abc"), -MAX_OFFSET_MS);
    s.hub.set_offset_ms("abc", 0);
    assert_eq!(s.hub.offset_ms("abc"), 0);
}

#[test]
fn key_is_video_id_or_stable_hash() {
    let mut q = query();
    assert_eq!(track_key(&q), "abcdefghijk");
    q.ytm_video_id = None;
    let a = track_key(&q);
    assert!(a.starts_with("h:") && a.len() == 2 + 32);
    // Case, punctuation, feat. suffix and 400 ms of duration noise do not change the key.
    let mut same = q.clone();
    same.title = "TEST song (feat. X)".into();
    same.artists = vec!["test  artist!".into()];
    same.duration_ms = Some(200_400);
    assert_eq!(track_key(&same), a);
    // A different track does.
    let mut other = q.clone();
    other.title = "Another Song".into();
    assert_ne!(track_key(&other), a);
    let mut longer = q;
    longer.duration_ms = Some(215_000);
    assert_ne!(track_key(&longer), a);
}

fn word_doc(name: &'static str) -> LyricsDoc {
    let mut d = doc(name);
    d.granularity = Granularity::Word;
    d
}

struct Multi {
    hub: LyricsHub,
    calls: Vec<Arc<AtomicUsize>>,
    dir: std::path::PathBuf,
}

impl Drop for Multi {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A hub with fake providers `(name, answer, credit, default_enabled)` in chain order.
fn multi(name: &str, fakes: Vec<(&'static str, Answer, Option<LyricsCredit>, bool)>) -> Multi {
    let dir = std::env::temp_dir().join(format!(
        "nemo-lyrics-multi-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    let mut calls = Vec::new();
    let entries = fakes
        .into_iter()
        .map(|(id, answer, credit, on)| {
            let c = Arc::new(AtomicUsize::new(0));
            calls.push(Arc::clone(&c));
            let fake = Fake {
                id: ProviderId::new(id),
                answer,
                calls: c,
                credit,
                info: if on {
                    ProviderInfo::OPEN
                } else {
                    ProviderInfo::opt_in("risky")
                },
            };
            ChainEntry {
                provider: Arc::new(fake),
                enabled: on,
            }
        })
        .collect();
    let hub = LyricsHub::with_chain(
        LyricsCache::open(&dir).expect("cache"),
        Handle::current(),
        entries,
        ChainOptions::default(),
    );
    Multi { hub, calls, dir }
}

fn credit() -> LyricsCredit {
    LyricsCredit {
        label: "Lyrics from Fake".into(),
        url: Some("https://fake.example/song/1".into()),
        by: Some("tester".into()),
        by_url: None,
        note: Some("community confidence: low".into()),
    }
}

#[tokio::test]
async fn lookup_all_lists_every_answer_best_first_with_notes() {
    let m = multi(
        "all",
        vec![
            ("line-src", Ok(Some(doc("line-src"))), None, true),
            (
                "word-src",
                Ok(Some(word_doc("word-src"))),
                Some(credit()),
                true,
            ),
            ("empty-src", Ok(None), None, true),
            ("opt-in", Ok(Some(doc("opt-in"))), None, false),
        ],
    );
    let all = m.hub.lookup_all(&query()).await;
    let names: Vec<&str> = all.iter().map(|c| c.provider.as_str()).collect();
    // Word first; the disabled provider is not asked; a miss does not appear.
    assert_eq!(names, ["word-src", "line-src"]);
    assert_eq!(all[0].granularity, Granularity::Word);
    assert_eq!(all[0].score, 100);
    assert_eq!(
        all[0].source_note.as_deref(),
        Some("community confidence: low")
    );
    assert_eq!(
        all[0].credit.as_ref().map(|c| c.label.as_str()),
        Some("Lyrics from Fake")
    );
    assert_eq!(m.calls[3].load(Ordering::SeqCst), 0);
    // A second call is served from the cache, credit included.
    let again = m.hub.lookup_all(&query()).await;
    assert_eq!(again.len(), 2);
    assert_eq!(again[0].credit, all[0].credit);
    assert_eq!(m.calls[0].load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn chosen_provider_is_returned_first_and_remembered() {
    let m = multi(
        "choose",
        vec![
            ("line-src", Ok(Some(doc("line-src"))), None, true),
            ("word-src", Ok(Some(word_doc("word-src"))), None, true),
        ],
    );
    let q = query();
    let key = track_key(&q);
    assert_eq!(m.hub.chosen(&key), None);
    // Without a pick the best granularity wins.
    let first = m.hub.lookup(&q).await;
    assert_eq!(first.doc.map(|d| d.provider.as_str()), Some("word-src"));

    m.hub.choose(&key, ProviderId::new("line-src"));
    assert_eq!(m.hub.chosen(&key), Some(ProviderId::new("line-src")));
    let picked = m.hub.lookup(&q).await;
    assert_eq!(picked.doc.map(|d| d.provider.as_str()), Some("line-src"));

    // The pick survives a refetch (it is a user setting) and still wins on fresh answers.
    m.hub.refetch(&key);
    let again = m.hub.lookup(&q).await;
    assert_eq!(again.doc.map(|d| d.provider.as_str()), Some("line-src"));

    m.hub.clear_choice(&key);
    assert_eq!(m.hub.chosen(&key), None);
    let best = m.hub.lookup(&q).await;
    assert_eq!(best.doc.map(|d| d.provider.as_str()), Some("word-src"));
}

#[tokio::test]
async fn credit_is_stored_with_the_hit() {
    let m = multi(
        "credit",
        vec![(
            "word-src",
            Ok(Some(word_doc("word-src"))),
            Some(credit()),
            true,
        )],
    );
    let q = query();
    let key = track_key(&q);
    let id = ProviderId::new("word-src");
    assert_eq!(m.hub.credit(&key, id), None);
    let _ = m.hub.lookup(&q).await;
    assert_eq!(m.hub.credit(&key, id), Some(credit()));
    m.hub.refetch(&key);
    assert_eq!(m.hub.credit(&key, id), None);
}

#[tokio::test]
async fn provider_list_and_choices_persist() {
    let dir = std::env::temp_dir().join(format!("nemo-lyrics-persist-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let hub = LyricsHub::new(&dir, Handle::current(), Vec::new()).expect("hub");
    let state = |hub: &LyricsHub| {
        hub.providers()
            .into_iter()
            .map(|p| (p.id.as_str(), p.enabled, p.best))
            .collect::<Vec<_>>()
    };
    // With no stored choice: syllable- and word-capable sources on, line-only grey sources
    // off, each grey one with a risk note.
    assert_eq!(
        state(&hub),
        [
            ("Local file", true, Granularity::Syllable),
            ("Unison", true, Granularity::Syllable),
            ("LRCLIB", true, Granularity::Line),
            ("Better Lyrics", true, Granularity::Syllable),
            ("SimpMusic", false, Granularity::Line),
            ("NetEase", false, Granularity::Line),
        ]
    );
    for p in hub.providers() {
        assert_eq!(
            p.risk_note.is_some(),
            !p.default_enabled,
            "{}",
            p.id.as_str()
        );
        assert!(p.id == LOCAL_ID || !p.hosts.is_empty());
        assert!(p.default_enabled || p.best <= Granularity::Line);
    }
    assert!(hub.set_provider_enabled(ProviderId::new("NetEase"), true));
    assert!(hub.set_provider_enabled(ProviderId::new("Unison"), false));
    assert!(!hub.set_provider_enabled(ProviderId::new("nope"), true));
    drop(hub);

    let again = LyricsHub::new(&dir, Handle::current(), Vec::new()).expect("hub");
    let now = state(&again);
    assert!(now.contains(&("NetEase", true, Granularity::Line)));
    assert!(now.contains(&("Unison", false, Granularity::Syllable)));
    // A provider the user never touched follows the current default.
    assert!(now.contains(&("Better Lyrics", true, Granularity::Syllable)));
    drop(again);
    let _ = std::fs::remove_dir_all(&dir);
}
