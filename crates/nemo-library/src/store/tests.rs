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
