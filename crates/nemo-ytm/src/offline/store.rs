//! The SQLite index of the offline cache, `offline.db`, and the names of its folders.
//!
//! The index lists each saved song with the opaque name of its file, its segments, the
//! download queue, the local play counts for smart downloads, saved albums and playlists, and
//! the covers. Every statement is parameterized. Migrations run in order, each once and in its
//! own transaction, and `schema_migration` records when each ran.

use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use rusqlite::{Connection, params};

use super::{Clock, OfflineError};

/// The index file inside the cache folder.
pub(super) const DB_FILE: &str = "offline.db";
/// Saved songs.
pub(super) const BLOBS: &str = "blobs";
/// Downloads in progress.
pub(super) const TMP: &str = "tmp";
/// Covers.
pub(super) const ART: &str = "art";
/// Every folder the cache creates.
pub(super) const FOLDERS: [&str; 3] = [BLOBS, TMP, ART];

/// Migrations in order; migration `n` is at index `n - 1`.
const MIGRATIONS: [&str; 1] = [MIGRATION_1];

/// The first layout. File columns hold opaque names only, never a path.
const MIGRATION_1: &str = "
CREATE TABLE art (
    id     INTEGER PRIMARY KEY,
    source TEXT NOT NULL UNIQUE,
    file   TEXT NOT NULL UNIQUE,
    bytes  INTEGER NOT NULL
);
CREATE TABLE song (
    video_id     TEXT PRIMARY KEY NOT NULL,
    file         TEXT NOT NULL UNIQUE,
    bytes        INTEGER NOT NULL,
    codec        TEXT NOT NULL,
    bitrate_kbps INTEGER,
    reason       TEXT NOT NULL CHECK (reason IN ('user', 'smart')),
    meta         TEXT NOT NULL,
    art_id       INTEGER REFERENCES art(id) ON DELETE SET NULL,
    saved_at     INTEGER NOT NULL,
    checked_at   INTEGER,
    unavailable  INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE segment (
    video_id   TEXT NOT NULL REFERENCES song(video_id) ON DELETE CASCADE,
    start_ms   INTEGER NOT NULL,
    end_ms     INTEGER NOT NULL,
    category   TEXT NOT NULL,
    segment_id TEXT NOT NULL
);
CREATE INDEX segment_video ON segment(video_id);
CREATE TABLE queue (
    video_id    TEXT PRIMARY KEY NOT NULL,
    position    INTEGER NOT NULL,
    reason      TEXT NOT NULL CHECK (reason IN ('user', 'smart')),
    state       TEXT NOT NULL CHECK (state IN ('queued', 'saving', 'failed')),
    fail_reason TEXT,
    part_file   TEXT UNIQUE,
    done_bytes  INTEGER NOT NULL DEFAULT 0,
    total_bytes INTEGER,
    meta        TEXT NOT NULL,
    added_at    INTEGER NOT NULL
);
CREATE TABLE play_count (
    video_id    TEXT PRIMARY KEY NOT NULL,
    plays       INTEGER NOT NULL DEFAULT 0,
    last_played INTEGER NOT NULL
);
CREATE TABLE collection (
    id        INTEGER PRIMARY KEY,
    kind      TEXT NOT NULL CHECK (kind IN ('album', 'playlist')),
    browse_id TEXT NOT NULL,
    title     TEXT NOT NULL,
    saved_at  INTEGER NOT NULL,
    UNIQUE (kind, browse_id)
);
CREATE TABLE collection_song (
    collection_id INTEGER NOT NULL REFERENCES collection(id) ON DELETE CASCADE,
    video_id      TEXT NOT NULL,
    position      INTEGER NOT NULL,
    PRIMARY KEY (collection_id, video_id)
);
";

/// The open index. One connection guarded by a mutex.
pub(super) struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    /// Open (or create) the index at `path` and run the migrations it has not seen.
    pub(super) fn open(path: &Path, clock: &dyn Clock) -> Result<Self, OfflineError> {
        let mut conn = Connection::open(path)?;
        // `journal_mode` returns a row, so it cannot go through `execute_batch`.
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
        conn.execute_batch(
            "PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS schema_migration (
                 version    INTEGER PRIMARY KEY NOT NULL,
                 applied_at INTEGER NOT NULL
             );",
        )?;
        migrate(&mut conn, clock)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        // A poisoned lock only means another thread panicked mid-statement; SQLite stays valid.
        self.conn.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// How many songs are saved.
    pub(super) fn saved_count(&self) -> Result<u64, OfflineError> {
        let count: i64 = self
            .lock()
            .query_row("SELECT COUNT(*) FROM song", [], |row| row.get(0))?;
        Ok(u64::try_from(count).unwrap_or(0))
    }
}

/// Run every migration newer than the last one recorded. An index written by a newer version
/// of the app is refused rather than guessed at.
fn migrate(conn: &mut Connection, clock: &dyn Clock) -> Result<(), OfflineError> {
    let applied: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migration",
        [],
        |row| row.get(0),
    )?;
    let known = i64::try_from(MIGRATIONS.len()).unwrap_or(i64::MAX);
    if applied > known {
        return Err(OfflineError::Db);
    }
    for (version, sql) in (1_i64..).zip(MIGRATIONS) {
        if version <= applied {
            continue;
        }
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT INTO schema_migration (version, applied_at) VALUES (?1, ?2)",
            params![version, clock.now_secs()],
        )?;
        tx.commit()?;
    }
    Ok(())
}
