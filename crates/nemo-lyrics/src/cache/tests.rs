//! Unit tests for the lyrics cache.

use nemo_core::lyrics::{Granularity, LyricLine, LyricWord, MatchedMeta};
use std::sync::atomic::{AtomicI64, Ordering};

use super::*;

struct TestClock(AtomicI64);

impl Clock for TestClock {
    fn now_secs(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

const P: ProviderId = ProviderId::new("TestProvider");
const Q: ProviderId = ProviderId::new("OtherProvider");

struct Fixture {
    dir: std::path::PathBuf,
    clock: Arc<TestClock>,
    cache: LyricsCache,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "nemo-lyrics-cache-{name}-{}-{}",
            std::process::id(),
            SystemClock.now_secs()
        ));
        let clock = Arc::new(TestClock(AtomicI64::new(1_000_000)));
        let cache = LyricsCache::open_with_clock(&dir, clock.clone()).expect("open");
        Self { dir, clock, cache }
    }

    fn advance(&self, d: Duration) {
        self.clock.0.fetch_add(secs_i64(d), Ordering::SeqCst);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn sample_doc() -> LyricsDoc {
    LyricsDoc {
        lines: vec![
            LyricLine {
                start_ms: Some(1000),
                end_ms: Some(2500),
                text: "alpha beta".into(),
                words: vec![
                    LyricWord {
                        start_ms: 1000,
                        end_ms: Some(1500),
                        text: "alpha".into(),
                    },
                    LyricWord {
                        start_ms: 1500,
                        end_ms: None,
                        text: "beta".into(),
                    },
                ],
            },
            LyricLine {
                start_ms: None,
                end_ms: None,
                text: "tail \u{65e5}\u{672c}".into(),
                words: vec![],
            },
        ],
        granularity: Granularity::Word,
        provider: P,
        matched: Some(MatchedMeta {
            title: "Test Song".into(),
            artists: vec!["A".into(), "B".into()],
            album: None,
            duration_ms: Some(200_000),
        }),
        instrumental: false,
    }
}

#[test]
fn hit_round_trips_exactly() {
    let f = Fixture::new("roundtrip");
    assert_eq!(f.cache.get("k", P).expect("get"), None);
    f.cache.put_hit("k", P, &sample_doc(), 93).expect("put");
    let entry = f.cache.get("k", P).expect("get").expect("some");
    assert_eq!(
        entry.status,
        EntryStatus::Hit {
            doc: sample_doc(),
            score: 93
        }
    );
    assert_eq!(entry.expires_at - entry.fetched_at, secs_i64(HIT_TTL));
    // Other provider and other track are separate rows.
    assert_eq!(f.cache.get("k", Q).expect("get"), None);
    assert_eq!(f.cache.get("other", P).expect("get"), None);
}

#[test]
fn ttl_hit_30_days_miss_7_days_error_15_minutes() {
    let f = Fixture::new("ttl");
    f.cache.put_hit("hit", P, &sample_doc(), 80).expect("put");
    f.cache.put_miss("miss", P).expect("put");
    f.cache.put_error("err", P, None).expect("put");

    f.advance(Duration::from_secs(ERROR_TTL.as_secs() - 1));
    assert!(f.cache.get("err", P).expect("get").is_some());
    f.advance(Duration::from_secs(1));
    assert!(f.cache.get("err", P).expect("get").is_none());
    assert!(f.cache.get("miss", P).expect("get").is_some());

    f.advance(Duration::from_secs(
        MISS_TTL.as_secs() - ERROR_TTL.as_secs(),
    ));
    assert!(f.cache.get("miss", P).expect("get").is_none());
    assert!(f.cache.get("hit", P).expect("get").is_some());

    f.advance(Duration::from_secs(HIT_TTL.as_secs() - MISS_TTL.as_secs()));
    assert!(f.cache.get("hit", P).expect("get").is_none());
}

#[test]
fn error_follows_longer_retry_after() {
    let f = Fixture::new("retry");
    f.cache
        .put_error("k", P, Some(Duration::from_secs(3600)))
        .expect("put");
    f.advance(Duration::from_secs(3599));
    assert_eq!(
        f.cache.get("k", P).expect("get").map(|e| e.status),
        Some(EntryStatus::Error)
    );
    f.advance(Duration::from_secs(2));
    assert!(f.cache.get("k", P).expect("get").is_none());
    // A short Retry-After never shortens the 15 minute floor.
    f.cache
        .put_error("k", P, Some(Duration::from_secs(1)))
        .expect("put");
    f.advance(Duration::from_secs(60));
    assert!(f.cache.get("k", P).expect("get").is_some());
}

#[test]
fn put_replaces_and_remove_track_clears_one_track() {
    let f = Fixture::new("replace");
    f.cache.put_miss("k", P).expect("put");
    f.cache.put_hit("k", P, &sample_doc(), 70).expect("put");
    assert!(matches!(
        f.cache.get("k", P).expect("get").map(|e| e.status),
        Some(EntryStatus::Hit { .. })
    ));
    f.cache.put_miss("k", Q).expect("put");
    f.cache.put_miss("keep", P).expect("put");
    f.cache.remove_track("k").expect("remove");
    assert!(f.cache.get("k", P).expect("get").is_none());
    assert!(f.cache.get("k", Q).expect("get").is_none());
    assert!(f.cache.get("keep", P).expect("get").is_some());
}

#[test]
fn clear_drops_lyrics_but_keeps_offsets() {
    let f = Fixture::new("clear");
    f.cache.put_hit("k", P, &sample_doc(), 70).expect("put");
    f.cache.set_offset("k", -250).expect("offset");
    f.cache.clear().expect("clear");
    assert!(f.cache.get("k", P).expect("get").is_none());
    assert_eq!(f.cache.offset("k").expect("offset"), -250);
}

#[test]
fn offsets_default_to_zero_and_update() {
    let f = Fixture::new("offset");
    assert_eq!(f.cache.offset("k").expect("offset"), 0);
    f.cache.set_offset("k", 350).expect("set");
    assert_eq!(f.cache.offset("k").expect("offset"), 350);
    f.cache.set_offset("k", -50).expect("set");
    assert_eq!(f.cache.offset("k").expect("offset"), -50);
    f.cache.set_offset("k", 0).expect("set");
    assert_eq!(f.cache.offset("k").expect("offset"), 0);
}

#[test]
fn quotes_in_keys_are_data_not_sql() {
    let f = Fixture::new("sql");
    let key = "x'); DROP TABLE lyrics_cache; --";
    f.cache.put_miss(key, P).expect("put");
    assert!(f.cache.get(key, P).expect("get").is_some());
    f.cache.set_offset(key, 5).expect("set");
    assert_eq!(f.cache.offset(key).expect("offset"), 5);
}

#[test]
fn data_survives_reopen_and_uses_wal() {
    let f = Fixture::new("reopen");
    f.cache.put_hit("k", P, &sample_doc(), 88).expect("put");
    f.cache.set_offset("k", 100).expect("set");
    let again = LyricsCache::open_with_clock(&f.dir, f.clock.clone()).expect("reopen");
    assert!(again.get("k", P).expect("get").is_some());
    assert_eq!(again.offset("k").expect("offset"), 100);
    let mode: String = again
        .lock()
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .expect("pragma");
    assert_eq!(mode.to_lowercase(), "wal");
}

#[test]
fn instrumental_and_unsynced_round_trip() {
    let f = Fixture::new("instr");
    let doc = LyricsDoc {
        lines: vec![],
        granularity: Granularity::Unsynced,
        provider: P,
        matched: None,
        instrumental: true,
    };
    f.cache.put_hit("k", P, &doc, 100).expect("put");
    assert_eq!(
        f.cache.get("k", P).expect("get").map(|e| e.status),
        Some(EntryStatus::Hit { doc, score: 100 })
    );
}

#[test]
fn an_expired_pinned_hit_is_still_served_until_unpinned() {
    let f = Fixture::new("pin");
    f.cache.put_hit("k", P, &sample_doc(), 90).expect("put");
    f.cache.put_hit("other", P, &sample_doc(), 90).expect("put");
    f.cache.set_pinned("k", true).expect("pin");
    f.advance(HIT_TTL + Duration::from_secs(1));
    assert!(matches!(
        f.cache.get("k", P).expect("get").map(|e| e.status),
        Some(EntryStatus::Hit { .. })
    ));
    assert!(f.cache.get("other", P).expect("get").is_none());
    f.cache.set_pinned("k", false).expect("unpin");
    assert!(f.cache.get("k", P).expect("get").is_none());
}

#[test]
fn only_hits_are_pinned() {
    let f = Fixture::new("pinmiss");
    f.cache.put_miss("k", P).expect("put");
    f.cache.put_error("k", Q, None).expect("put");
    f.cache.set_pinned("k", true).expect("pin");
    f.advance(MISS_TTL + Duration::from_secs(1));
    assert!(f.cache.get("k", P).expect("get").is_none());
    assert!(f.cache.get("k", Q).expect("get").is_none());
}

#[test]
fn a_refreshed_hit_keeps_its_pin_and_a_miss_drops_it() {
    let f = Fixture::new("pinput");
    f.cache.put_hit("k", P, &sample_doc(), 90).expect("put");
    f.cache.set_pinned("k", true).expect("pin");
    f.cache.put_hit("k", P, &sample_doc(), 95).expect("refresh");
    f.advance(HIT_TTL + Duration::from_secs(1));
    assert!(f.cache.get("k", P).expect("get").is_some());
    f.cache.put_miss("k", P).expect("miss");
    f.advance(MISS_TTL + Duration::from_secs(1));
    assert!(f.cache.get("k", P).expect("get").is_none());
}

#[test]
fn an_existing_cache_file_gets_the_pinned_flag() {
    let dir = std::env::temp_dir().join(format!(
        "nemo-lyrics-cache-migrate-{}-{}",
        std::process::id(),
        SystemClock.now_secs()
    ));
    std::fs::create_dir_all(&dir).expect("dir");
    {
        // The layout before the pinned flag, with one row in it.
        let old = Connection::open(dir.join(DB_FILE)).expect("old");
        old.execute_batch(
            "CREATE TABLE lyrics_cache (
                 track_key TEXT NOT NULL, provider TEXT NOT NULL, status TEXT NOT NULL,
                 granularity TEXT, body BLOB, score INTEGER,
                 fetched_at INTEGER NOT NULL, expires_at INTEGER NOT NULL,
                 PRIMARY KEY (track_key, provider)
             );
             INSERT INTO lyrics_cache (track_key, provider, status, fetched_at, expires_at)
             VALUES ('k', 'TestProvider', 'miss', 0, 9000000000);",
        )
        .expect("old layout");
    }
    let clock = Arc::new(TestClock(AtomicI64::new(1_000_000)));
    let cache = LyricsCache::open_with_clock(&dir, clock.clone()).expect("open");
    assert_eq!(
        cache.get("k", P).expect("get").map(|e| e.status),
        Some(EntryStatus::Miss)
    );
    cache.set_pinned("k", true).expect("pin");
    let version: i64 = cache
        .lock()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .expect("version");
    assert_eq!(version, 2);
    drop(cache);
    // Opening again runs no change twice.
    let again = LyricsCache::open_with_clock(&dir, clock).expect("reopen");
    assert!(again.get("k", P).expect("get").is_some());
    drop(again);
    let _ = std::fs::remove_dir_all(&dir);
}
