//! Unit tests for the app-managed `yt-dlp` copy. No network.

use super::*;

const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nemo-managed-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

fn at(secs: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(secs)
}

fn managed() -> ManagedYtDlp {
    ManagedYtDlp::new(
        PathBuf::from("DATA").join("bin").join("yt-dlp"),
        Build::LinuxOnefile,
    )
}

fn start(auto_install: bool, user_set: bool, found: Option<&Path>) -> Startup<'_> {
    Startup {
        auto_install,
        user_set,
        found,
    }
}

#[test]
fn nothing_found_means_install() {
    let m = managed();
    assert_eq!(
        plan(&start(true, false, None), &m, None, at(0)),
        Plan::Install
    );
    // Switched off: never download.
    assert_eq!(
        plan(&start(false, false, None), &m, None, at(0)),
        Plan::Leave
    );
}

#[test]
fn a_copy_the_user_installed_is_never_touched() {
    let m = managed();
    let on_path = PathBuf::from("/usr/bin/yt-dlp");
    assert_eq!(
        plan(&start(true, false, Some(&on_path)), &m, None, at(0)),
        Plan::Leave
    );
    // A path set in the settings, even the managed one, is the user's choice.
    let program = m.program();
    assert_eq!(
        plan(&start(true, true, Some(&program)), &m, None, at(0)),
        Plan::Leave
    );
    assert_eq!(plan(&start(true, true, None), &m, None, at(0)), Plan::Leave);
}

#[test]
fn the_managed_copy_is_checked_at_most_once_a_day() {
    let m = managed();
    let program = m.program();
    let s = start(true, false, Some(&program));
    let day = CHECK_INTERVAL.as_secs();
    assert_eq!(plan(&s, &m, None, at(day)), Plan::CheckForUpdate);
    assert_eq!(plan(&s, &m, Some(at(day)), at(day + 60)), Plan::Leave);
    assert_eq!(plan(&s, &m, Some(at(day)), at(2 * day - 1)), Plan::Leave);
    assert_eq!(
        plan(&s, &m, Some(at(day)), at(2 * day)),
        Plan::CheckForUpdate
    );
    // The clock moved back: check rather than wait forever.
    assert_eq!(
        plan(&s, &m, Some(at(3 * day)), at(day)),
        Plan::CheckForUpdate
    );
    // Switched off: no update either.
    let off = start(false, false, Some(&program));
    assert_eq!(plan(&off, &m, None, at(day)), Plan::Leave);
}

#[test]
fn update_only_when_the_published_hash_differs() {
    assert!(!update_needed(Some(HASH_A), HASH_A));
    assert!(!update_needed(Some(&HASH_A.to_ascii_uppercase()), HASH_A));
    assert!(update_needed(Some(HASH_A), HASH_B));
    assert!(update_needed(None, HASH_A));
}

#[test]
fn the_program_path_follows_the_build() {
    let dir = PathBuf::from("DATA").join("bin").join("yt-dlp");
    let linux = ManagedYtDlp::new(dir.clone(), Build::LinuxOnefile);
    assert_eq!(linux.program(), dir.join("yt-dlp"));
    let windows = ManagedYtDlp::new(dir.clone(), Build::WindowsOnedir);
    assert_eq!(windows.program(), dir.join("yt-dlp.exe"));
    assert_eq!(Build::LinuxOnefile.asset(), "yt-dlp_linux");
    assert_eq!(Build::WindowsOnedir.asset(), "yt-dlp_win.zip");
}

#[test]
fn the_platform_build_and_folder_match_the_lookup() {
    let expected = if cfg!(all(windows, target_arch = "x86_64")) {
        Some(Build::WindowsOnedir)
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some(Build::LinuxOnefile)
    } else {
        None
    };
    assert_eq!(Build::for_this_platform(), expected);
    if let (Some(m), Some(dir)) = (
        ManagedYtDlp::for_this_platform(),
        super::super::managed_dir(),
    ) {
        assert_eq!(m.program(), dir.join(m.build.program_name()));
        assert!(dir.ends_with(Path::new("NemoMusic").join("bin").join("yt-dlp")));
    }
}

#[test]
fn hash_and_check_time_round_trip_through_files() {
    let dir = temp_dir("records");
    let m = ManagedYtDlp::new(dir.clone(), Build::LinuxOnefile);
    assert_eq!(m.installed_hash(), None);
    assert_eq!(m.last_check(), None);
    m.record(Some(HASH_A), at(1_000));
    assert_eq!(m.installed_hash().as_deref(), Some(HASH_A));
    assert_eq!(m.last_check(), Some(at(1_000)));
    // A check without an install keeps the hash and moves the time.
    m.record(None, at(2_000));
    assert_eq!(m.installed_hash().as_deref(), Some(HASH_A));
    assert_eq!(m.last_check(), Some(at(2_000)));
    // Garbage in the files reads as missing.
    std::fs::write(dir.join(HASH_FILE), "not a hash").expect("write");
    std::fs::write(dir.join(CHECK_FILE), "soon").expect("write");
    assert_eq!(m.installed_hash(), None);
    assert_eq!(m.last_check(), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_installed_program_switches_every_resolver_clone() {
    let resolver = crate::ytdlp::YtDlpResolver::new();
    let clone = resolver.clone();
    assert_eq!(clone.program(), "yt-dlp");
    let slot = resolver.program_slot();
    assert!(slot.set(PathBuf::from("/data/yt-dlp")));
    assert_eq!(clone.program(), "/data/yt-dlp");
    assert_eq!(resolver.program(), "/data/yt-dlp");
    // Set once: a second install in the same run keeps the first path.
    assert!(!slot.set(PathBuf::from("/other")));
    assert_eq!(clone.program(), "/data/yt-dlp");
}
