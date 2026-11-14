//! Tests for the offline index and its file layout.

use std::collections::HashSet;
use std::sync::atomic::{AtomicI64, Ordering};

use rusqlite::Connection;

use super::*;

mod mode;
mod remove;

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
    assert_eq!(migrations(&d), vec![(1, 100), (2, 100)]);
    assert_eq!(cache.saved_count(), Ok(0));
}

#[test]
fn reopening_keeps_rows_and_runs_no_migration_twice() {
    let d = dir("reopen");
    drop(OfflineCache::open(&d.0, clock(100)).expect("first"));
    let file = opaque_name().expect("name");
    // The file exists, so the start-up cleanup keeps the row.
    std::fs::write(d.0.join("blobs").join(&file), b"x").expect("blob");
    raw(&d)
        .execute(
            "INSERT INTO song (video_id, file, bytes, codec, reason, meta, saved_at)
             VALUES ('synthetic', ?1, 10, 'opus', 'user', '{}', 100)",
            [file],
        )
        .expect("insert");
    let cache = OfflineCache::open(&d.0, clock(999)).expect("again");
    assert_eq!(cache.saved_count(), Ok(1));
    assert_eq!(migrations(&d), vec![(1, 100), (2, 100)]);
}

#[test]
fn an_empty_index_file_is_migrated() {
    let d = dir("empty");
    std::fs::create_dir_all(&d.0).expect("dir");
    std::fs::write(d.0.join("offline.db"), b"").expect("empty file");
    let cache = OfflineCache::open(&d.0, clock(5)).expect("open");
    assert_eq!(cache.saved_count(), Ok(0));
    assert_eq!(migrations(&d), vec![(1, 5), (2, 5)]);
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
    for err in [
        OfflineError::Db,
        OfflineError::Io,
        OfflineError::Random,
        OfflineError::Closed,
    ] {
        let text = err.to_string();
        assert!(!text.contains('/') && !text.contains('\\'), "{text}");
    }
}

/// Insert a saved song with the opaque file name `file` (synthetic ids only).
fn insert_song(dir: &Dir, video_id: &str, file: &str) {
    raw(dir)
        .execute(
            "INSERT INTO song (video_id, file, bytes, codec, reason, meta, saved_at)
             VALUES (?1, ?2, 1, 'opus', 'user', '{}', 1)",
            [video_id, file],
        )
        .expect("insert song");
}

#[test]
fn start_up_deletes_part_files_no_queue_row_names() {
    let d = dir("parts");
    drop(OfflineCache::open(&d.0, clock(1)).expect("first"));
    let queued = opaque_name().expect("name");
    let stray = opaque_name().expect("name");
    raw(&d)
        .execute(
            "INSERT INTO queue (video_id, position, reason, state, part_file, meta, added_at)
             VALUES ('aaaaaaaaaa1', 0, 'user', 'saving', ?1, '{}', 1)",
            [&queued],
        )
        .expect("insert queue");
    let tmp = d.0.join("tmp");
    std::fs::write(tmp.join(format!("{queued}.part")), b"x").expect("queued part");
    std::fs::write(tmp.join(format!("{stray}.part")), b"x").expect("stray part");
    let cache = OfflineCache::open(&d.0, clock(2)).expect("again");
    assert_eq!(cache.startup_cleanup().parts_removed, 1);
    assert!(tmp.join(format!("{queued}.part")).is_file());
    assert!(!tmp.join(format!("{stray}.part")).exists());
    assert!(!cache.startup_cleanup().index_reset);
    assert!(!cache.take_reset_notice());
}

#[test]
fn start_up_marks_a_song_with_a_missing_file_unsaved() {
    let d = dir("missing");
    drop(OfflineCache::open(&d.0, clock(1)).expect("first"));
    let kept = opaque_name().expect("name");
    let gone = opaque_name().expect("name");
    std::fs::write(d.0.join("blobs").join(&kept), b"x").expect("blob");
    insert_song(&d, "aaaaaaaaaa1", &kept);
    insert_song(&d, "bbbbbbbbbb2", &gone);
    raw(&d)
        .execute(
            "INSERT INTO segment (video_id, start_ms, end_ms, category, segment_id)
             VALUES ('bbbbbbbbbb2', 0, 10, 'intro', 's1')",
            [],
        )
        .expect("insert segment");
    let cache = OfflineCache::open(&d.0, clock(2)).expect("again");
    let unsaved = VideoId::parse("bbbbbbbbbb2").expect("id");
    assert_eq!(cache.startup_cleanup().unsaved, vec![unsaved]);
    assert_eq!(cache.saved_count(), Ok(1));
    let segments: i64 = raw(&d)
        .query_row("SELECT COUNT(*) FROM segment", [], |row| row.get(0))
        .expect("count");
    assert_eq!(segments, 0);
}

fn assert_reset(d: &Dir) {
    let cache = OfflineCache::open(&d.0, clock(7)).expect("reset and open");
    assert!(cache.startup_cleanup().index_reset);
    assert!(cache.take_reset_notice(), "told once");
    assert!(!cache.take_reset_notice(), "not told twice");
    assert_eq!(cache.saved_count(), Ok(0));
    assert_eq!(migrations(d), vec![(1, 7), (2, 7)]);
    for folder in ["blobs", "tmp", "art"] {
        let entries = std::fs::read_dir(d.0.join(folder)).expect("folder").count();
        assert_eq!(entries, 0, "{folder}");
    }
    drop(cache);
    let again = OfflineCache::open(&d.0, clock(8)).expect("reopen");
    assert!(!again.startup_cleanup().index_reset);
}

#[test]
fn a_truncated_index_is_deleted_with_its_files() {
    let d = dir("truncated");
    drop(OfflineCache::open(&d.0, clock(1)).expect("first"));
    let file = opaque_name().expect("name");
    std::fs::write(d.0.join("blobs").join(&file), b"x").expect("blob");
    insert_song(&d, "aaaaaaaaaa1", &file);
    let db = d.0.join("offline.db");
    let len = std::fs::metadata(&db).expect("meta").len();
    assert!(len > 8192, "{len}");
    std::fs::OpenOptions::new()
        .write(true)
        .open(&db)
        .and_then(|f| f.set_len(4096 * 2))
        .expect("truncate");
    assert_reset(&d);
}

#[test]
fn an_index_that_is_not_a_database_is_reset() {
    let d = dir("garbage");
    std::fs::create_dir_all(d.0.join("blobs")).expect("dir");
    std::fs::write(d.0.join("offline.db"), [0x5a_u8; 8192]).expect("garbage");
    std::fs::write(d.0.join("blobs").join(opaque_name().expect("name")), b"x").expect("blob");
    assert_reset(&d);
}
