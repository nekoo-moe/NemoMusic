//! Install the official yt-dlp release builds into the app's own folder.
//!
//! Windows x64: the "onedir" build (`yt-dlp_win.zip`). The one-file `yt-dlp.exe` (winget,
//! `PyInstaller`) unpacks itself into a temp folder on every run, about 4 s on the owner's
//! laptop (perf-notes.md). The onedir build is the same program already unpacked, so a run
//! starts at once. Linux `x86_64`: the same onedir build as `yt-dlp_linux.zip`, unpacked with
//! the system `unzip` (no zip crate is approved); without `unzip` the one-file `yt-dlp_linux`
//! build, which unpacks itself on every run (about 0.9 s here, 2026-11-19).
//!
//! Every install downloads `SHA2-256SUMS` and the asset from the latest GitHub release, checks
//! the asset's SHA-256 against the list, and only then puts it in place (a staging folder or a
//! temp file, then a rename). The app calls this through [`crate::ytdlp::managed`] on a worker
//! thread; `examples/install_yt_dlp.rs` runs it by hand. Only TLS and the published SHA-256
//! list protect the download; the release's GPG signature is not checked.

use std::fmt::Write as _;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::http::{USER_AGENT, https_redirects};

/// Release asset with the onedir build for 64-bit Windows.
pub const ASSET: &str = "yt-dlp_win.zip";
/// Release asset with the one-file build for 64-bit (`x86_64`) Linux.
pub const LINUX_ASSET: &str = "yt-dlp_linux";
/// Release asset with the onedir build for 64-bit (`x86_64`) Linux.
pub const LINUX_ZIP_ASSET: &str = "yt-dlp_linux.zip";
/// The release's SHA-256 list.
pub const SUMS: &str = "SHA2-256SUMS";
/// Base of the latest-release download links.
pub const RELEASE_BASE: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/";
/// Largest asset accepted, in bytes (the 2026.10.26 zip is about 24 MB).
const MAX_ASSET_BYTES: u64 = 200 * 1024 * 1024;
/// Largest checksum list accepted, in bytes.
const MAX_SUMS_BYTES: u64 = 64 * 1024;

/// Installing the onedir build failed. Texts are fixed and hold no URL.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InstallError {
    /// A download failed.
    #[error("download failed: {0}")]
    Download(String),
    /// The checksum list has no line for the asset.
    #[error("the checksum list does not name the release asset")]
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

/// The folder in `root` (or `root` itself, or one level down) that holds the file `program`.
fn program_root(root: &Path, program: &str) -> Option<PathBuf> {
    if root.join(program).is_file() {
        return Some(root.to_path_buf());
    }
    std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.join(program).is_file())
}

fn tar_program() -> PathBuf {
    std::env::var_os("SystemRoot").map_or_else(
        || PathBuf::from("tar.exe"),
        |root| PathBuf::from(root).join("System32").join("tar.exe"),
    )
}

/// The HTTPS-only client used for release downloads: [`USER_AGENT`], at most
/// [`MAX_REDIRECTS`](crate::http::MAX_REDIRECTS) redirects, each on HTTPS.
pub(crate) fn client() -> Result<reqwest::blocking::Client, InstallError> {
    reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_mins(5))
        .redirect(https_redirects())
        .https_only(true)
        .build()
        .map_err(|_| InstallError::Download("could not build the HTTP client".to_owned()))
}

/// Download the release's [`SUMS`] list.
pub(crate) fn fetch_sums(client: &reqwest::blocking::Client) -> Result<String, InstallError> {
    let sums = download(client, SUMS, MAX_SUMS_BYTES)?;
    Ok(String::from_utf8_lossy(&sums).into_owned())
}

/// Fail with [`InstallError::ChecksumMismatch`] unless `bytes` hash to `expected`.
pub(crate) fn verify(bytes: &[u8], expected: &str) -> Result<(), InstallError> {
    if sha256_hex(bytes) == expected {
        Ok(())
    } else {
        Err(InstallError::ChecksumMismatch)
    }
}

/// Download `name` and check it against `expected` (lower-case hex SHA-256).
pub(crate) fn fetch_asset(
    client: &reqwest::blocking::Client,
    name: &str,
    expected: &str,
) -> Result<Vec<u8>, InstallError> {
    let bytes = download(client, name, MAX_ASSET_BYTES)?;
    verify(&bytes, expected)?;
    Ok(bytes)
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
    install_with(target_dir, ASSET, unpack_onedir)
}

/// Download and verify the Linux one-file build so that `target_dir/yt-dlp` exists (mode
/// 0o755). Returns the path of that program. Blocks; call it from a worker thread.
///
/// # Errors
/// [`InstallError`] when a download fails, the checksum does not match, or a file operation
/// fails.
pub fn install_onefile(target_dir: &Path) -> Result<PathBuf, InstallError> {
    install_with(target_dir, LINUX_ASSET, place_onefile)
}

/// Download, verify and unpack the Linux onedir build so that `target_dir/yt-dlp` and its
/// `_internal` folder exist. Needs `unzip` ([`unzip_on_path`]). Returns the path of the program.
/// Blocks; call it from a worker thread.
///
/// # Errors
/// [`InstallError`] when a download fails, the checksum does not match, `unzip` is missing or
/// fails, or a file operation fails.
pub fn install_linux_onedir(target_dir: &Path) -> Result<PathBuf, InstallError> {
    install_with(target_dir, LINUX_ZIP_ASSET, unpack_linux_onedir)
}

