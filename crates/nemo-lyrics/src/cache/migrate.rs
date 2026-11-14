//! Changes to the lyrics cache layout after the first one, each run once at open.
//!
//! `PRAGMA user_version` records the last change applied. A file from before this module has
//! version 0, so every change runs on it; a new file gets the first layout and then the same
//! changes.

use rusqlite::Connection;

use super::CacheError;

/// Changes in order; change `n` moves `user_version` from `n - 1` to `n`.
const CHANGES: [&str; 1] = [
    // 1: a pinned flag, so the lyrics of songs saved offline do not expire (offline-cache).
    "ALTER TABLE lyrics_cache ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;",
];

/// Apply every change newer than the file's `user_version`, each in its own transaction.
pub(super) fn run(conn: &mut Connection) -> Result<(), CacheError> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    for (target, sql) in (1_i64..).zip(CHANGES) {
        if target <= version {
            continue;
        }
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", target)?;
        tx.commit()?;
    }
    Ok(())
}
