//! Tests for local playlists: every change survives a reopen, missing files stay listed.

use std::path::Path;
use std::time::Duration;

use nemo_core::track::{Artwork, TrackId, VideoId};

use super::*;
use crate::scan::track_from_path;
use crate::store::TrackRow;

struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0); // Test cleanup; a leftover is harmless.
    }
}

fn dir(name: &str) -> Dir {
    let path = std::env::temp_dir().join(format!(
        "nemo-library-playlists-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path); // A leftover from an earlier run is fine to drop.
    std::fs::create_dir_all(path.join("Music")).expect("dir");
    Dir(path)
}

fn file(d: &Dir, name: &str) -> PathBuf {
    let path = d.0.join("Music").join(name);
    std::fs::write(&path, b"audio").expect("file");
    path
}

fn row(path: &Path) -> TrackRow {
    TrackRow {
        path: path.to_path_buf(),
        size: 5,
        title: "Song".to_owned(),
        ..TrackRow::default()
    }
}

fn streamed() -> Track {
    Track {
        id: TrackId("ytm:bbbbbbbbbb2".to_owned()),
        title: "Streamed".to_owned(),
        artists: vec!["Someone".to_owned()],
        album: Some("Live".to_owned()),
        duration: Some(Duration::from_secs(210)),
        origin: Origin::YoutubeMusic,
        video_id: Some(VideoId::parse("bbbbbbbbbb2").expect("id")),
        local_path: None,
        artwork: Artwork::None,
        source_url: None,
    }
}

fn paths(store: &Store, list: usize) -> Vec<Option<PathBuf>> {
    store.playlists().expect("playlists")[list]
        .entries
        .iter()
        .map(|e| e.path.clone())
        .collect()
}

#[test]
fn a_local_track_becomes_a_file_entry_and_a_streamed_one_is_kept_whole() {
    let local = track_from_path(PathBuf::from("/Music/a.flac"));
    assert_eq!(
        entry_for(&local),
        NewEntry::File {
            path: PathBuf::from("/Music/a.flac"),
            cue_index: 0
        }
    );
    assert_eq!(entry_for(&streamed()), NewEntry::Stream(streamed()));
}

#[test]
fn every_change_is_saved_at_once_and_kept_after_reopen() {
    let d = dir("reopen");
    let (a, b, c) = (file(&d, "a.flac"), file(&d, "b.flac"), file(&d, "c.flac"));
    let mut store = Store::open(&d.0).expect("open");
    let evening = store.create_playlist("Evening").expect("create");
    let gone = store.create_playlist("Gone soon").expect("create");
    let added = add_tracks(
        &mut store,
        evening,
        &[track_from_path(a.clone()), streamed()],
    );
    assert_eq!(added, Ok(2));
    assert_eq!(
        add_files(&mut store, evening, &[b.clone(), c.clone()]),
        Ok(2)
    );
    store.rename_playlist(evening, "Night").expect("rename");
    store.move_entry(evening, 3, 0).expect("move");
    store.remove_entry(evening, 2).expect("remove");
    store.delete_playlist(gone).expect("delete");
    drop(store);

    let store = Store::open(&d.0).expect("reopen");
    let lists = store.playlists().expect("playlists");
    assert_eq!(lists.len(), 1);
    assert_eq!((lists[0].id, lists[0].name.as_str()), (evening, "Night"));
    assert_eq!(paths(&store, 0), vec![Some(c), Some(a), Some(b)]);
    assert!(lists[0].entries.iter().all(|e| !e.missing));
}

#[test]
fn a_failed_add_keeps_nothing() {
    let d = dir("failed");
    let a = file(&d, "a.flac");
    let mut store = Store::open(&d.0).expect("open");
    assert_eq!(
        add_files(&mut store, LocalPlaylistId(42), &[a]),
        Err(LibraryError::Db),
        "no such playlist"
    );
    assert!(store.playlists().expect("playlists").is_empty());
}

#[test]
fn an_entry_whose_file_is_gone_stays_flagged_missing_through_a_rescan() {
    let d = dir("missing");
    let music = d.0.join("Music");
    let (here, gone) = (file(&d, "here.flac"), file(&d, "gone.flac"));
    let mut store = Store::open(&d.0).expect("open");
    store.add_folder(&music).expect("folder");
    store
        .write_tracks(&music, &[row(&here), row(&gone)])
        .expect("scan");
    let id = store.create_playlist("Mixed").expect("create");
    add_files(&mut store, id, &[here.clone(), gone.clone()]).expect("add");
    add_tracks(&mut store, id, &[streamed()]).expect("add");

    // The file goes, and the rescan drops its track row.
    std::fs::remove_file(&gone).expect("delete the test file");
    assert_eq!(store.remove_files(std::slice::from_ref(&gone)), Ok(1));
    drop(store);
    let mut store = Store::open(&d.0).expect("reopen");
    let entries = store.playlists().expect("playlists")[0].entries.clone();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[1].path.as_ref(), Some(&gone));
    let missing: Vec<bool> = entries.iter().map(|e| e.missing).collect();
    assert_eq!(missing, vec![false, true, false]);
    assert_eq!(entries[2].track, Some(streamed()));

    // The file comes back: the same entry plays again.
    file(&d, "gone.flac");
    store.write_tracks(&music, &[row(&gone)]).expect("rescan");
    let entries = store.playlists().expect("playlists")[0].entries.clone();
    assert!(entries.iter().all(|e| !e.missing));
}
