//! SQLite lyrics cache and per-track offsets (ADR 0002).
//!
//! Two tables in `lyrics.sqlite3` inside the directory given at construction:
//! `lyrics_cache(track_key, provider, status, granularity, body, score, fetched_at, expires_at)`
//! keyed by `(track_key, provider)`, and `lyrics_offset(track_key, offset_ms)`.
//!
//! Time to live: a hit lives 30 days, a miss 7 days, an error 15 minutes (longer when the provider
//! said `Retry-After` for more). Expired rows are never returned. The database runs in WAL mode and
//! every statement is parameterized. Lyrics are copyrighted text: they stay in this local file.
//! [`LyricsCache::clear`] drops the cached lyrics but keeps the user's offsets.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nemo_core::lyrics::{
    Granularity, LyricLine, LyricWord, LyricsCredit, LyricsDoc, MatchedMeta, ProviderId,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

/// Hit lifetime.
pub const HIT_TTL: Duration = Duration::from_hours(30 * 24);
/// Miss (negative cache) lifetime.
pub const MISS_TTL: Duration = Duration::from_hours(7 * 24);
/// Error lifetime (minimum).
pub const ERROR_TTL: Duration = Duration::from_mins(15);

const DB_FILE: &str = "lyrics.sqlite3";

/// A cache operation failed.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    /// SQLite reported an error.
    #[error("lyrics cache database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// The cache directory could not be created.
    #[error("could not create the lyrics cache directory: {0}")]
    Io(#[from] std::io::Error),
    /// A stored body could not be encoded or decoded.
    #[error("lyrics cache body error: {0}")]
    Body(String),
}

/// Source of the current time, so tests can move it.
pub trait Clock: Send + Sync {
    /// Seconds since the Unix epoch.
    fn now_secs(&self) -> i64;
}

/// The system clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_secs(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
    }
}

/// What a cached row says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryStatus {
    /// The provider had lyrics.
    Hit {
        /// The lyrics.
        doc: LyricsDoc,
        /// Match score, 0 to 100.
        score: u8,
    },
    /// The provider had no lyrics.
    Miss,
    /// The provider failed recently.
    Error,
}

/// A fresh (not expired) cache row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheEntry {
    /// What the row says.
    pub status: EntryStatus,
    /// When it was stored, Unix seconds.
    pub fetched_at: i64,
    /// When it stops being valid, Unix seconds.
    pub expires_at: i64,
}

// Private serde mirror of the nemo-core types, which carry no serde. The provider is not stored:
// it is the row's `provider` column and is restored from the id the caller asks with.

#[derive(Serialize, Deserialize)]
enum GranularityRec {
    Unsynced,
    Line,
    Word,
    Syllable,
}

#[derive(Serialize, Deserialize)]
struct WordRec {
    start_ms: u64,
    end_ms: Option<u64>,
    text: String,
}

#[derive(Serialize, Deserialize)]
struct LineRec {
    start_ms: Option<u64>,
    end_ms: Option<u64>,
    text: String,
    words: Vec<WordRec>,
}

#[derive(Serialize, Deserialize)]
struct MatchedRec {
    title: String,
    artists: Vec<String>,
    album: Option<String>,
    duration_ms: Option<u64>,
}

#[derive(Serialize, Deserialize)]
struct DocRec {
    lines: Vec<LineRec>,
    granularity: GranularityRec,
    matched: Option<MatchedRec>,
    instrumental: bool,
}

impl From<Granularity> for GranularityRec {
    fn from(g: Granularity) -> Self {
        match g {
            Granularity::Unsynced => Self::Unsynced,
            Granularity::Line => Self::Line,
            Granularity::Word => Self::Word,
            Granularity::Syllable => Self::Syllable,
        }
    }
}

impl From<GranularityRec> for Granularity {
    fn from(g: GranularityRec) -> Self {
        match g {
            GranularityRec::Unsynced => Self::Unsynced,
            GranularityRec::Line => Self::Line,
            GranularityRec::Word => Self::Word,
            GranularityRec::Syllable => Self::Syllable,
        }
    }
}

fn granularity_name(g: Granularity) -> &'static str {
    match g {
        Granularity::Unsynced => "unsynced",
        Granularity::Line => "line",
        Granularity::Word => "word",
        Granularity::Syllable => "syllable",
    }
}

