//! The library database, `library.db`: folders, tracks, albums, local playlists and loudness.
//!
//! One file in the folder `nemo-app` gives (`AppPaths.library`), in WAL mode so readers never
//! wait for the writer. The scan's writer thread holds the only write connection
//! ([`Store::open`]); every reader (the start-up load, the prepare thread's playback lookup)
//! opens its own read-only one ([`Store::open_reader`]). Migrations run in order, each once
//! and in its own transaction, and `schema_migration` records them. Every statement is
//! parameterized. Paths and tags are user data: they are stored here and never logged, and
//! errors carry no path or tag ([`LibraryError`]).

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use nemo_core::library::{LibraryError, Loudness};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};

mod playlists;
mod tracks;

pub use playlists::NewEntry;
pub use tracks::TrackRow;

/// The database file inside the library folder.
pub const DB_FILE: &str = "library.db";

/// Migrations in order; migration `n` is at index `n - 1`.
const MIGRATIONS: [&str; 1] = [MIGRATION_1];

/// The first layout. A track is one file, or one track of a CUE sheet (`cue_index` from 1;
/// 0 is the whole file). Playlist entries name a file by path, not a track row, so they
/// survive rescans; a streamed song is stored as JSON in `stream`.
const MIGRATION_1: &str = "
CREATE TABLE folder (
    id       INTEGER PRIMARY KEY,
    path     TEXT NOT NULL UNIQUE,
    added_at INTEGER NOT NULL
);
CREATE TABLE album (
    id           INTEGER PRIMARY KEY,
    title        TEXT NOT NULL,
    album_artist TEXT NOT NULL DEFAULT '',
    gapless      INTEGER NOT NULL DEFAULT 0,
    UNIQUE (title, album_artist)
);
CREATE TABLE track (
    id            INTEGER PRIMARY KEY,
    folder_id     INTEGER NOT NULL REFERENCES folder(id) ON DELETE CASCADE,
    path          TEXT NOT NULL,
    cue_index     INTEGER NOT NULL DEFAULT 0,
    size          INTEGER NOT NULL,
    modified_ms   INTEGER NOT NULL,
    title         TEXT NOT NULL,
    artists       TEXT NOT NULL,
    album_id      INTEGER REFERENCES album(id) ON DELETE SET NULL,
    album_artist  TEXT,
    disc          INTEGER,
    number        INTEGER,
    year          INTEGER,
    genre         TEXT,
    duration_ms   INTEGER,
    status        TEXT NOT NULL CHECK (status IN ('playable', 'unplayable', 'missing')),
    reason        TEXT CHECK (reason IN ('unsupported', 'decode', 'unreadable')),
    span_start_ms INTEGER,
    span_end_ms   INTEGER,
    track_gain_db REAL,
    album_gain_db REAL,
    track_peak    REAL,
    album_peak    REAL,
    UNIQUE (path, cue_index)
);
CREATE INDEX track_folder ON track(folder_id);
CREATE TABLE playlist (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE TABLE playlist_entry (
    playlist_id INTEGER NOT NULL REFERENCES playlist(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    path        TEXT,
    cue_index   INTEGER NOT NULL DEFAULT 0,
    stream      TEXT,
    PRIMARY KEY (playlist_id, position),
    CHECK (path IS NOT NULL OR stream IS NOT NULL)
);
CREATE TABLE loudness (
    path            TEXT PRIMARY KEY NOT NULL,
    size            INTEGER NOT NULL,
    modified_ms     INTEGER NOT NULL,
    integrated_lufs REAL NOT NULL,
    true_peak       REAL NOT NULL
);
";

/// Any SQLite failure. Only the kind survives: SQLite messages can quote a path or a tag.
pub(crate) fn db<E>(_: E) -> LibraryError {
    LibraryError::Db
}

/// A path as stored text. A path that is not valid Unicode cannot be stored.
pub(crate) fn path_text(path: &Path) -> Result<&str, LibraryError> {
    path.to_str()
        .ok_or(LibraryError::Io(std::io::ErrorKind::InvalidData))
}

/// Seconds since the Unix epoch.
fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// One connection to `library.db`.
pub struct Store {
    conn: Connection,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store").finish_non_exhaustive()
    }
}

impl Store {
    /// Open (or create) the write connection in `dir`: WAL mode, foreign keys, and every
    /// migration the file has not seen. Only the scan's writer thread holds one.
    ///
    /// # Errors
    /// [`LibraryError::Io`] when the folder cannot be created, [`LibraryError::Db`] when the
    /// file cannot be opened or migrated, or comes from a newer app (it is kept, not changed).
    pub fn open(dir: &Path) -> Result<Self, LibraryError> {
        std::fs::create_dir_all(dir).map_err(|e| LibraryError::Io(e.kind()))?;
        let mut conn = Connection::open(dir.join(DB_FILE)).map_err(db)?;
        // `journal_mode` returns a row, so it cannot go through `execute_batch`.
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))
            .map_err(db)?;
        conn.execute_batch(
            "PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS schema_migration (
                 version    INTEGER PRIMARY KEY NOT NULL,
                 applied_at INTEGER NOT NULL
             );",
        )
        .map_err(db)?;
        migrate(&mut conn)?;
        Ok(Self { conn })
    }

    /// Open a read-only connection to the library in `dir`, for one reader thread. The write
    /// connection must have created the file first.
    ///
    /// # Errors
    /// [`LibraryError::Db`] when the file is missing or cannot be opened.
    pub fn open_reader(dir: &Path) -> Result<Self, LibraryError> {
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let conn = Connection::open_with_flags(dir.join(DB_FILE), flags).map_err(db)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(db)?;
        Ok(Self { conn })
    }

    /// The watched folders, in the order they were added.
    ///
    /// # Errors
    /// [`LibraryError::Db`] on a database failure.
    pub fn folders(&self) -> Result<Vec<PathBuf>, LibraryError> {
        let mut stmt = self
            .conn
            .prepare("SELECT path FROM folder ORDER BY id")
            .map_err(db)?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db)?
            .map(|r| r.map(PathBuf::from))
            .collect::<Result<_, _>>()
            .map_err(db)?;
        Ok(rows)
    }

    /// Watch a folder. Adding one that is already watched changes nothing.
    ///
    /// # Errors
    /// [`LibraryError::Io`] for a path that is not valid Unicode, or [`LibraryError::Db`].
    pub fn add_folder(&mut self, path: &Path) -> Result<(), LibraryError> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO folder (path, added_at) VALUES (?1, ?2)",
                params![path_text(path)?, now_secs()],
            )
            .map_err(db)?;
        Ok(())
    }

    /// Stop watching a folder: its row and every track row under it go. No file is touched.
    /// Returns how many tracks were removed.
    ///
    /// # Errors
    /// [`LibraryError::Io`] for a path that is not valid Unicode, or [`LibraryError::Db`].
    pub fn remove_folder(&mut self, path: &Path) -> Result<usize, LibraryError> {
        let text = path_text(path)?;
        let tx = self.conn.transaction().map_err(db)?;
        let removed = tx
            .execute(
                "DELETE FROM track WHERE folder_id = (SELECT id FROM folder WHERE path = ?1)",
                [text],
            )
            .map_err(db)?;
        tx.execute("DELETE FROM folder WHERE path = ?1", [text])
            .map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(removed)
    }

    /// Mark an album gapless (or not), from the iTunes `pgap` tag or the user's choice.
    ///
    /// # Errors
    /// [`LibraryError::Db`] on a database failure.
    pub fn set_gapless(
        &mut self,
        title: &str,
        album_artist: &str,
        gapless: bool,
    ) -> Result<(), LibraryError> {
        self.conn
            .execute(
                "INSERT INTO album (title, album_artist, gapless) VALUES (?1, ?2, ?3)
                 ON CONFLICT (title, album_artist) DO UPDATE SET gapless = excluded.gapless",
                params![title, album_artist, gapless],
            )
            .map_err(db)?;
        Ok(())
    }

    /// Whether an album is marked gapless; `false` for an album the library does not know.
    ///
    /// # Errors
    /// [`LibraryError::Db`] on a database failure.
    pub fn gapless(&self, title: &str, album_artist: &str) -> Result<bool, LibraryError> {
        Ok(self
            .conn
            .query_row(
                "SELECT gapless FROM album WHERE title = ?1 AND album_artist = ?2",
                params![title, album_artist],
                |r| r.get::<_, bool>(0),
            )
            .optional()
            .map_err(db)?
            .unwrap_or(false))
    }

    /// Store the measured loudness of a file, for its current size and modified time.
    ///
    /// # Errors
    /// [`LibraryError::Io`] for a path that is not valid Unicode, or [`LibraryError::Db`].
    pub fn put_loudness(
        &mut self,
        path: &Path,
        stamp: (u64, i64),
        loudness: Loudness,
    ) -> Result<(), LibraryError> {
        let size = i64::try_from(stamp.0).unwrap_or(i64::MAX);
        self.conn
            .execute(
                "INSERT OR REPLACE INTO loudness
                     (path, size, modified_ms, integrated_lufs, true_peak)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    path_text(path)?,
                    size,
                    stamp.1,
                    f64::from(loudness.integrated_lufs),
                    f64::from(loudness.true_peak)
                ],
            )
            .map_err(db)?;
        Ok(())
    }

    /// The stored loudness of a file, only when its size and modified time (`stamp`) still
    /// match the measured file.
    ///
    /// # Errors
    /// [`LibraryError::Io`] for a path that is not valid Unicode, or [`LibraryError::Db`].
    pub fn loudness(
        &self,
        path: &Path,
        stamp: (u64, i64),
    ) -> Result<Option<Loudness>, LibraryError> {
        let size = i64::try_from(stamp.0).unwrap_or(i64::MAX);
        self.conn
            .query_row(
                "SELECT integrated_lufs, true_peak FROM loudness
                 WHERE path = ?1 AND size = ?2 AND modified_ms = ?3",
                params![path_text(path)?, size, stamp.1],
                |r| {
                    Ok(Loudness {
                        integrated_lufs: tracks::narrow(r.get(0)?),
                        true_peak: tracks::narrow(r.get(1)?),
                    })
                },
            )
            .optional()
            .map_err(db)
    }
}

/// Run every migration newer than the last one recorded. A file written by a newer version
/// of the app is refused rather than guessed at.
fn migrate(conn: &mut Connection) -> Result<(), LibraryError> {
    let applied: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migration",
            [],
            |r| r.get(0),
        )
        .map_err(db)?;
    let known = i64::try_from(MIGRATIONS.len()).unwrap_or(i64::MAX);
    if applied > known {
        return Err(LibraryError::Db);
    }
    for (version, sql) in (1_i64..).zip(MIGRATIONS) {
        if version <= applied {
            continue;
        }
        let tx = conn.transaction().map_err(db)?;
        tx.execute_batch(sql).map_err(db)?;
        tx.execute(
            "INSERT INTO schema_migration (version, applied_at) VALUES (?1, ?2)",
            params![version, now_secs()],
        )
        .map_err(db)?;
        tx.commit().map_err(db)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