/// True when a program named `unzip` is in a folder of `PATH` (the Linux onedir install needs it).
#[must_use]
pub fn unzip_on_path() -> bool {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .any(|dir| dir.join("unzip").is_file())
}

type Place = fn(&Path, &[u8]) -> Result<PathBuf, InstallError>;

fn install_with(target_dir: &Path, asset: &str, place: Place) -> Result<PathBuf, InstallError> {
    let client = client()?;
    let sums = fetch_sums(&client)?;
    let expected = checksum_for(&sums, asset).ok_or(InstallError::NoChecksum)?;
    // Human-paced: one request at a time, a short pause between them.
    std::thread::sleep(Duration::from_secs(1));
    let bytes = fetch_asset(&client, asset, &expected)?;
    place(target_dir, &bytes)
}

/// Write the verified one-file build to `target_dir/yt-dlp` through a temp file and a rename,
/// so a half-written program never sits at the final path.
pub(crate) fn place_onefile(target_dir: &Path, bytes: &[u8]) -> Result<PathBuf, InstallError> {
    std::fs::create_dir_all(target_dir).map_err(io_err("create folder"))?;
    let program = target_dir.join("yt-dlp");
    let partial = target_dir.join("yt-dlp.partial");
    std::fs::write(&partial, bytes).map_err(io_err("write program"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = std::fs::set_permissions(&partial, std::fs::Permissions::from_mode(0o755)) {
            let _ = std::fs::remove_file(&partial);
            return Err(io_err("mark executable")(e));
        }
    }
    if let Err(e) = std::fs::rename(&partial, &program) {
        let _ = std::fs::remove_file(&partial);
        return Err(io_err("move the program into place")(e));
    }
    Ok(program)
}

/// Unpack the verified onedir zip with the Windows `tar.exe` into a staging folder, then swap
/// it into `target_dir`.
pub(crate) fn unpack_onedir(target_dir: &Path, zip: &[u8]) -> Result<PathBuf, InstallError> {
    let tar = |zip_path: &Path, staging: &Path| {
        Command::new(tar_program())
            .arg("-xf")
            .arg(zip_path)
            .arg("-C")
            .arg(staging)
            .status()
            .map_err(|e| InstallError::Unpack(format!("tar.exe: {}", e.kind())))
    };
    let names = Names {
        partial: "yt-dlp_win.zip.partial",
        in_zip: "yt-dlp.exe",
        installed: "yt-dlp.exe",
    };
    unpack_zip(target_dir, zip, &names, &tar)
}

/// Unpack the verified Linux onedir zip with `unzip` into a staging folder, then swap it into
/// `target_dir` and mark the program executable.
pub(crate) fn unpack_linux_onedir(target_dir: &Path, zip: &[u8]) -> Result<PathBuf, InstallError> {
    let unzip = |zip_path: &Path, staging: &Path| {
        Command::new("unzip")
            .arg("-q")
            .arg("-o")
            .arg(zip_path)
            .arg("-d")
            .arg(staging)
            .status()
            .map_err(|e| InstallError::Unpack(format!("unzip: {}", e.kind())))
    };
    // The zip holds `yt-dlp_linux` and `_internal/`; the app looks for `yt-dlp` (the PyInstaller
    // bootloader finds `_internal` next to the executable whatever its name).
    let names = Names {
        partial: "yt-dlp_linux.zip.partial",
        in_zip: "yt-dlp_linux",
        installed: "yt-dlp",
    };
    let program = unpack_zip(target_dir, zip, &names, &unzip)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
            .map_err(io_err("mark executable"))?;
    }
    Ok(program)
}

/// Runs the extractor: `(zip file, staging folder)` to its exit status.
type Extract<'a> = &'a dyn Fn(&Path, &Path) -> Result<std::process::ExitStatus, InstallError>;

/// File names of one onedir archive.
struct Names {
    /// Where the archive is written while it is unpacked.
    partial: &'static str,
    /// The program's name inside the archive.
    in_zip: &'static str,
    /// The program's name in the installed folder.
    installed: &'static str,
}

/// Write `zip` next to `target_dir`, extract it into a staging folder, find the folder that
/// holds the program, and swap it into `target_dir` (the old install is kept until the new one
/// is in place). Returns the installed program's path.
fn unpack_zip(
    target_dir: &Path,
    zip: &[u8],
    names: &Names,
    extract: Extract<'_>,
) -> Result<PathBuf, InstallError> {
    let parent = target_dir
        .parent()
        .ok_or_else(|| InstallError::Io("target has no parent folder".to_owned()))?;
    std::fs::create_dir_all(parent).map_err(io_err("create folder"))?;
    let zip_path = parent.join(names.partial);
    let staging = parent.join("yt-dlp.staging");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(io_err("create staging folder"))?;
    std::fs::write(&zip_path, zip).map_err(io_err("write archive"))?;
    let status = extract(&zip_path, &staging);
    let _ = std::fs::remove_file(&zip_path);
    let root = match status {
        Ok(status) if status.success() => program_root(&staging, names.in_zip)
            .ok_or_else(|| InstallError::Unpack(format!("no {} in the archive", names.in_zip))),
        Ok(_) => Err(InstallError::Unpack(
            "the extractor reported an error".to_owned(),
        )),
        Err(e) => Err(e),
    };
    let root = root.and_then(|root| {
        if names.in_zip != names.installed {
            std::fs::rename(root.join(names.in_zip), root.join(names.installed))
                .map_err(io_err("rename the program"))?;
        }
        Ok(root)
    });
    let root = match root {
        Ok(root) => root,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(e);
        }
    };

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
    Ok(target_dir.join(names.installed))
}

#[cfg(test)]
mod tests;