fn encode_doc(doc: &LyricsDoc) -> Result<Vec<u8>, CacheError> {
    let rec = DocRec {
        lines: doc
            .lines
            .iter()
            .map(|l| LineRec {
                start_ms: l.start_ms,
                end_ms: l.end_ms,
                text: l.text.clone(),
                words: l
                    .words
                    .iter()
                    .map(|w| WordRec {
                        start_ms: w.start_ms,
                        end_ms: w.end_ms,
                        text: w.text.clone(),
                    })
                    .collect(),
            })
            .collect(),
        granularity: doc.granularity.into(),
        matched: doc.matched.as_ref().map(|m| MatchedRec {
            title: m.title.clone(),
            artists: m.artists.clone(),
            album: m.album.clone(),
            duration_ms: m.duration_ms,
        }),
        instrumental: doc.instrumental,
    };
    serde_json::to_vec(&rec).map_err(|e| CacheError::Body(e.to_string()))
}

fn decode_doc(body: &[u8], provider: ProviderId) -> Result<LyricsDoc, CacheError> {
    let rec: DocRec = serde_json::from_slice(body).map_err(|e| CacheError::Body(e.to_string()))?;
    Ok(LyricsDoc {
        lines: rec
            .lines
            .into_iter()
            .map(|l| LyricLine {
                start_ms: l.start_ms,
                end_ms: l.end_ms,
                text: l.text,
                words: l
                    .words
                    .into_iter()
                    .map(|w| LyricWord {
                        start_ms: w.start_ms,
                        end_ms: w.end_ms,
                        text: w.text,
                    })
                    .collect(),
            })
            .collect(),
        granularity: rec.granularity.into(),
        provider,
        matched: rec.matched.map(|m| MatchedMeta {
            title: m.title,
            artists: m.artists,
            album: m.album,
            duration_ms: m.duration_ms,
        }),
        instrumental: rec.instrumental,
    })
}

fn secs_i64(d: Duration) -> i64 {
    i64::try_from(d.as_secs()).unwrap_or(i64::MAX)
}

/// The lyrics cache. Cheap to share behind an `Arc`; one connection guarded by a mutex.
pub struct LyricsCache {
    conn: Mutex<Connection>,
    clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for LyricsCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LyricsCache").finish_non_exhaustive()
    }
}

impl LyricsCache {
    /// Open (or create) the cache in `dir` with the system clock.
    ///
    /// # Errors
    /// [`CacheError`] if the directory or database cannot be created.
    pub fn open(dir: &Path) -> Result<Self, CacheError> {
        Self::open_with_clock(dir, Arc::new(SystemClock))
    }

    /// Open (or create) the cache in `dir` with a given clock.
    ///
    /// # Errors
    /// [`CacheError`] if the directory or database cannot be created.
    pub fn open_with_clock(dir: &Path, clock: Arc<dyn Clock>) -> Result<Self, CacheError> {
        std::fs::create_dir_all(dir)?;
        let conn = Connection::open(dir.join(DB_FILE))?;
        Self::init(conn, clock)
    }

