//! The SQLite index of the offline cache, `offline.db`, and the names of its folders.
//!
//! The index lists each saved song with the opaque name of its file, its segments, the
//! download queue, the local play counts for smart downloads, saved albums and playlists, and
//! the covers. Every statement is parameterized. Migrations run in order, each once and in its
//! own transaction, and `schema_migration` records when each ran.

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use nemo_core::track::VideoId;
use rusqlite::{Connection, ErrorCode, params};

use super::{Clock, OfflineError, StartupCleanup, is_opaque_name};

mod remove;

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
const MIGRATIONS: [&str; 2] = [MIGRATION_1, MIGRATION_2];

/// `single`: the user saved the song by itself, not only as part of an album or playlist, so
/// removing a collection keeps it (offline-cache task 6).
const MIGRATION_2: &str = "
ALTER TABLE song ADD COLUMN single INTEGER NOT NULL DEFAULT 0;
ALTER TABLE queue ADD COLUMN single INTEGER NOT NULL DEFAULT 0;
";

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

/// The files SQLite keeps beside the index in WAL mode.
const DB_SIDE_FILES: [&str; 2] = ["offline.db-wal", "offline.db-shm"];

/// Why the index could not be opened.
enum OpenError {
    /// The file is not a database or is corrupt: the cache is reset.
    Damaged,
    /// Any other failure, including an index from a newer app. Nothing is deleted.
    Other(OfflineError),
}

impl From<rusqlite::Error> for OpenError {
    fn from(err: rusqlite::Error) -> Self {
        // Only the code is kept: SQLite messages can quote values such as a video id.
        match err.sqlite_error_code() {
            Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => Self::Damaged,
            _ => Self::Other(OfflineError::Db),
        }
    }
}

/// The open index. One connection guarded by a mutex; `None` once [`Store::wipe`] closed it.
pub(super) struct Store {
    conn: Mutex<Option<Connection>>,
}

impl Store {
    /// Open (or create) the index in the cache folder `root`. A damaged index is deleted with
    /// every cache file and a new one is made; the flag says whether that happened.
    pub(super) fn open_or_reset(
        root: &Path,
        clock: &dyn Clock,
    ) -> Result<(Self, bool), OfflineError> {
        let path = root.join(DB_FILE);
        match Self::open(&path, clock) {
            Ok(store) => Ok((store, false)),
            Err(OpenError::Other(err)) => Err(err),
            Err(OpenError::Damaged) => {
                tracing::warn!("the offline index was damaged; the offline cache was reset");
                reset_folder(root)?;
                match Self::open(&path, clock) {
                    Ok(store) => Ok((store, true)),
                    Err(OpenError::Other(err)) => Err(err),
                    Err(OpenError::Damaged) => Err(OfflineError::Db),
                }
            }
        }
    }

    /// Open (or create) the index at `path`, check it and run the migrations it has not seen.
    fn open(path: &Path, clock: &dyn Clock) -> Result<Self, OpenError> {
        let mut conn = Connection::open(path)?;
        // `journal_mode` returns a row, so it cannot go through `execute_batch`.
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
        conn.execute_batch(
            "PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;
        let check: String = conn.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
        if check != "ok" {
            return Err(OpenError::Damaged);
        }
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migration (
                 version    INTEGER PRIMARY KEY NOT NULL,
                 applied_at INTEGER NOT NULL
             );",
        )?;
        migrate(&mut conn, clock)?;
        Ok(Self {
            conn: Mutex::new(Some(conn)),
        })
    }

    /// Start-up cleanup in the cache folder `root`: delete `.part` files that no queue row
    /// names, and mark songs whose file is missing as unsaved.
    pub(super) fn clean_up(&self, root: &Path) -> Result<StartupCleanup, OfflineError> {
        let parts_removed = self.remove_stray_parts(&root.join(TMP))?;
        let unsaved = self.unsave_missing(&root.join(BLOBS))?;
        Ok(StartupCleanup {
            index_reset: false,
            parts_removed,
            unsaved,
        })
    }

