//! Install the official yt-dlp "onedir" Windows build (`yt-dlp_win.zip`).
//!
//! The one-file `yt-dlp.exe` (winget, `PyInstaller`) unpacks itself into a temp folder on every
//! run, about 4 s on the owner's laptop (perf-notes.md). The onedir build is the same program
//! already unpacked, so a run starts at once.
//!
//! [`install_onedir`] is never called by the app on its own: the user (or a developer, see
//! `examples/install_yt_dlp.rs`) starts it. It downloads `SHA2-256SUMS` and `yt-dlp_win.zip`
//! from the latest GitHub release, checks the zip's SHA-256 against the list, unpacks it with
//! the Windows `tar.exe` into a staging folder and only then swaps it into place.

use std::fmt::Write as _;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::http::{USER_AGENT, https_redirects};

/// Release asset with the onedir build for 64-bit Windows.
pub const ASSET: &str = "yt-dlp_win.zip";
/// Base of the latest-release download links.
pub const RELEASE_BASE: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/";
/// Largest zip accepted, in bytes (the 2026.10.26 asset is about 24 MB).
const MAX_ZIP_BYTES: u64 = 200 * 1024 * 1024;
/// Largest checksum list accepted, in bytes.
const MAX_SUMS_BYTES: u64 = 64 * 1024;

/// Installing the onedir build failed. Texts are fixed and hold no URL.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InstallError {
    /// A download failed.
    #[error("download failed: {0}")]
    Download(String),
    /// The checksum list has no line for the zip.
    #[error("the checksum list does not name {ASSET}")]
    NoChecksum,
    /// The zip does not match its published SHA-256.
    #[error("the download does not match its published SHA-256")]
    ChecksumMismatch,
    /// Unpacking failed or the archive has no `yt-dlp.exe`.
    #[error("could not unpack the archive: {0}")]
    Unpack(String),
    /// A file operation failed.
    #[error("file error: {0}")]
    Io(String),
}

fn io_err(what: &str) -> impl Fn(std::io::Error) -> InstallError + '_ {
    move |e| InstallError::Io(format!("{what}: {}", e.kind()))
}

/// The SHA-256 the checksum list gives for `name`, lower-case hex.
///
/// Lines look like `<64 hex>  <file name>` (two spaces, as `sha256sum` writes them; a `*`
/// before the name marks binary mode).
#[must_use]
pub fn checksum_for(list: &str, name: &str) -> Option<String> {
    list.lines().find_map(|line| {
        let (hash, file) = line.trim().split_once(char::is_whitespace)?;
        let file = file.trim().trim_start_matches('*');
        (file == name && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| hash.to_ascii_lowercase())
    })
}

/// Lower-case hex SHA-256 of `bytes`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        let _ = write!(out, "{b:02x}");
    }
    out
}

fn download(
    client: &reqwest::blocking::Client,
    name: &str,
    cap: u64,
) -> Result<Vec<u8>, InstallError> {
    let response = client
        .get(format!("{RELEASE_BASE}{name}"))
        .send()
        .map_err(|_| InstallError::Download(format!("{name}: request failed")))?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(InstallError::Download(format!("{name}: HTTP {status}")));
    }
    let mut bytes = Vec::new();
    response
        .take(cap + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| InstallError::Download(format!("{name}: connection lost")))?;
    if bytes.len() as u64 > cap {
        return Err(InstallError::Download(format!("{name}: too large")));
    }
    Ok(bytes)
}

/// The folder in `root` (or `root` itself, or one level down) that holds `yt-dlp.exe`.
fn program_root(root: &Path) -> Option<PathBuf> {
    if root.join("yt-dlp.exe").is_file() {
        return Some(root.to_path_buf());
    }
    std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.join("yt-dlp.exe").is_file())
}

fn tar_program() -> PathBuf {
    std::env::var_os("SystemRoot").map_or_else(
        || PathBuf::from("tar.exe"),
        |root| PathBuf::from(root).join("System32").join("tar.exe"),
    )
}

