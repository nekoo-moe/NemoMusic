//! The local library in memory: scan folders, read tags, search.
//!
//! A SQLite store that remembers scans across restarts comes later. Scanning a few thousand
//! files on start is fast enough for the MVP; run it on a worker thread, never on the UI thread.

use std::path::{Path, PathBuf};

use nemo_core::track::Track;

use crate::scan::scan_folder;
use crate::tags::{apply_tags, read_tags};

/// Tracks found in the user's music folders.
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
        self.tracks.sort_by(|a, b| {
            sort_key(a)
                .cmp(&sort_key(b))
                .then_with(|| a.local_path.cmp(&b.local_path))
        });
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

    #[test]
    fn contains_path_checks_the_music_folders() {
        let lib = LocalLibrary::new(vec![PathBuf::from("C:/Music")]);
        assert!(lib.contains_path(Path::new("C:/Music/a/b.flac")));
        assert!(!lib.contains_path(Path::new("C:/Windows/b.flac")));
    }
}
