//! Tests for the offline index and its file layout.

use std::collections::HashSet;
use std::sync::atomic::{AtomicI64, Ordering};

use rusqlite::Connection;

use super::*;

struct FakeClock(AtomicI64);

impl Clock for FakeClock {
    fn now_secs(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn dir(name: &str) -> Dir {
    let path = std::env::temp_dir().join(format!("nemo-offline-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    Dir(path)
}

fn clock(at: i64) -> Arc<FakeClock> {
    Arc::new(FakeClock(AtomicI64::new(at)))
}

fn raw(dir: &Dir) -> Connection {
    Connection::open(dir.0.join("offline.db")).expect("raw open")
}

fn migrations(dir: &Dir) -> Vec<(i64, i64)> {
    let conn = raw(dir);
    let mut stmt = conn
        .prepare("SELECT version, applied_at FROM schema_migration ORDER BY version")
        .expect("prepare");
    stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows")
}

#[test]
fn open_creates_the_folders_and_every_table() {
    let d = dir("layout");
    let cache = OfflineCache::open(&d.0, clock(100)).expect("open");
    for folder in ["blobs", "tmp", "art"] {
        assert!(d.0.join(folder).is_dir(), "{folder}");
    }
    let conn = raw(&d);
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
        .expect("prepare");
    let tables: HashSet<String> = stmt
        .query_map([], |row| row.get(0))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows");
    for table in [
        "song",
        "segment",
        "queue",
        "play_count",
        "collection",
        "collection_song",
        "art",
        "schema_migration",
    ] {
        assert!(tables.contains(table), "{table}");
    }
    assert_eq!(migrations(&d), vec![(1, 100)]);
    assert_eq!(cache.saved_count(), Ok(0));
}

#[test]
fn reopening_keeps_rows_and_runs_no_migration_twice() {
    let d = dir("reopen");
    drop(OfflineCache::open(&d.0, clock(100)).expect("first"));
    raw(&d)
        .execute(
            "INSERT INTO song (video_id, file, bytes, codec, reason, meta, saved_at)
             VALUES ('synthetic', ?1, 10, 'opus', 'user', '{}', 100)",
            [opaque_name().expect("name")],
        )
        .expect("insert");
    let cache = OfflineCache::open(&d.0, clock(999)).expect("again");
    assert_eq!(cache.saved_count(), Ok(1));
    assert_eq!(migrations(&d), vec![(1, 100)]);
}

#[test]
fn an_empty_index_file_is_migrated() {
    let d = dir("empty");
    std::fs::create_dir_all(&d.0).expect("dir");
    std::fs::write(d.0.join("offline.db"), b"").expect("empty file");
    let cache = OfflineCache::open(&d.0, clock(5)).expect("open");
    assert_eq!(cache.saved_count(), Ok(0));
    assert_eq!(migrations(&d), vec![(1, 5)]);
}

#[test]
fn an_index_from_a_newer_app_is_refused() {
    let d = dir("newer");
    drop(OfflineCache::open(&d.0, clock(1)).expect("first"));
    raw(&d)
        .execute(
            "INSERT INTO schema_migration (version, applied_at) VALUES (99, 1)",
            [],
        )
        .expect("insert");
    assert_eq!(
        OfflineCache::open(&d.0, clock(2)).map(|_| ()),
        Err(OfflineError::Db)
    );
}

#[test]
fn opaque_names_are_random_hex() {
    let names: HashSet<String> = (0..200).map(|_| opaque_name().expect("name")).collect();
    assert_eq!(names.len(), 200);
    assert!(names.iter().all(|n| is_opaque_name(n)));
}

#[test]
fn no_file_name_holds_a_video_id_or_a_title() {
    let d = dir("names");
    let cache = OfflineCache::open(&d.0, clock(1)).expect("open");
    for bad in ["dQw4w9WgXcQ", "Never Gonna Give You Up", "../escape", ""] {
        for kind in [FileKind::Song, FileKind::Part, FileKind::Art] {
            assert!(cache.file_path(kind, bad).is_none(), "{bad}");
        }
    }
    for kind in [FileKind::Song, FileKind::Part, FileKind::Art] {
        let name = opaque_name().expect("name");
        let path = cache.file_path(kind, &name).expect("path");
        std::fs::write(path.expose(), b"x").expect("write");
    }
    for folder in ["blobs", "tmp", "art"] {
        for entry in std::fs::read_dir(d.0.join(folder)).expect("read dir") {
            let file = entry.expect("entry").file_name();
            let file = file.to_str().expect("utf-8");
            let stem = file.strip_suffix(".part");
            assert!(is_opaque_name(stem.unwrap_or(file)), "{folder}/{file}");
            assert_eq!(stem.is_some(), folder == "tmp", "{folder}/{file}");
        }
    }
}

#[test]
fn paths_never_reach_debug_output_or_error_texts() {
    let d = dir("debug");
    let cache = OfflineCache::open(&d.0, clock(1)).expect("open");
    let path = cache
        .file_path(FileKind::Song, &opaque_name().expect("name"))
        .expect("path");
    assert_eq!(format!("{path:?}"), "[redacted]");
    assert!(!format!("{cache:?}").contains("nemo-offline"));
    for err in [OfflineError::Db, OfflineError::Io, OfflineError::Random] {
        let text = err.to_string();
        assert!(!text.contains('/') && !text.contains('\\'), "{text}");
    }
}
