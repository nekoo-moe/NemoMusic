//! The local library in memory: the snapshot the catalog reads, filled from `library.db`
//! at start ([`LocalLibrary::load`]) or by a folder scan ([`LocalLibrary::rescan`]).
//!
//! Loading reads every stored row on a worker thread, so the library shows at start without a
//! rescan (criterion 5). Both block; never call them on the UI thread.

use std::path::{Path, PathBuf};

use nemo_core::audio::Origin;
use nemo_core::library::{LibraryError, TrackStatus};
use nemo_core::track::{Artwork, Track, TrackId};

use crate::scan::scan_folder;
use crate::store::{Store, TrackRow};
use crate::tags::{apply_tags, read_tags};

/// Tracks found in the user's music folders: the snapshot [`crate::LocalCatalog`] reads.
#[derive(Debug, Clone, Default)]
pub struct LocalLibrary {
    folders: Vec<PathBuf>,
    tracks: Vec<Track>,
    unreadable: Vec<PathBuf>,
}

impl LocalLibrary {
    /// An empty library for these folders. Call [`LocalLibrary::rescan`] to fill it.
    #[must_use]
    pub fn new(folders: Vec<PathBuf>) -> Self {
        Self {
            folders,
            ..Self::default()
        }
    }

    /// The snapshot stored in `library.db`: its folders and every track whose file was there
    /// at the last scan, sorted as [`LocalLibrary::rescan`] sorts. Reads only; blocks, so call
    /// it from a worker thread with a reader ([`Store::open_reader`]).
    ///
    /// # Errors
    /// [`LibraryError::Db`] when the rows cannot be read.
    pub fn load(store: &Store) -> Result<Self, LibraryError> {
        let folders = store.folders()?;
        let mut tracks: Vec<Track> = store
            .all_tracks()?
            .into_iter()
            .filter(|row| row.status != TrackStatus::Missing)
            .map(track_of)
            .collect();
        sort_tracks(&mut tracks);
        Ok(Self {
            folders,
            tracks,
            unreadable: Vec::new(),
        })
    }

    /// Scan every folder and read tags. Blocks; call it from a worker thread.
    pub fn rescan(&mut self) {
        self.tracks.clear();
        self.unreadable.clear();
        for folder in &self.folders {
            let report = scan_folder(folder);
            self.unreadable.extend(report.unreadable);
            for mut track in report.tracks {
                if let Some(path) = track.local_path.clone()
                    && let Some(info) = read_tags(&path)
                {
                    apply_tags(&mut track, info);
                }
                self.tracks.push(track);
            }
        }
        sort_tracks(&mut self.tracks);
    }

    /// The music folders.
    #[must_use]
    pub fn folders(&self) -> &[PathBuf] {
        &self.folders
    }

    /// Every track, sorted by artist, album, then path.
    #[must_use]
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    /// Paths that could not be read during the last scan.
    #[must_use]
    pub fn unreadable(&self) -> &[PathBuf] {
        &self.unreadable
    }

    /// Tracks whose title, artist or album contains every word of `query` (case-insensitive).
    /// An empty query returns nothing.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&Track> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        if words.is_empty() {
            return Vec::new();
        }
        self.tracks
            .iter()
            .filter(|t| {
                let hay = haystack(t);
                words.iter().all(|w| hay.contains(w.as_str()))
            })
            .collect()
    }

    /// Whether a path belongs to one of the music folders. Use it before playing a path that
    /// came from outside the library.
    #[must_use]
    pub fn contains_path(&self, path: &Path) -> bool {
        self.folders.iter().any(|f| path.starts_with(f))
    }
}

/// A stored row as the catalog lists it. A CUE track (`cue_index` n > 0) gets `#n` after the
/// path in its id, so the tracks of one file stay apart.
fn track_of(row: TrackRow) -> Track {
    let TrackRow {
        path,
        cue_index,
        title,
        artists,
        album,
        duration,
        ..
    } = row;
    // The id `track_from_path` gives, so a loaded track and a scanned one match.
    let id = if cue_index > 0 {
        format!("local:{}#{cue_index}", path.display())
    } else {
        format!("local:{}", path.display())
    };
    Track {
        id: TrackId(id),
        title,
        artists,
        album,
        duration,
        origin: Origin::Local,
        video_id: None,
        artwork: Artwork::LocalFile(path.clone()),
        source_url: None,
        local_path: Some(path),
    }
}

/// By artist, then album (ignoring case), then path; equal keys keep their order.
fn sort_tracks(tracks: &mut [Track]) {
    tracks.sort_by_cached_key(|t| (sort_key(t), t.local_path.clone()));
}

fn sort_key(t: &Track) -> (String, String) {
    (
        t.artists
            .first()
            .map(|a| a.to_lowercase())
            .unwrap_or_default(),
        t.album
            .as_deref()
            .map(str::to_lowercase)
            .unwrap_or_default(),
    )
}

