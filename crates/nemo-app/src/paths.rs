//! Where `NemoMusic` keeps its local data (`product.md`, Local only).
//!
//! Everything lives under one root so that sign-out and "clear all data" can remove it in one
//! place. On Windows the root is `%LOCALAPPDATA%\NemoMusic` (not roaming, see ADR 0001); on
//! Linux it is `$XDG_DATA_HOME/NemoMusic` or `~/.local/share/NemoMusic`.

use std::path::{Path, PathBuf};

/// Folder name under the platform data directory.
pub const APP_DIR: &str = "NemoMusic";

/// The app's local folders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    /// Root of all local data.
    pub root: PathBuf,
    /// Cover art cache.
    pub images: PathBuf,
    /// Lyrics cache and per-track offsets (SQLite).
    pub lyrics: PathBuf,
    /// Local library database.
    pub library: PathBuf,
    /// Log files. Never hold secrets.
    pub logs: PathBuf,
    /// Optional helper programs, such as a private `yt-dlp.exe`.
    pub bin: PathBuf,
    /// The sign-in window's own `WebView2` profile. Sign-out deletes it.
    pub webview: PathBuf,
    /// Cached account header (name, avatar). Sign-out deletes it.
    pub account: PathBuf,
    /// Offline cache of saved songs: files with opaque names and `offline.db`. The cache
    /// creates it, not [`AppPaths::create_all`]; sign-out removes it completely.
    pub offline: PathBuf,
}

/// No data directory could be found for this user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "no local data folder is set for this user (LOCALAPPDATA, or XDG_DATA_HOME and HOME, are missing)"
)]
pub struct NoDataDir;

impl AppPaths {
    /// Lay out the folders under `root`. Creates nothing.
    #[must_use]
    pub fn under(root: &Path) -> Self {
        let root = root.to_path_buf();
        Self {
            images: root.join("cache").join("images"),
            lyrics: root.join("lyrics"),
            library: root.join("library"),
            logs: root.join("logs"),
            bin: root.join("bin"),
            webview: root.join("webview"),
            account: root.join("account"),
            offline: root.join("offline"),
            root,
        }
    }

    /// The platform default. `override_root` wins (used by tests and the `--data-dir` flag).
    ///
    /// # Errors
    /// Returns [`NoDataDir`] when no override is given and the platform folder is unknown.
    pub fn resolve(override_root: Option<&Path>) -> Result<Self, NoDataDir> {
        if let Some(root) = override_root {
            return Ok(Self::under(root));
        }
        platform_data_dir()
            .map(|base| Self::under(&base.join(APP_DIR)))
            .ok_or(NoDataDir)
    }

    /// Create every folder that does not exist yet.
    ///
    /// # Errors
    /// Returns the I/O error of the first folder that cannot be created.
    pub fn create_all(&self) -> std::io::Result<()> {
        for dir in [
            &self.images,
            &self.lyrics,
            &self.library,
            &self.logs,
            &self.bin,
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}

/// `%LOCALAPPDATA%` on Windows, `$XDG_DATA_HOME` or `~/.local/share` elsewhere.
fn platform_data_dir() -> Option<PathBuf> {
    let non_empty = |name: &str| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if cfg!(windows) {
        non_empty("LOCALAPPDATA")
    } else {
        non_empty("XDG_DATA_HOME")
            .or_else(|| non_empty("HOME").map(|h| h.join(".local").join("share")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_folder_sits_under_the_root() {
        let root = Path::new("C:/data/NemoMusic");
        let p = AppPaths::under(root);
        for dir in [
            &p.images, &p.lyrics, &p.library, &p.logs, &p.bin, &p.webview, &p.account, &p.offline,
        ] {
            assert!(dir.starts_with(root), "{}", dir.display());
        }
        assert_eq!(p.root, root);
    }

    #[test]
    fn offline_cache_has_its_own_folder_under_the_root() {
        let root = Path::new("C:/data/NemoMusic");
        let p = AppPaths::under(root);
        assert_eq!(p.offline, root.join("offline"));
        for other in [
            &p.images, &p.lyrics, &p.library, &p.logs, &p.bin, &p.webview, &p.account,
        ] {
            assert!(!other.starts_with(&p.offline) && !p.offline.starts_with(other));
        }
    }

    #[test]
    fn override_wins_over_the_platform_folder() {
        let p = AppPaths::resolve(Some(Path::new("D:/elsewhere"))).expect("override");
        assert_eq!(p.root, Path::new("D:/elsewhere"));
    }

    #[test]
    fn create_all_makes_the_folders() {
        let root = std::env::temp_dir().join(format!("nemo-app-paths-{}", std::process::id()));
        let p = AppPaths::under(&root);
        p.create_all().expect("create");
        assert!(p.images.is_dir() && p.lyrics.is_dir() && p.logs.is_dir());
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
