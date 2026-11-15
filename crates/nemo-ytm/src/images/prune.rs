//! One-time removal of cover files from the cache format before sharp covers.
//!
//! Before sharp covers, a file was named after the SHA-256 hex of the listed (small) thumbnail
//! address. Those files are low resolution and are no longer looked up first, so they only take
//! space. Files now start with [`FILE_PREFIX`]. At the first start with this format, every regular
//! file in the image cache folder whose name does not match it is deleted, then [`MARKER`] is
//! written so this runs once. Folders, links and anything outside the cache folder are never
//! touched.

use std::path::Path;

/// Prefix of every cache file name in the current format.
pub const FILE_PREFIX: &str = "v2-";

/// Empty file that records that old files were pruned.
pub const MARKER: &str = "format-v2";

/// Suffixes a current-format name may carry after the hash.
const SUFFIXES: [&str; 3] = ["", ".missing", ".tmp"];

/// True for `v2-<64 lower-case hex>` with an optional `.missing` or `.tmp` suffix.
#[must_use]
pub fn is_current_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix(FILE_PREFIX) else {
        return false;
    };
    SUFFIXES.iter().any(|suffix| {
        rest.strip_suffix(suffix).is_some_and(|hex| {
            hex.len() == 64 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        })
    })
}

/// Delete old-format files in `dir` once. Returns how many were deleted (`0` when the marker
/// already exists). Blocking: call it on a worker thread.
///
/// # Errors
/// The I/O error when the folder cannot be listed or the marker cannot be written; the prune is
/// then tried again at the next start. A file that cannot be deleted is skipped.
pub fn prune_old_files(dir: &Path) -> std::io::Result<usize> {
    let marker = dir.join(MARKER);
    if marker.is_file() {
        return Ok(0);
    }
    let mut removed = 0;
    for entry in std::fs::read_dir(dir)? {
        let Ok(entry) = entry else { continue };
        // `DirEntry::file_type` does not follow links: only real files are candidates.
        let is_file = entry.file_type().is_ok_and(|t| t.is_file());
        let name = entry.file_name();
        let keep = name
            .to_str()
            .is_some_and(|n| n == MARKER || is_current_name(n));
        if is_file && !keep && std::fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    std::fs::write(&marker, [])?;
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::{MARKER, is_current_name, prune_old_files};
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            // Only digits, letters and dashes: `{:?}` of a time holds `:`, which Windows
            // does not allow in a folder name.
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos());
            let dir = std::env::temp_dir().join(format!(
                "nemo-prune-{tag}-{}-{nanos}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).expect("temp dir");
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            // Cleanup of a test folder; a leftover only wastes temp space.
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn names(dir: &std::path::Path) -> Vec<String> {
        let mut out: Vec<String> = std::fs::read_dir(dir)
            .expect("read dir")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        out.sort();
        out
    }

    #[test]
    fn names_of_the_current_format() {
        let hex = "a".repeat(64);
        assert!(is_current_name(&format!("v2-{hex}")));
        assert!(is_current_name(&format!("v2-{hex}.missing")));
        assert!(is_current_name(&format!("v2-{hex}.tmp")));
        assert!(!is_current_name(&hex));
        assert!(!is_current_name(&format!("v2-{}", "A".repeat(64))));
        assert!(!is_current_name(&format!("v2-{hex}.jpg")));
        assert!(!is_current_name("v2-abc"));
    }

    #[test]
    fn old_cover_files_are_pruned_once_and_new_ones_kept() {
        let dir = TempDir::new("once");
        let hex = "0123456789abcdef".repeat(4);
        let old = [hex.clone(), format!("{hex}.tmp"), "stray.bin".to_owned()];
        let new = [format!("v2-{hex}"), format!("v2-{hex}.missing")];
        for name in old.iter().chain(&new) {
            std::fs::write(dir.0.join(name), b"x").expect("write");
        }
        std::fs::create_dir(dir.0.join("folder")).expect("sub folder");

        assert_eq!(prune_old_files(&dir.0).expect("prune"), 3);
        let mut expected = vec![
            "folder".to_owned(),
            MARKER.to_owned(),
            new[0].clone(),
            new[1].clone(),
        ];
        expected.sort();
        assert_eq!(names(&dir.0), expected, "folders and new files stay");

        // The marker makes the second start a no-op, even for a file of the old shape.
        std::fs::write(dir.0.join(&old[0]), b"x").expect("write");
        assert_eq!(prune_old_files(&dir.0).expect("prune"), 0);
        assert!(dir.0.join(&old[0]).is_file());
    }

    #[test]
    fn a_missing_folder_is_an_error_and_writes_nothing() {
        let dir = TempDir::new("gone");
        let gone = dir.0.join("nope");
        assert!(prune_old_files(&gone).is_err());
        assert!(!gone.exists());
    }
}
