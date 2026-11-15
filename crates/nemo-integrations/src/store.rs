//! The integrations database `integrations.sqlite3` (integrations spec, task 13).
//!
//! One SQLite file in WAL mode with two tables: `scrobbles`, the Last.fm queue (see
//! [`crate::lastfm::queue`]), and `meta`, a key and value table for the three switches and the
//! public Last.fm user name. No secret is kept here: the Last.fm key, secret and session key live
//! in the credential store. Media controls default to on (criterion 9), so the first migration
//! writes that switch.
//!
//! Schema changes go through [`MIGRATIONS`]: step `n` moves `PRAGMA user_version` from `n` to
//! `n + 1`, inside one transaction, so a crash never leaves a half-built schema. A file written
//! by a newer build is refused, not changed.
//!
//! If the file cannot be opened, [`Store::open`] returns an error. The caller then leaves Last.fm
//! unavailable; OS media controls and Discord presence do not use this database and keep working.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use rusqlite::{Connection, OptionalExtension, params};

/// File name of the database inside the integrations folder.
pub const DB_FILE: &str = "integrations.sqlite3";

/// Meta key of the OS media controls switch (`"1"` or `"0"`).
pub const META_MEDIA_CONTROLS: &str = "media_controls";
/// Meta key of the Last.fm switch (`"1"` or `"0"`).
pub const META_LASTFM: &str = "lastfm";
/// Meta key of the Discord presence switch (`"1"` or `"0"`).
pub const META_DISCORD: &str = "discord";
/// Meta key of the Last.fm user name (public data, not a secret).
pub const META_LASTFM_USER: &str = "lastfm_user";

/// One schema step each. The index of a step is the `user_version` it starts from.
const MIGRATIONS: &[&str] = &["
    CREATE TABLE scrobbles (
        id         INTEGER PRIMARY KEY,
        ts         INTEGER NOT NULL,
        artist     TEXT NOT NULL,
        track      TEXT NOT NULL,
        album      TEXT,
        duration_s INTEGER,
        attempts   INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE meta (
        key   TEXT PRIMARY KEY NOT NULL,
        value TEXT NOT NULL
    );
    INSERT INTO meta (key, value) VALUES ('media_controls', '1');
"];

/// The integrations database could not be used.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The folder could not be created.
    #[error("cannot create the integrations folder {path}: {source}")]
    CreateDir {
        /// The folder.
        path: PathBuf,
        /// What the file system said.
        source: std::io::Error,
    },
    /// SQLite refused the file or a statement.
    #[error("integrations database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// The file was written by a newer build of the app.
    #[error(
        "integrations database version {found} is newer than this build supports ({supported})"
    )]
    TooNew {
        /// `user_version` found in the file.
        found: usize,
        /// Highest version this build knows.
        supported: usize,
    },
}

/// The integrations database. Cheap to clone; every clone shares one connection.
#[derive(Debug, Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
    /// Set after the cap warning was logged, cleared when the queue is below the cap again.
    pub(crate) cap_warned: Arc<std::sync::atomic::AtomicBool>,
}

impl Store {
    /// Open (or create) `integrations.sqlite3` inside `dir` and bring its schema up to date.
    ///
    /// # Errors
    /// [`StoreError`] when the folder or file cannot be created or opened, a migration fails, or
    /// the file comes from a newer build.
    pub fn open(dir: &Path) -> Result<Self, StoreError> {
        std::fs::create_dir_all(dir).map_err(|source| StoreError::CreateDir {
            path: dir.to_path_buf(),
            source,
        })?;
        Self::init(Connection::open(dir.join(DB_FILE))?)
    }

    /// An in-memory database, for tests of other modules.
    #[cfg(test)]
    pub(crate) fn open_in_memory() -> Result<Self, StoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self, StoreError> {
        // `journal_mode` returns a row, so it cannot go through `execute_batch`. An in-memory
        // database answers `memory`, which is fine.
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        migrate(&mut conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            cap_warned: Arc::default(),
        })
    }

    /// Run `f` with the connection locked.
    pub(crate) fn with_conn<T>(&self, f: impl FnOnce(&mut Connection) -> T) -> T {
        let mut guard: MutexGuard<'_, Connection> =
            self.conn.lock().unwrap_or_else(PoisonError::into_inner);
        f(&mut guard)
    }

    /// The value stored under `key`, if any.
    ///
    /// # Errors
    /// [`StoreError::Sqlite`] when the query fails.
    pub fn meta_get(&self, key: &str) -> Result<Option<String>, StoreError> {
        self.with_conn(|c| {
            c.query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| {
                r.get(0)
            })
            .optional()
            .map_err(StoreError::from)
        })
    }

    /// Store `value` under `key`, replacing an older value.
    ///
    /// # Errors
    /// [`StoreError::Sqlite`] when the statement fails.
    pub fn meta_set(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map(|_| ())
            .map_err(StoreError::from)
        })
    }

    /// Delete `key`. A missing key is not an error.
    ///
    /// # Errors
    /// [`StoreError::Sqlite`] when the statement fails.
    pub fn meta_remove(&self, key: &str) -> Result<(), StoreError> {
        self.with_conn(|c| {
            c.execute("DELETE FROM meta WHERE key = ?1", params![key])
                .map(|_| ())
                .map_err(StoreError::from)
        })
    }

    /// Read a switch. A missing key reads as `default`.
    ///
    /// # Errors
    /// [`StoreError::Sqlite`] when the query fails.
    pub fn switch(&self, key: &str, default: bool) -> Result<bool, StoreError> {
        Ok(self.meta_get(key)?.map_or(default, |v| v == "1"))
    }

    /// Write a switch.
    ///
    /// # Errors
    /// [`StoreError::Sqlite`] when the statement fails.
    pub fn set_switch(&self, key: &str, on: bool) -> Result<(), StoreError> {
        self.meta_set(key, if on { "1" } else { "0" })
    }
}

