//! Tests for the library database: migrations, reopen, WAL, folders, tracks, playlists.

use std::time::Duration;

use nemo_core::audio::Origin;
use nemo_core::library::{
    LocalPlaylistId, ReplayGainValues, TrackSpan, TrackStatus, UnplayableReason,
};
use nemo_core::track::{Artwork, Thumbnail, Track, TrackId, VideoId};

use super::*;

struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn dir(name: &str) -> Dir {
    let path =
        std::env::temp_dir().join(format!("nemo-library-store-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    Dir(path)
}

fn migrations(store: &Store) -> Vec<i64> {
    let mut stmt = store
        .conn
        .prepare("SELECT version FROM schema_migration ORDER BY version")
        .expect("prepare");
    stmt.query_map([], |r| r.get(0))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows")
}

fn music(d: &Dir) -> PathBuf {
    d.0.join("Music")
}

fn row(path: PathBuf, title: &str) -> TrackRow {
    TrackRow {
        path,
        size: 1234,
        modified_ms: 1_700_000_000_000,
        title: title.to_owned(),
        artists: vec!["Synthetic Artist".to_owned(), "Guest".to_owned()],
        album: Some("Synthetic Album".to_owned()),
        album_artist: Some("Synthetic Artist".to_owned()),
        disc: Some(1),
        number: Some(2),
        year: Some(2026),
        genre: Some("Ambient".to_owned()),
        duration: Some(Duration::from_millis(183_250)),
        ..TrackRow::default()
    }
}

fn stream_track() -> Track {
    Track {
        id: TrackId("ytm:aaaaaaaaaa1".to_owned()),
        title: "Streamed".to_owned(),
        artists: vec!["Someone".to_owned()],
        album: None,
        duration: Some(Duration::from_secs(200)),
        origin: Origin::YoutubeMusic,
        video_id: Some(VideoId::parse("aaaaaaaaaa1").expect("id")),
        local_path: None,
        artwork: Artwork::Remote(vec![Thumbnail {
            url: "https://i.ytimg.com/vi/x/hq.jpg".to_owned(),
            width: 120,
            height: 90,
        }]),
        source_url: None,
    }
}

#[test]
fn open_runs_migration_1_once_in_wal_mode() {
    let d = dir("open");
    let store = Store::open(&d.0).expect("open");
    assert_eq!(migrations(&store), vec![1]);
    let mode: String = store
        .conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .expect("mode");
    assert_eq!(mode, "wal");
    drop(store);
    let again = Store::open(&d.0).expect("reopen");
    assert_eq!(migrations(&again), vec![1]);
}

#[test]
fn a_file_from_a_newer_app_is_refused() {
    let d = dir("newer");
    let store = Store::open(&d.0).expect("open");
    store
        .conn
        .execute("INSERT INTO schema_migration VALUES (9, 0)", [])
        .expect("insert");
    drop(store);
    assert_eq!(Store::open(&d.0).map(|_| ()), Err(LibraryError::Db));
}

#[test]
fn rows_survive_reopen_and_a_reader_sees_them_while_the_writer_is_open() {
    let d = dir("reopen");
    let mut writer = Store::open(&d.0).expect("open");
    writer.add_folder(&music(&d)).expect("folder");
    writer.add_folder(&music(&d)).expect("adding twice is fine");
    writer
        .write_tracks(&music(&d), &[row(music(&d).join("a.flac"), "A")])
        .expect("write");
    let reader = Store::open_reader(&d.0).expect("reader");
    assert_eq!(reader.folders().expect("folders"), vec![music(&d)]);
    assert_eq!(reader.all_tracks().expect("tracks").len(), 1);
    assert_eq!(
        reader.conn.execute("DELETE FROM track", []).map_err(db),
        Err(LibraryError::Db)
    );
    drop((writer, reader));
    let again = Store::open(&d.0).expect("reopen");
    assert_eq!(again.all_tracks().expect("tracks")[0].title, "A");
}

#[test]
fn a_reader_without_a_library_file_fails() {
    let d = dir("no-file");
    std::fs::create_dir_all(&d.0).expect("dir");
    assert_eq!(Store::open_reader(&d.0).map(|_| ()), Err(LibraryError::Db));
}

#[test]
fn every_track_field_round_trips_and_a_rewrite_replaces_the_row() {
    let d = dir("round-trip");
    let mut store = Store::open(&d.0).expect("open");
    store.add_folder(&music(&d)).expect("folder");
    let mut cue = row(music(&d).join("live.flac"), "Part two");
    cue.cue_index = 2;
    cue.span = Some(TrackSpan {
        start: Duration::from_millis(61_500),
        end: Some(Duration::from_millis(122_500)),
    });
    cue.gain = ReplayGainValues {
        track_gain_db: Some(-6.5),
        album_gain_db: Some(-7.25),
        track_peak: Some(0.98),
        album_peak: Some(1.0),
    };
    let mut broken = row(music(&d).join("broken.ogg"), "Broken");
    broken.status = TrackStatus::Unplayable(UnplayableReason::DecodeFailed);
    broken.album = None;
    store
        .write_tracks(&music(&d), &[cue.clone(), broken.clone()])
        .expect("write");
    assert_eq!(
        store.all_tracks().expect("tracks"),
        vec![broken.clone(), cue.clone()]
    );

    let playback = store
        .playback(&cue.path, 2)
        .expect("lookup")
        .expect("stored");
    assert_eq!((playback.span, playback.gain), (cue.span, cue.gain));
    assert_eq!(store.playback(&cue.path, 1).expect("lookup"), None);

    let mut changed = broken.clone();
    changed.size = 99;
    changed.status = TrackStatus::Playable;
    store
        .write_tracks(&music(&d), &[changed.clone()])
        .expect("rewrite");
    let all = store.all_tracks().expect("tracks");
    assert_eq!(all.len(), 2);
    assert_eq!(all[0], changed);
}

#[test]
fn stamps_and_removal_of_gone_files() {
    let d = dir("stamps");
    let mut store = Store::open(&d.0).expect("open");
    store.add_folder(&music(&d)).expect("folder");
    let a = music(&d).join("a.flac");
    let b = music(&d).join("b.flac");
    store
        .write_tracks(&music(&d), &[row(a.clone(), "A"), row(b.clone(), "B")])
        .expect("write");
    let stamps = store.stamps(&music(&d)).expect("stamps");
    assert_eq!(stamps.get(&a), Some(&(1234, 1_700_000_000_000)));
    assert_eq!(store.remove_files(&[b]).expect("remove"), 1);
    assert_eq!(store.all_tracks().expect("tracks").len(), 1);
}

#[test]
fn removing_a_folder_removes_its_tracks_only_and_no_file() {
    let d = dir("folders");
    std::fs::create_dir_all(music(&d)).expect("dir");
    let file = music(&d).join("kept.flac");
    std::fs::write(&file, b"audio").expect("file");
    let other = d.0.join("Other");
    let mut store = Store::open(&d.0).expect("open");
    store.add_folder(&music(&d)).expect("folder");
    store.add_folder(&other).expect("other");
    store
        .write_tracks(&music(&d), &[row(file.clone(), "Kept")])
        .expect("write");
    store
        .write_tracks(&other, &[row(other.join("x.flac"), "X")])
        .expect("write");
    assert_eq!(store.remove_folder(&music(&d)).expect("remove"), 1);
    assert_eq!(store.folders().expect("folders"), vec![other]);
    assert_eq!(store.all_tracks().expect("tracks")[0].title, "X");
    assert_eq!(std::fs::read(&file).expect("file is still there"), b"audio");
    assert_eq!(
        store.write_tracks(&music(&d), &[row(file, "Kept")]),
        Err(LibraryError::Db),
        "an unwatched folder takes no tracks"
    );
}

#[test]
fn playlists_save_each_change_and_survive_reopen() {
    let d = dir("playlists");
    let mut store = Store::open(&d.0).expect("open");
    store.add_folder(&music(&d)).expect("folder");
    let here = music(&d).join("here.flac");
    std::fs::create_dir_all(music(&d)).expect("dir");
    std::fs::write(&here, b"audio").expect("file");
    store
        .write_tracks(&music(&d), &[row(here.clone(), "Here")])
        .expect("write");
    let id = store.create_playlist("Evening").expect("create");
    let added = store.add_entries(
        id,
        &[
            NewEntry::File {
                path: here.clone(),
                cue_index: 0,
            },
            NewEntry::Stream(stream_track()),
            NewEntry::File {
                path: music(&d).join("gone.flac"),
                cue_index: 0,
            },
        ],
    );
    assert_eq!(added, Ok(3));
    store.rename_playlist(id, "Night").expect("rename");
    store.move_entry(id, 2, 0).expect("move");
    drop(store);

    let mut store = Store::open(&d.0).expect("reopen");
    let lists = store.playlists().expect("playlists");
    assert_eq!(lists.len(), 1);
    assert_eq!(lists[0].name, "Night");
    let entries = &lists[0].entries;
    assert_eq!(entries[0].path, Some(music(&d).join("gone.flac")));
    assert!(entries[0].missing);
    assert_eq!(entries[1].path, Some(here));
    assert!(!entries[1].missing);
    assert_eq!(entries[2].track, Some(stream_track()));
    assert!(!entries[2].missing);

    store.remove_entry(id, 0).expect("remove");
    assert_eq!(store.playlists().expect("playlists")[0].entries.len(), 2);
    assert_eq!(store.move_entry(id, 0, 5), Err(LibraryError::Db));
    assert_eq!(store.remove_entry(id, 9), Err(LibraryError::Db));
    assert_eq!(
        store.rename_playlist(LocalPlaylistId(99), "x"),
        Err(LibraryError::Db)
    );
    store.delete_playlist(id).expect("delete");
    assert!(store.playlists().expect("playlists").is_empty());
}

#[test]
fn gapless_albums_and_loudness_by_file_stamp() {
    let d = dir("extras");
    let mut store = Store::open(&d.0).expect("open");
    assert!(!store.gapless("Live", "Band").expect("unknown"));
    store.set_gapless("Live", "Band", true).expect("set");
    assert!(store.gapless("Live", "Band").expect("known"));
    store.set_gapless("Live", "Band", false).expect("unset");
    assert!(!store.gapless("Live", "Band").expect("known"));

    let file = music(&d).join("quiet.flac");
    let loud = Loudness {
        integrated_lufs: -23.5,
        true_peak: 0.5,
    };
    store.put_loudness(&file, (10, 20), loud).expect("put");
    assert_eq!(store.loudness(&file, (10, 20)).expect("get"), Some(loud));
    assert_eq!(store.loudness(&file, (11, 20)).expect("changed file"), None);
}

/// A library with one folder, enough tracks to span several pages, and a playlist.
fn filled(d: &Dir) {
    let mut store = Store::open(&d.0).expect("open");
    store.add_folder(&music(d)).expect("folder");
    let rows: Vec<TrackRow> = (0..300)
        .map(|i| row(music(d).join(format!("{i:03}.flac")), &format!("Song {i}")))
        .collect();
    store.write_tracks(&music(d), &rows).expect("write");
    let id = store.create_playlist("Kept").expect("create");
    store
        .add_entries(id, &[NewEntry::Stream(stream_track())])
        .expect("add");
}

fn kept_copies(d: &Dir) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(&d.0)
        .expect("dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(BROKEN_PREFIX))
        .collect();
    names.sort();
    names
}

#[test]
fn a_truncated_file_is_kept_and_the_library_rebuilt() {
    let d = dir("truncated");
    filled(&d);
    let file = d.0.join(DB_FILE);
    let len = std::fs::metadata(&file).expect("meta").len();
    assert!(
        len > 8 * 4096,
        "the test file spans many pages ({len} bytes)"
    );
    std::fs::OpenOptions::new()
        .write(true)
        .open(&file)
        .and_then(|f| f.set_len(len / 2))
        .expect("truncate");
    let damaged = std::fs::read(&file).expect("read");

    let mut store = Store::open(&d.0).expect("open rebuilds");
    let rebuilt = store.take_rebuilt().expect("reported");
    assert_eq!(rebuilt.event(), ScanEvent::Rebuilt);
    assert_eq!(store.take_rebuilt(), None, "reported once");
    assert_eq!(
        rebuilt.kept.parent(),
        Some(d.0.as_path()),
        "kept beside the new file"
    );
    let kept_name = rebuilt.kept.file_name().expect("name").to_string_lossy();
    assert!(kept_name.starts_with(BROKEN_PREFIX));
    assert!(kept_name[BROKEN_PREFIX.len()..].parse::<i64>().is_ok());
    assert_eq!(
        std::fs::read(&rebuilt.kept).expect("kept"),
        damaged,
        "the copy is the damaged file, byte for byte"
    );
    // The read-only check may leave SQLite's empty `-wal` and `-shm` beside it; they move too.
    let copies = kept_copies(&d);
    assert!(copies.contains(&kept_name.to_string()));
    assert!(copies.iter().all(|n| n.starts_with(kept_name.as_ref())));

    assert_eq!(migrations(&store), vec![1]);
    assert!(store.all_tracks().expect("tracks").is_empty());
    assert!(store.playlists().expect("playlists").is_empty());
    assert_eq!(
        store.folders().expect("folders"),
        vec![music(&d)],
        "the folder list is read back so the rescan has something to scan"
    );
    drop(store);
    let mut again = Store::open(&d.0).expect("reopen");
    assert_eq!(again.take_rebuilt(), None, "the new file is sound");
    assert_eq!(kept_copies(&d), copies);
}

#[test]
fn a_file_that_is_no_database_is_kept_with_its_wal_and_shm() {
    let d = dir("garbage");
    std::fs::create_dir_all(&d.0).expect("dir");
    let garbage = vec![0xA5_u8; 3 * 4096];
    std::fs::write(d.0.join(DB_FILE), &garbage).expect("db");
    std::fs::write(d.0.join(format!("{DB_FILE}-wal")), b"wal").expect("wal");
    std::fs::write(d.0.join(format!("{DB_FILE}-shm")), b"shm").expect("shm");

    let mut store = Store::open(&d.0).expect("open rebuilds");
    let rebuilt = store.take_rebuilt().expect("reported");
    assert_eq!(std::fs::read(&rebuilt.kept).expect("kept"), garbage);
    let side = |suffix: &str| {
        let mut name = rebuilt.kept.clone().into_os_string();
        name.push(suffix);
        PathBuf::from(name)
    };
    assert_eq!(std::fs::read(side("-wal")).expect("kept wal"), b"wal");
    // `-shm` is SQLite's rebuildable index, not data: the check may rewrite it, but it moves.
    assert!(side("-shm").exists());
    assert!(store.folders().expect("folders").is_empty());
    assert_eq!(kept_copies(&d).len(), 3);
}

#[test]
fn a_sound_file_is_not_rebuilt_and_the_notice_names_the_playlists_not_a_path() {
    let d = dir("sound");
    filled(&d);
    let mut store = Store::open(&d.0).expect("open");
    assert_eq!(store.take_rebuilt(), None);
    assert_eq!(store.all_tracks().expect("tracks").len(), 300);
    assert!(kept_copies(&d).is_empty());
    assert!(REBUILT_NOTICE.contains("old playlists"));
    assert!(!REBUILT_NOTICE.contains('/') && !REBUILT_NOTICE.contains('\\'));
}
