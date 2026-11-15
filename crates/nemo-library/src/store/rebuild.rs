//! Setting a damaged `library.db` aside (criterion 6): `PRAGMA quick_check` on a read-only
//! connection, then a rename of the file with its `-wal` and `-shm` to
//! `library.db.broken-<unix time>`. Nothing is deleted.

use std::path::{Path, PathBuf};

use nemo_core::library::{LibraryError, ScanEvent};
use rusqlite::{Connection, ErrorCode, OpenFlags};

use super::{DB_FILE, db, now_secs};

/// Name of the kept copy of a damaged file, before the unix time: `library.db.broken-<time>`.
pub const BROKEN_PREFIX: &str = "library.db.broken-";

/// What the user reads with [`ScanEvent::Rebuilt`]. It names no path.
pub const REBUILT_NOTICE: &str = "The library file was damaged, so NemoMusic made a new one \
    and is scanning your folders again. The old file was kept beside it as \
    \"library.db.broken-…\"; that copy still holds your old playlists.";

/// A damaged library file was set aside and a new one made. Taken once from
/// [`Store::take_rebuilt`](super::Store::take_rebuilt).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rebuilt {
    /// The kept copy of the damaged file (`library.db.broken-<unix time>`). User data: never
    /// log it.
    pub kept: PathBuf,
}

impl Rebuilt {
    /// The event to send to every scan subscriber.
    #[must_use]
    pub const fn event(&self) -> ScanEvent {
        ScanEvent::Rebuilt
    }

    /// The text to show with the event ([`REBUILT_NOTICE`]).
    #[must_use]
    pub const fn notice(&self) -> &'static str {
        REBUILT_NOTICE
    }
}

/// The state of an existing library file, read before the write connection opens it.
pub(super) enum Health {
    /// Absent or whole.
    Sound,
    /// Damaged; the watched folders that could still be read from it.
    Damaged(Vec<PathBuf>),
}

/// Whether a SQLite failure means the file itself is damaged (not busy, locked or missing).
fn is_damage(e: &rusqlite::Error) -> bool {
    e.sqlite_error_code()
        .is_some_and(|c| matches!(c, ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase))
}

/// Check the file with `PRAGMA quick_check` on a read-only connection, which never writes or
/// checkpoints into the file or its `-wal`, so a damaged file keeps its bytes. SQLite may
/// create or rewrite the `-shm`, its rebuildable index, which holds no library data.
pub(super) fn health(file: &Path) -> Result<Health, LibraryError> {
    if !file.exists() {
        return Ok(Health::Sound);
    }
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(file, flags).map_err(db)?;
    let verdict = conn.query_row("PRAGMA quick_check(1)", [], |r| r.get::<_, String>(0));
    match verdict {
        Ok(text) if text == "ok" => Ok(Health::Sound),
        Ok(_) => Ok(Health::Damaged(salvage_folders(&conn))),
        Err(e) if is_damage(&e) => Ok(Health::Damaged(salvage_folders(&conn))),
        Err(_) => Err(LibraryError::Db),
    }
}

/// The watched folders a damaged file still yields, so the rebuild can rescan them. Best
/// effort: nothing when the folder table cannot be read.
fn salvage_folders(conn: &Connection) -> Vec<PathBuf> {
    // A truncated file is shorter than its header says, which SQLite refuses unless
    // `writable_schema` is on. The connection is read-only, so nothing can be written.
    if conn.execute_batch("PRAGMA writable_schema = ON;").is_err() {
        return Vec::new();
    }
    let Ok(mut stmt) = conn.prepare("SELECT path FROM folder ORDER BY id") else {
        return Vec::new();
    };
    stmt.query_map([], |r| r.get::<_, String>(0))
        .map(|rows| rows.map_while(Result::ok).map(PathBuf::from).collect())
        .unwrap_or_default()
}

/// The file and its `-wal` and `-shm`, as `(from, to)` for the kept copy at `time`.
fn broken_names(dir: &Path, time: i64) -> [(PathBuf, PathBuf); 3] {
    let kept = format!("{BROKEN_PREFIX}{time}");
    ["", "-wal", "-shm"].map(|suffix| {
        (
            dir.join(format!("{DB_FILE}{suffix}")),
            dir.join(format!("{kept}{suffix}")),
        )
    })
}

/// Rename the damaged file with its `-wal` and `-shm` to `library.db.broken-<unix time>`
/// (one second later while that name is taken). Nothing is deleted. Returns the kept file.
pub(super) fn keep_damaged(dir: &Path) -> Result<PathBuf, LibraryError> {
    let start = now_secs();
    let names = (start..start.saturating_add(100))
        .map(|time| broken_names(dir, time))
        .find(|names| names.iter().all(|(_, to)| !to.exists()))
        .ok_or(LibraryError::Io(std::io::ErrorKind::AlreadyExists))?;
    // The `-wal` and `-shm` go first, so a failure leaves the main file where it was.
    for (from, to) in names.iter().rev() {
        if from.exists() {
            std::fs::rename(from, to).map_err(|e| LibraryError::Io(e.kind()))?;
        }
    }
    let [(_, kept), ..] = names;
    Ok(kept)
}
