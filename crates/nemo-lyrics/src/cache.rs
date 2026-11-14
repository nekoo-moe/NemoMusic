//! SQLite lyrics cache and per-track offsets (ADR 0002).
//!
//! Two tables in `lyrics.sqlite3` inside the directory given at construction:
//! `lyrics_cache(track_key, provider, status, granularity, body, score, fetched_at, expires_at,
//! pinned)` keyed by `(track_key, provider)`, and `lyrics_offset(track_key, offset_ms)`.
//!
//! Time to live: a hit lives 30 days, a miss 7 days, an error 15 minutes (longer when the provider
//! said `Retry-After` for more). Expired rows are never returned, except a pinned hit, which is kept
//! for a song saved offline and never expires. The database runs in WAL mode and
//! every statement is parameterized. Lyrics are copyrighted text: they stay in this local file.
//! [`LyricsCache::clear`] drops the cached lyrics but keeps the user's offsets.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use nemo_core::lyrics::{LyricsCredit, LyricsDoc, ProviderId};
use rusqlite::{Connection, OptionalExtension, params};

/// Hit lifetime.
pub const HIT_TTL: Duration = Duration::from_hours(30 * 24);
/// Miss (negative cache) lifetime.
pub const MISS_TTL: Duration = Duration::from_hours(7 * 24);
/// Error lifetime (minimum).
pub const ERROR_TTL: Duration = Duration::from_mins(15);

const DB_FILE: &str = "lyrics.sqlite3";

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

    fn init(mut conn: Connection, clock: Arc<dyn Clock>) -> Result<Self, CacheError> {
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
        migrate::run(&mut conn)?;
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

    /// The fresh row for a track and provider, or `None` if absent or expired. A pinned row
    /// (see [`LyricsCache::set_pinned`]) never expires.
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
                 WHERE track_key = ?1 AND provider = ?2 AND (expires_at > ?3 OR pinned = 1)",
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
        // An upsert, not a replace, so a refreshed hit keeps its pin; anything else drops it.
        self.lock().execute(
            "INSERT INTO lyrics_cache
                 (track_key, provider, status, granularity, body, score, fetched_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT (track_key, provider) DO UPDATE SET
                 status = excluded.status, granularity = excluded.granularity,
                 body = excluded.body, score = excluded.score,
                 fetched_at = excluded.fetched_at, expires_at = excluded.expires_at,
                 pinned = lyrics_cache.pinned AND excluded.status = 'hit'",
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

    /// Keep the found lyrics of one track from expiring (`keep` true), for a song saved
    /// offline, or let them expire again (`keep` false). Only hits are pinned: a pinned miss or
    /// error would stop the track from ever being asked again.
    ///
    /// # Errors
    /// [`CacheError`] on database errors.
    pub fn set_pinned(&self, track_key: &str, keep: bool) -> Result<(), CacheError> {
        self.lock().execute(
            "UPDATE lyrics_cache SET pinned = ?2 WHERE track_key = ?1 AND (status = 'hit' OR ?2 = 0)",
            params![track_key, keep],
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

mod clock;
mod error;
mod migrate;
mod record;

pub use clock::{Clock, SystemClock};
pub use error::CacheError;
use record::{decode_doc, encode_doc, granularity_name};

#[cfg(test)]
mod tests;
