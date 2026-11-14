//! Finding the `yt-dlp` executable.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Folder name winget uses for the `yt-dlp.yt-dlp` package under `...\WinGet\Packages`.
const WINGET_PACKAGE_DIR: &str = "yt-dlp.yt-dlp_Microsoft.Winget.Source_8wekyb3d8bbwe";

/// Folder (under the app's `bin`) of the unpacked official "onedir" Windows build.
pub const ONEDIR_DIR: &str = "yt-dlp";

/// Find the `yt-dlp` executable.
///
/// Linux: `$XDG_DATA_HOME/NemoMusic/bin/yt-dlp/yt-dlp` (or under `~/.local/share`), then every
/// directory on `PATH`, then `$XDG_DATA_HOME/NemoMusic/bin/yt-dlp`. No winget folders.
///
/// Windows order: the unpacked onedir build `%LOCALAPPDATA%\NemoMusic\bin\yt-dlp\yt-dlp.exe` (see
/// [`crate::ytdlp::install_onedir`]; it starts in well under a second because it does not unpack itself
/// on every run, unlike the one-file `yt-dlp.exe`), then every directory on `PATH`, then `%LOCALAPPDATA%\Microsoft\WinGet\Links\yt-dlp.exe`
/// (where winget puts command links), then `%LOCALAPPDATA%\NemoMusic\bin\yt-dlp.exe` (a copy the
/// app downloads), then the winget package folder itself (a fresh install is there before the
/// `Links` shim and `PATH` are refreshed). Returns `None` when none exists.
#[must_use]
pub fn locate_yt_dlp() -> Option<PathBuf> {
    locate_in(
        std::env::var_os("PATH").as_deref(),
        platform_data_dir().as_deref(),
        &Path::is_file,
    )
}

/// `%LOCALAPPDATA%` on Windows, `$XDG_DATA_HOME` or `~/.local/share` elsewhere (the same base
/// folder `nemo-app` uses for its data).
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

fn locate_in(
    path_var: Option<&OsStr>,
    data_dir: Option<&Path>,
    is_file: &dyn Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    let onedir = data_dir.map(|base| {
        base.join("NemoMusic")
            .join("bin")
            .join(ONEDIR_DIR)
            .join(name)
    });
    let on_path = path_var
        .into_iter()
        .flat_map(std::env::split_paths)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(name));
    let known = data_dir.into_iter().flat_map(|base| {
        let winget = |leaf: &Path| base.join("Microsoft").join("WinGet").join(leaf).join(name);
        let app_bin = base.join("NemoMusic").join("bin").join(name);
        if cfg!(windows) {
            vec![
                winget(Path::new("Links")),
                app_bin,
                // winget installs the portable package here; the `Links` shim (and `PATH`)
                // only appear after a new login, so a fresh install is found through it.
                winget(&Path::new("Packages").join(WINGET_PACKAGE_DIR)),
            ]
        } else {
            vec![app_bin]
        }
    });
    onedir
        .into_iter()
        .chain(on_path)
        .chain(known)
        .find(|candidate| is_file(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn locate_on_linux_skips_winget() {
        let data = PathBuf::from("DATA");
        let bin = data.join("NemoMusic").join("bin");
        let onedir = bin.join(ONEDIR_DIR).join("yt-dlp");
        let on_path = PathBuf::from("bin1").join("yt-dlp");
        let app_bin = bin.join("yt-dlp");
        let winget = data
            .join("Microsoft")
            .join("WinGet")
            .join("Links")
            .join("yt-dlp");
        let path_var = std::env::join_paths(["bin0", "bin1"]).expect("joins");
        let find = |want: &[&PathBuf]| {
            let want: Vec<PathBuf> = want.iter().map(|p| (*p).clone()).collect();
            locate_in(Some(&path_var), Some(&data), &move |p: &Path| {
                want.iter().any(|w| w == p)
            })
        };
        assert_eq!(find(&[&onedir, &on_path, &app_bin]), Some(onedir.clone()));
        assert_eq!(find(&[&on_path, &app_bin]), Some(on_path.clone()));
        assert_eq!(find(&[&app_bin]), Some(app_bin.clone()));
        assert_eq!(find(&[&winget]), None);
    }

    #[cfg(windows)]
    #[test]
    fn locate_prefers_path_then_winget_then_app_bin() {
        let name = "yt-dlp.exe";
        let local = PathBuf::from("LOCAL");
        let on_path = PathBuf::from("bin1").join(name);
        let winget = local
            .join("Microsoft")
            .join("WinGet")
            .join("Links")
            .join(name);
        let nemo = local.join("NemoMusic").join("bin").join(name);
        let path_var = std::env::join_paths(["bin0", "bin1"]).expect("joins");

        let all = |p: &Path| p == on_path || p == winget || p == nemo;
        let onedir = local
            .join("NemoMusic")
            .join("bin")
            .join(ONEDIR_DIR)
            .join(name);
        let with_onedir = |p: &Path| all(p) || p == onedir;
        assert_eq!(
            locate_in(Some(&path_var), Some(&local), &with_onedir),
            Some(onedir.clone())
        );
        assert_eq!(
            locate_in(Some(&path_var), Some(&local), &all),
            Some(on_path.clone())
        );
        let no_path = |p: &Path| p == winget || p == nemo;
        assert_eq!(
            locate_in(Some(&path_var), Some(&local), &no_path),
            Some(winget.clone())
        );
        let only_nemo = |p: &Path| p == nemo;
        assert_eq!(
            locate_in(Some(&path_var), Some(&local), &only_nemo),
            Some(nemo.clone())
        );
        let package = local
            .join("Microsoft")
            .join("WinGet")
            .join("Packages")
            .join(WINGET_PACKAGE_DIR)
            .join(name);
        let only_package = |p: &Path| p == package;
        assert_eq!(
            locate_in(Some(&path_var), Some(&local), &only_package),
            Some(package.clone())
        );
        assert_eq!(locate_in(Some(&path_var), None, &only_nemo), None);
        assert_eq!(locate_in(None, None, &all), None);
        assert_eq!(
            locate_in(Some(&path_var), Some(&local), &|_: &Path| false),
            None
        );
    }
}