fn haystack(t: &Track) -> String {
    let mut s = t.title.to_lowercase();
    for a in &t.artists {
        s.push(' ');
        s.push_str(&a.to_lowercase());
    }
    if let Some(album) = &t.album {
        s.push(' ');
        s.push_str(&album.to_lowercase());
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library_in(tag: &str, files: &[&str]) -> (PathBuf, LocalLibrary) {
        let root = std::env::temp_dir().join(format!("nemo-lib-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root); // A leftover from an earlier run is fine to drop.
        for f in files {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().expect("parent")).expect("dirs");
            std::fs::write(p, b"x").expect("write");
        }
        let mut lib = LocalLibrary::new(vec![root.clone()]);
        lib.rescan();
        (root, lib)
    }

    #[test]
    fn rescan_collects_tracks_and_survives_unparsable_files() {
        let (root, lib) = library_in("rescan", &["a/Night Drive.flac", "b/Morning.wav", "c.txt"]);
        assert_eq!(lib.tracks().len(), 2);
        assert!(lib.tracks().iter().all(|t| t.artists.is_empty()));
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn search_matches_every_word_ignoring_case() {
        let (root, lib) = library_in(
            "search",
            &["Night Drive.flac", "Night Swim.flac", "Day.flac"],
        );
        let hits: Vec<_> = lib
            .search("night DRIVE")
            .iter()
            .map(|t| t.title.clone())
            .collect();
        assert_eq!(hits, ["Night Drive"]);
        assert_eq!(lib.search("night").len(), 2);
        assert_eq!(lib.search("   ").len(), 0);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    fn stored(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nemo-lib-db-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir); // A leftover from an earlier run is fine to drop.
        dir
    }

    fn row(path: &str, artist: &str, album: Option<&str>) -> TrackRow {
        TrackRow {
            path: PathBuf::from(path),
            title: Path::new(path)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            artists: vec![artist.to_owned()],
            album: album.map(str::to_owned),
            duration: Some(std::time::Duration::from_secs(90)),
            ..TrackRow::default()
        }
    }

    #[test]
    fn load_reads_the_stored_rows_sorted_without_touching_the_disk() {
        let dir = stored("load");
        let music = PathBuf::from("/nowhere/Music");
        let mut store = Store::open(&dir).expect("open");
        store.add_folder(&music).expect("folder");
        let mut gone = row("/nowhere/Music/gone.flac", "A", None);
        gone.status = TrackStatus::Missing;
        let mut cue = row("/nowhere/Music/live.flac", "B", Some("Live"));
        cue.cue_index = 2;
        cue.title = "Second".to_owned();
        let mut broken = row("/nowhere/Music/broken.ogg", "c", None);
        broken.status = TrackStatus::Unplayable(nemo_core::library::UnplayableReason::DecodeFailed);
        let rows = [
            row("/nowhere/Music/z.flac", "b", Some("live")),
            gone,
            cue,
            broken,
            row("/nowhere/Music/a.flac", "B", Some("Live")),
        ];
        store.write_tracks(&music, &rows).expect("write");
        drop(store);

        let lib = LocalLibrary::load(&Store::open_reader(&dir).expect("reader")).expect("load");
        assert_eq!(lib.folders(), [music]);
        let ids: Vec<&str> = lib.tracks().iter().map(|t| t.id.0.as_str()).collect();
        assert_eq!(
            ids,
            [
                "local:/nowhere/Music/a.flac",
                "local:/nowhere/Music/live.flac#2",
                "local:/nowhere/Music/z.flac",
                "local:/nowhere/Music/broken.ogg",
            ],
            "by artist and album ignoring case, then path; the missing file is left out"
        );
        let first = &lib.tracks()[0];
        let scanned = crate::scan::track_from_path(PathBuf::from("/nowhere/Music/a.flac"));
        assert_eq!(first.id, scanned.id, "a loaded track has the scanned id");
        assert_eq!(first.artwork, scanned.artwork);
        assert_eq!(first.local_path, scanned.local_path);
        assert_eq!(
            (first.title.as_str(), first.album.as_deref()),
            ("a", Some("Live"))
        );
        assert_eq!(first.duration, Some(std::time::Duration::from_secs(90)));
        assert_eq!(lib.search("second").len(), 1);
        let _ = std::fs::remove_dir_all(dir); // Test cleanup; a leftover is harmless.
    }

    /// Criterion 5: shown within one second of start. Timed in release builds only.
    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "timing budget holds for release builds only"
    )]
    fn fifty_thousand_rows_load_within_the_one_second_budget() {
        let dir = stored("budget");
        let music = PathBuf::from("/nowhere/Music");
        let mut store = Store::open(&dir).expect("open");
        store.add_folder(&music).expect("folder");
        let rows: Vec<TrackRow> = (0..crate::scan::MAX_FILES)
            .map(|i| {
                let mut r = row(
                    &format!(
                        "/nowhere/Music/Artist {}/Album {}/{i:05} Song.flac",
                        i % 900,
                        i % 4000
                    ),
                    &format!("Artist {}", i % 900),
                    Some(&format!("Album {}", i % 4000)),
                );
                r.genre = Some("Ambient".to_owned());
                r.year = Some(2026);
                r
            })
            .collect();
        store.write_tracks(&music, &rows).expect("write");
        drop(store);

        let start = std::time::Instant::now();
        let lib = LocalLibrary::load(&Store::open_reader(&dir).expect("reader")).expect("load");
        let took = start.elapsed();
        assert_eq!(lib.tracks().len(), crate::scan::MAX_FILES);
        assert!(
            took < std::time::Duration::from_secs(1),
            "loaded in {took:?}"
        );
        let _ = std::fs::remove_dir_all(dir); // Test cleanup; a leftover is harmless.
    }

    #[test]
    fn contains_path_checks_the_music_folders() {
        let lib = LocalLibrary::new(vec![PathBuf::from("C:/Music")]);
        assert!(lib.contains_path(Path::new("C:/Music/a/b.flac")));
        assert!(!lib.contains_path(Path::new("C:/Windows/b.flac")));
    }
}