    fn remove_stray_parts(&self, tmp: &Path) -> Result<u64, OfflineError> {
        let known: HashSet<String> = self.with_conn(|conn| {
            let mut stmt =
                conn.prepare("SELECT part_file FROM queue WHERE part_file IS NOT NULL")?;
            let names = stmt
                .query_map([], |row| row.get(0))?
                .collect::<Result<_, _>>()?;
            Ok(names)
        })?;
        let mut removed = 0;
        for entry in std::fs::read_dir(tmp)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(stem) = name.to_str().and_then(|n| n.strip_suffix(".part")) else {
                continue;
            };
            if !known.contains(stem) && entry.file_type()?.is_file() {
                std::fs::remove_file(entry.path())?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    fn unsave_missing(&self, blobs: &Path) -> Result<Vec<VideoId>, OfflineError> {
        self.with_conn(|conn| unsave_missing(conn, blobs))
    }

    /// Run `f` on the open connection.
    pub(super) fn with_conn<T>(
        &self,
        f: impl FnOnce(&mut Connection) -> Result<T, OfflineError>,
    ) -> Result<T, OfflineError> {
        // A poisoned lock only means another thread panicked mid-statement; SQLite stays valid.
        let mut guard = self.conn.lock().unwrap_or_else(PoisonError::into_inner);
        let conn = guard.as_mut().ok_or(OfflineError::Closed)?;
        f(conn)
    }

    /// Close the connection. Every later call fails with [`OfflineError::Closed`].
    pub(super) fn close(&self) {
        let mut guard = self.conn.lock().unwrap_or_else(PoisonError::into_inner);
        // Dropping the connection closes it; SQLite checkpoints the WAL on the last close.
        drop(guard.take());
    }

    /// How many songs are saved.
    pub(super) fn saved_count(&self) -> Result<u64, OfflineError> {
        let count: i64 = self.with_conn(|conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM song", [], |row| row.get(0))?)
        })?;
        Ok(u64::try_from(count).unwrap_or(0))
    }
}

/// Mark songs whose file is missing as unsaved; returns their ids.
fn unsave_missing(conn: &mut Connection, blobs: &Path) -> Result<Vec<VideoId>, OfflineError> {
    let rows: Vec<(String, String)> = {
        let mut stmt = conn.prepare("SELECT video_id, file FROM song")?;
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?
    };
    let missing: Vec<String> = rows
        .into_iter()
        .filter(|(_, file)| !is_opaque_name(file) || !blobs.join(file).is_file())
        .map(|(video_id, _)| video_id)
        .collect();
    let tx = conn.transaction()?;
    for video_id in &missing {
        // Segments go with the row (`ON DELETE CASCADE`).
        tx.execute("DELETE FROM song WHERE video_id = ?1", [video_id])?;
    }
    tx.commit()?;
    Ok(missing
        .iter()
        .filter_map(|id| VideoId::parse(id).ok())
        .collect())
}

/// Run every migration newer than the last one recorded. An index written by a newer version
/// of the app is refused rather than guessed at.
fn migrate(conn: &mut Connection, clock: &dyn Clock) -> Result<(), OpenError> {
    let applied: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migration",
        [],
        |row| row.get(0),
    )?;
    let known = i64::try_from(MIGRATIONS.len()).unwrap_or(i64::MAX);
    if applied > known {
        return Err(OpenError::Other(OfflineError::Db));
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

/// Delete the index and every cache file under `root`, then make the empty folders again.
/// The connection to the old index must be closed first.
fn reset_folder(root: &Path) -> Result<(), OfflineError> {
    for name in std::iter::once(DB_FILE).chain(DB_SIDE_FILES) {
        match std::fs::remove_file(root.join(name)) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => return Err(err.into()),
            _ => {}
        }
    }
    for folder in FOLDERS {
        let dir = root.join(folder);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        std::fs::create_dir_all(&dir)?;
    }
    Ok(())
}