/// Download, verify and unpack the onedir build so that `target_dir\yt-dlp.exe` exists.
/// Returns the path of that program. Blocks; call it from a worker thread.
///
/// `target_dir` is normally `%LOCALAPPDATA%\NemoMusic\bin\yt-dlp` (where
/// [`locate_yt_dlp`](crate::ytdlp::locate_yt_dlp) looks first). An existing install there is
/// replaced only after the new one is verified and unpacked.
///
/// # Errors
/// [`InstallError`] when a download fails, the checksum does not match, or unpacking fails.
pub fn install_onedir(target_dir: &Path) -> Result<PathBuf, InstallError> {
    let parent = target_dir
        .parent()
        .ok_or_else(|| InstallError::Io("target has no parent folder".to_owned()))?;
    std::fs::create_dir_all(parent).map_err(io_err("create folder"))?;
    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(300))
        .redirect(https_redirects())
        .https_only(true)
        .build()
        .map_err(|_| InstallError::Download("could not build the HTTP client".to_owned()))?;
    let sums = download(&client, "SHA2-256SUMS", MAX_SUMS_BYTES)?;
    let sums = String::from_utf8_lossy(&sums);
    let expected = checksum_for(&sums, ASSET).ok_or(InstallError::NoChecksum)?;
    std::thread::sleep(Duration::from_secs(1));
    let zip = download(&client, ASSET, MAX_ZIP_BYTES)?;
    if sha256_hex(&zip) != expected {
        return Err(InstallError::ChecksumMismatch);
    }

    let zip_path = parent.join("yt-dlp_win.zip.partial");
    let staging = parent.join("yt-dlp.staging");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(io_err("create staging folder"))?;
    std::fs::write(&zip_path, &zip).map_err(io_err("write archive"))?;
    let status = Command::new(tar_program())
        .arg("-xf")
        .arg(&zip_path)
        .arg("-C")
        .arg(&staging)
        .status()
        .map_err(|e| InstallError::Unpack(format!("tar.exe: {}", e.kind())))?;
    let _ = std::fs::remove_file(&zip_path);
    if !status.success() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(InstallError::Unpack("tar.exe reported an error".to_owned()));
    }
    let root = program_root(&staging)
        .ok_or_else(|| InstallError::Unpack("no yt-dlp.exe in the archive".to_owned()))?;

    let old = parent.join("yt-dlp.old");
    let _ = std::fs::remove_dir_all(&old);
    if target_dir.exists() {
        std::fs::rename(target_dir, &old).map_err(io_err("move the old install"))?;
    }
    if let Err(e) = std::fs::rename(&root, target_dir) {
        if old.exists() {
            let _ = std::fs::rename(&old, target_dir);
        }
        return Err(io_err("move the new install")(e));
    }
    let _ = std::fs::remove_dir_all(&old);
    let _ = std::fs::remove_dir_all(&staging);
    Ok(target_dir.join("yt-dlp.exe"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn the_checksum_line_of_the_asset_is_found() {
        let list = format!(
            "{}  yt-dlp\n{HASH}  yt-dlp_win.zip\n{}  yt-dlp_win_x86.zip\n",
            "f".repeat(64),
            "e".repeat(64)
        );
        assert_eq!(checksum_for(&list, ASSET).as_deref(), Some(HASH));
        let starred = format!("{}  *yt-dlp_win.zip\r\n", HASH.to_ascii_uppercase());
        assert_eq!(checksum_for(&starred, ASSET).as_deref(), Some(HASH));
        assert_eq!(checksum_for("abc  yt-dlp_win.zip", ASSET), None);
        assert_eq!(checksum_for(&format!("{HASH}  other.zip"), ASSET), None);
        assert_eq!(checksum_for("", ASSET), None);
    }

    #[test]
    fn sha256_of_a_known_text() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn the_program_folder_is_found_at_the_root_or_one_level_down() {
        let base = std::env::temp_dir().join(format!("nemo-onedir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let nested = base.join("yt-dlp_win");
        std::fs::create_dir_all(&nested).expect("dirs");
        assert_eq!(program_root(&base), None);
        std::fs::write(nested.join("yt-dlp.exe"), b"x").expect("file");
        assert_eq!(program_root(&base), Some(nested.clone()));
        std::fs::write(base.join("yt-dlp.exe"), b"x").expect("file");
        assert_eq!(program_root(&base), Some(base.clone()));
        let _ = std::fs::remove_dir_all(&base);
    }
}