fn migrate(conn: &mut Connection) -> Result<(), StoreError> {
    let raw: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let found = usize::try_from(raw).unwrap_or(usize::MAX);
    if found > MIGRATIONS.len() {
        return Err(StoreError::TooNew {
            found,
            supported: MIGRATIONS.len(),
        });
    }
    for (from, sql) in MIGRATIONS.iter().enumerate().skip(found) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        // `PRAGMA` does not take bound parameters; `from + 1` is a number we computed.
        tx.pragma_update(
            None,
            "user_version",
            i64::try_from(from + 1).unwrap_or(i64::MAX),
        )?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A fresh folder under the system temp dir, removed on drop.
    pub(crate) struct TempDir(pub PathBuf);

    impl TempDir {
        pub(crate) fn new(tag: &str) -> Self {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static N: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "nemo-integrations-{tag}-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            // Best effort: a leftover temp folder is harmless.
            drop(std::fs::remove_dir_all(&self.0));
        }
    }

    fn user_version(store: &Store) -> usize {
        let raw: i64 = store.with_conn(|c| {
            c.pragma_query_value(None, "user_version", |r| r.get(0))
                .expect("user_version")
        });
        usize::try_from(raw).expect("non-negative")
    }

    #[test]
    fn migrates_an_empty_file_to_the_latest_version() {
        let dir = TempDir::new("migrate");
        let store = Store::open(&dir.0).expect("open");
        assert_eq!(user_version(&store), MIGRATIONS.len());
        assert!(dir.0.join(DB_FILE).is_file());
    }

    #[test]
    fn opening_twice_keeps_version_and_data() {
        let dir = TempDir::new("twice");
        let first = Store::open(&dir.0).expect("open");
        first.meta_set(META_LASTFM_USER, "someone").expect("set");
        drop(first);
        let second = Store::open(&dir.0).expect("reopen");
        assert_eq!(user_version(&second), MIGRATIONS.len());
        assert_eq!(
            second.meta_get(META_LASTFM_USER).expect("get").as_deref(),
            Some("someone")
        );
    }

    #[test]
    fn media_controls_default_on_and_the_others_off() {
        let store = Store::open_in_memory().expect("open");
        assert!(store.switch(META_MEDIA_CONTROLS, false).expect("read"));
        assert!(!store.switch(META_LASTFM, false).expect("read"));
        assert!(!store.switch(META_DISCORD, false).expect("read"));
    }

    #[test]
    fn a_switch_survives_reopen() {
        let dir = TempDir::new("switch");
        let store = Store::open(&dir.0).expect("open");
        store.set_switch(META_MEDIA_CONTROLS, false).expect("off");
        store.set_switch(META_LASTFM, true).expect("on");
        drop(store);
        let store = Store::open(&dir.0).expect("reopen");
        assert!(!store.switch(META_MEDIA_CONTROLS, true).expect("read"));
        assert!(store.switch(META_LASTFM, false).expect("read"));
    }

    #[test]
    fn meta_remove_forgets_the_key() {
        let store = Store::open_in_memory().expect("open");
        store.meta_set(META_LASTFM_USER, "x").expect("set");
        store.meta_remove(META_LASTFM_USER).expect("remove");
        assert_eq!(store.meta_get(META_LASTFM_USER).expect("get"), None);
    }

    #[test]
    fn a_newer_database_is_refused_and_left_alone() {
        let dir = TempDir::new("newer");
        std::fs::create_dir_all(&dir.0).expect("mkdir");
        let path = dir.0.join(DB_FILE);
        let newer = MIGRATIONS.len() + 1;
        Connection::open(&path)
            .expect("open raw")
            .pragma_update(None, "user_version", i64::try_from(newer).expect("small"))
            .expect("set version");
        let err = Store::open(&dir.0).expect_err("must refuse");
        assert!(matches!(err, StoreError::TooNew { found, .. } if found == newer));
    }

    #[test]
    fn an_unopenable_file_is_an_error_not_a_panic() {
        let dir = TempDir::new("unopenable");
        // A directory sits where the database file should be.
        std::fs::create_dir_all(dir.0.join(DB_FILE)).expect("mkdir");
        assert!(Store::open(&dir.0).is_err());
        // Media controls and Discord do not use the database: building the service still works.
        let service = crate::build();
        assert!(nemo_core::integrations::IntegrationsService::media_controls(&service));
    }
}
