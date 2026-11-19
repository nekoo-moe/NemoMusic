//! Tests of the warm-up with a fake `yt-dlp` script (Linux) and no network.

use super::*;

#[cfg(target_os = "linux")]
fn script_in(dir: &std::path::Path, body: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    std::fs::create_dir_all(dir).expect("dir");
    let script = dir.join("fake-yt-dlp");
    std::fs::write(&script, format!("#!/bin/sh\n{body}\n")).expect("script");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    script
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("runtime")
}

fn lines_in(file: &std::path::Path) -> usize {
    std::fs::read_to_string(file).map_or(0, |t| t.lines().count())
}

#[cfg(target_os = "linux")]
#[test]
fn the_warm_up_runs_the_program_once_and_asks_for_nothing_else() {
    let dir = std::env::temp_dir().join(format!("nemo-warmup-once-{}", std::process::id()));
    let log = dir.join("runs");
    let script = script_in(&dir, &format!("echo \"$@\" >> '{}'", log.display()));
    let rt = runtime();
    let ytdlp = YtDlpResolver::with_program(&script);
    let warm = WarmUp::start(rt.handle(), ytdlp, HttpStreamOpener::new(), Duration::ZERO);
    for _ in 0..100 {
        if lines_in(&log) > 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(lines_in(&log), 1, "one run");
    let args = std::fs::read_to_string(&log).expect("log");
    assert_eq!(args.trim(), "--ignore-config --version");
    drop(warm);
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(target_os = "linux")]
#[test]
fn dropping_the_warm_up_before_it_starts_runs_nothing() {
    let dir = std::env::temp_dir().join(format!("nemo-warmup-drop-{}", std::process::id()));
    let log = dir.join("runs");
    let script = script_in(&dir, &format!("echo x >> '{}'", log.display()));
    let rt = runtime();
    let warm = WarmUp::start(
        rt.handle(),
        YtDlpResolver::with_program(&script),
        HttpStreamOpener::new(),
        Duration::from_secs(30),
    );
    std::thread::sleep(Duration::from_millis(100));
    drop(warm);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(lines_in(&log), 0, "cancelled before the pause ended");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_program_is_not_an_error() {
    let rt = runtime();
    let ytdlp = YtDlpResolver::with_program("nemo-no-such-yt-dlp-xyz");
    assert!(!rt.block_on(ytdlp.warm_up()));
}