    fn init(conn: Connection, clock: Arc<dyn Clock>) -> Result<Self, CacheError> {
        // `journal_mode` returns a row, so it cannot go through `execute_batch`.
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
        conn.execute_batch(
            "PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS lyrics_cache (
                 track_key   TEXT NOT NULL,
                 provider    TEXT NOT NULL,
                 status      TEXT NOT NULL,
                 granularity TEXT,
                 body        BLOB,
                 score       INTEGER,
                 fetched_at  INTEGER NOT NULL,
                 expires_at  INTEGER NOT NULL,
                 PRIMARY KEY (track_key, provider)
             );
             CREATE TABLE IF NOT EXISTS lyrics_offset (
                 track_key TEXT PRIMARY KEY NOT NULL,
                 offset_ms INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS lyrics_choice (
                 track_key TEXT PRIMARY KEY NOT NULL,
                 provider  TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS lyrics_credit (
                 track_key TEXT NOT NULL,
                 provider  TEXT NOT NULL,
                 label     TEXT NOT NULL,
                 url       TEXT,
                 by_name   TEXT,
                 by_url    TEXT,
                 note      TEXT,
                 PRIMARY KEY (track_key, provider)
             );
             CREATE TABLE IF NOT EXISTS provider_setting (
                 provider TEXT PRIMARY KEY NOT NULL,
                 enabled  INTEGER NOT NULL
             );",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
            clock,
        })
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        // A poisoned lock only means another thread panicked mid-statement; SQLite stays valid.
        self.conn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The fresh row for a track and provider, or `None` if absent or expired.
    ///
    /// # Errors
    /// [`CacheError`] on database or body errors.
    pub fn get(
        &self,
        track_key: &str,
        provider: ProviderId,
    ) -> Result<Option<CacheEntry>, CacheError> {
        let now = self.clock.now_secs();
        let row = self
            .lock()
            .query_row(
                "SELECT status, body, score, fetched_at, expires_at FROM lyrics_cache
                 WHERE track_key = ?1 AND provider = ?2 AND expires_at > ?3",
                params![track_key, provider.as_str(), now],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<Vec<u8>>>(1)?,
                        r.get::<_, Option<i64>>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, i64>(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((status, body, score, fetched_at, expires_at)) = row else {
            return Ok(None);
        };
        let status = match status.as_str() {
            "hit" => {
                let body = body.ok_or_else(|| CacheError::Body("hit without body".to_owned()))?;
                EntryStatus::Hit {
                    doc: decode_doc(&body, provider)?,
                    score: u8::try_from(score.unwrap_or(0).clamp(0, 100)).unwrap_or(0),
                }
            }
            "miss" => EntryStatus::Miss,
            "error" => EntryStatus::Error,
            other => return Err(CacheError::Body(format!("unknown status {other:?}"))),
        };
        Ok(Some(CacheEntry {
            status,
            fetched_at,
            expires_at,
        }))
    }

    fn put(
        &self,
        track_key: &str,
        provider: ProviderId,
        status: &str,
        doc: Option<(&LyricsDoc, u8)>,
        ttl: Duration,
    ) -> Result<(), CacheError> {
        let now = self.clock.now_secs();
        let expires = now.saturating_add(secs_i64(ttl));
        let (granularity, body, score) = match doc {
            Some((d, s)) => (
                Some(granularity_name(d.granularity)),
                Some(encode_doc(d)?),
                Some(i64::from(s)),
            ),
            None => (None, None, None),
        };
        self.lock().execute(
            "INSERT OR REPLACE INTO lyrics_cache
                 (track_key, provider, status, granularity, body, score, fetched_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                track_key,
                provider.as_str(),
                status,
                granularity,
                body,
                score,
                now,
                expires
            ],
        )?;
        Ok(())
    }

    /// Store lyrics for 30 days.
    ///
    /// # Errors
    /// [`CacheError`] on database or body errors.
    pub fn put_hit(
        &self,
        track_key: &str,
        provider: ProviderId,
        doc: &LyricsDoc,
        score: u8,
    ) -> Result<(), CacheError> {
        self.put(track_key, provider, "hit", Some((doc, score)), HIT_TTL)
    }

    /// Remember for 7 days that the provider has nothing.
    ///
    /// # Errors
    /// [`CacheError`] on database errors.
    pub fn put_miss(&self, track_key: &str, provider: ProviderId) -> Result<(), CacheError> {
        self.put(track_key, provider, "miss", None, MISS_TTL)
    }

    /// Remember a provider failure for 15 minutes, or for `retry_after` if that is longer.
    ///
    /// # Errors
    /// [`CacheError`] on database errors.
    pub fn put_error(
        &self,
        track_key: &str,
        provider: ProviderId,
        retry_after: Option<Duration>,
    ) -> Result<(), CacheError> {
        let ttl = retry_after.map_or(ERROR_TTL, |r| r.max(ERROR_TTL));
        self.put(track_key, provider, "error", None, ttl)
    }

    /// Forget everything cached for one track ("refetch lyrics"). The offset stays.
    ///
    /// # Errors
    /// [`CacheError`] on database errors.
    pub fn remove_track(&self, track_key: &str) -> Result<(), CacheError> {
        let conn = self.lock();
        conn.execute(
            "DELETE FROM lyrics_cache WHERE track_key = ?1",
            params![track_key],
        )?;
        conn.execute(
            "DELETE FROM lyrics_credit WHERE track_key = ?1",
            params![track_key],
        )?;
        Ok(())
    }

    /// Drop all cached lyrics, misses and errors. Per-track offsets are user settings and stay.
    ///
    /// # Errors
    /// [`CacheError`] on database errors.
    pub fn clear(&self) -> Result<(), CacheError> {
        let conn = self.lock();
        conn.execute("DELETE FROM lyrics_cache", [])?;
        conn.execute("DELETE FROM lyrics_credit", [])?;
        Ok(())
    }

    /// Store the credit a source asks for next to the lyrics it returned.
    ///
    /// # Errors
    /// [`CacheError`] on database errors.
    pub fn put_credit(
        &self,
        track_key: &str,
        provider: ProviderId,
        credit: &LyricsCredit,
    ) -> Result<(), CacheError> {
        self.lock().execute(
            "INSERT OR REPLACE INTO lyrics_credit
                 (track_key, provider, label, url, by_name, by_url, note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                track_key,
                provider.as_str(),
                credit.label,
                credit.url,
                credit.by,
                credit.by_url,
                credit.note
            ],
        )?;
        Ok(())
    }

    /// The stored credit for a track and provider, if any.
    ///
    /// # Errors
    /// [`CacheError`] on database errors.
    pub fn credit(
        &self,
        track_key: &str,
        provider: ProviderId,
    ) -> Result<Option<LyricsCredit>, CacheError> {
        Ok(self
            .lock()
            .query_row(
                "SELECT label, url, by_name, by_url, note FROM lyrics_credit
                 WHERE track_key = ?1 AND provider = ?2",
                params![track_key, provider.as_str()],
                |r| {
                    Ok(LyricsCredit {
                        label: r.get(0)?,
                        url: r.get(1)?,
                        by: r.get(2)?,
                        by_url: r.get(3)?,
                        note: r.get(4)?,
                    })
                },
            )
            .optional()?)
    }

    /// Remember the user's pick of provider for a track. It is a user setting: refetching and
    /// clearing the cache keep it, like offsets.
    ///
    /// # Errors
    /// [`CacheError`] on database errors.
    pub fn set_choice(&self, track_key: &str, provider: ProviderId) -> Result<(), CacheError> {
        self.lock().execute(
            "INSERT OR REPLACE INTO lyrics_choice (track_key, provider) VALUES (?1, ?2)",
            params![track_key, provider.as_str()],
        )?;
        Ok(())
    }

    /// The provider name the user picked for a track, if any.
    ///
    /// # Errors
    /// [`CacheError`] on database errors.
    pub fn choice(&self, track_key: &str) -> Result<Option<String>, CacheError> {
        Ok(self
            .lock()
            .query_row(
                "SELECT provider FROM lyrics_choice WHERE track_key = ?1",
                params![track_key],
                |r| r.get::<_, String>(0),
            )
            .optional()?)
    }

    /// Forget the user's pick for a track.
    ///
    /// # Errors
    /// [`CacheError`] on database errors.
    pub fn clear_choice(&self, track_key: &str) -> Result<(), CacheError> {
        self.lock().execute(
            "DELETE FROM lyrics_choice WHERE track_key = ?1",
            params![track_key],
        )?;
        Ok(())
    }

    /// Store whether the user switched a provider on or off.
    ///
    /// # Errors
    /// [`CacheError`] on database errors.
    pub fn set_provider_enabled(
        &self,
        provider: ProviderId,
        enabled: bool,
    ) -> Result<(), CacheError> {
        self.lock().execute(
            "INSERT OR REPLACE INTO provider_setting (provider, enabled) VALUES (?1, ?2)",
            params![provider.as_str(), enabled],
        )?;
        Ok(())
    }

    /// What the user chose for a provider, None if they never changed it.
    ///
    /// # Errors
    /// [`CacheError`] on database errors.
    pub fn provider_enabled(&self, provider: ProviderId) -> Result<Option<bool>, CacheError> {
        Ok(self
            .lock()
            .query_row(
                "SELECT enabled FROM provider_setting WHERE provider = ?1",
                params![provider.as_str()],
                |r| r.get::<_, bool>(0),
            )
            .optional()?)
    }

    /// The stored offset for a track in milliseconds (0 when none).
    ///
    /// # Errors
    /// [`CacheError`] on database errors.
    pub fn offset(&self, track_key: &str) -> Result<i64, CacheError> {
        Ok(self
            .lock()
            .query_row(
                "SELECT offset_ms FROM lyrics_offset WHERE track_key = ?1",
                params![track_key],
                |r| r.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0))
    }

    /// Store the offset for a track. An offset of 0 removes the row.
    ///
    /// # Errors
    /// [`CacheError`] on database errors.
    pub fn set_offset(&self, track_key: &str, offset_ms: i64) -> Result<(), CacheError> {
        let conn = self.lock();
        if offset_ms == 0 {
            conn.execute(
                "DELETE FROM lyrics_offset WHERE track_key = ?1",
                params![track_key],
            )?;
        } else {
            conn.execute(
                "INSERT OR REPLACE INTO lyrics_offset (track_key, offset_ms) VALUES (?1, ?2)",
                params![track_key, offset_ms],
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
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
}
