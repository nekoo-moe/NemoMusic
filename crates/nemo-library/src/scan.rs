//! Find audio files in a folder tree.
//!
//! Pure `std`, no tags yet: titles come from the file name. Tag reading with `lofty` and the
//! SQLite store come in a later task.

use std::path::{Path, PathBuf};

use nemo_core::audio::Origin;
use nemo_core::track::{Artwork, Track, TrackId};

/// File extensions the audio engine can play from disk (lowercase, no dot).
pub const AUDIO_EXTENSIONS: &[&str] = &["flac", "wav", "m4a", "mp3", "ogg", "opus"];

/// Deepest folder level the scan enters below the root. Stops runaway scans.
pub const MAX_DEPTH: usize = 16;

/// Most files a single scan returns.
pub const MAX_FILES: usize = 50_000;

/// What a scan found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScanReport {
    /// Playable tracks, sorted by path.
    pub tracks: Vec<Track>,
    /// Folders or files that could not be read. Paths only, never contents.
    pub unreadable: Vec<PathBuf>,
    /// `true` when [`MAX_FILES`] was reached and the scan stopped early.
    pub truncated: bool,
}

/// `true` when the file name has a playable audio extension.
#[must_use]
pub fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTENSIONS.iter().any(|a| a.eq_ignore_ascii_case(e)))
}

/// Scan `root` for audio files. Symbolic links are not followed, so a link loop cannot hang.
#[must_use]
pub fn scan_folder(root: &Path) -> ScanReport {
    let mut report = ScanReport::default();
    let mut stack = vec![(root.to_path_buf(), 0_usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            report.unreadable.push(dir);
            continue;
        };
        for entry in entries {
            let Ok(entry) = entry else {
                report.unreadable.push(dir.clone());
                continue;
            };
            let Ok(kind) = entry.file_type() else {
                report.unreadable.push(entry.path());
                continue;
            };
            let path = entry.path();
            if kind.is_dir() {
                if depth < MAX_DEPTH {
                    stack.push((path, depth + 1));
                }
            } else if kind.is_file() && is_audio_file(&path) {
                if report.tracks.len() >= MAX_FILES {
                    report.truncated = true;
                    break;
                }
                report.tracks.push(track_from_path(path));
            }
        }
        if report.truncated {
            break;
        }
    }
    report
        .tracks
        .sort_by(|a, b| a.local_path.cmp(&b.local_path));
    report
}

/// Build a track from a file path. The title is the file stem; artist and album stay empty
/// until tags are read. The cover is looked up from the file's own tags later.
#[must_use]
pub fn track_from_path(path: PathBuf) -> Track {
    let title = path.file_stem().map_or_else(
        || "Unknown".to_owned(),
        |s| s.to_string_lossy().into_owned(),
    );
    Track {
        id: TrackId(format!("local:{}", path.display())),
        title,
        artists: Vec::new(),
        album: None,
        duration: None,
        origin: Origin::Local,
        video_id: None,
        artwork: Artwork::LocalFile(path.clone()),
        source_url: None,
        local_path: Some(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nemo-library-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir); // A leftover from an earlier run is fine to drop.
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn extension_check_ignores_case_and_rejects_others() {
        assert!(is_audio_file(Path::new("a/b/Song.FLAC")));
        assert!(is_audio_file(Path::new("x.m4a")));
        assert!(!is_audio_file(Path::new("cover.jpg")));
        assert!(!is_audio_file(Path::new("noext")));
        assert!(!is_audio_file(Path::new("flac")));
    }

    #[test]
    fn scan_finds_nested_audio_and_skips_other_files() {
        let root = temp_root("scan");
        std::fs::create_dir_all(root.join("Album A/Disc 1")).expect("dirs");
        for f in [
            "Album A/Disc 1/01 First.flac",
            "Album A/02 Second.wav",
            "notes.txt",
            "cover.jpg",
        ] {
            std::fs::write(root.join(f), b"x").expect("write");
        }
        let report = scan_folder(&root);
        let titles: Vec<_> = report.tracks.iter().map(|t| t.title.as_str()).collect();
        // Sorted by path: "Album A/02 Second.wav" comes before "Album A/Disc 1/...".
        assert_eq!(titles, ["02 Second", "01 First"]);
        assert!(report.unreadable.is_empty() && !report.truncated);
        let t = &report.tracks[0];
        assert_eq!(t.origin, Origin::Local);
        assert!(t.video_id.is_none());
        assert_eq!(
            t.artwork,
            Artwork::LocalFile(t.local_path.clone().expect("path"))
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn missing_root_is_reported_not_panicked() {
        let report = scan_folder(Path::new("Z:/this/folder/does/not/exist/nemo"));
        assert_eq!(report.tracks, Vec::new());
        assert_eq!(report.unreadable.len(), 1);
    }
}
