//! Unit tests for the `yt-dlp` resolver and its arguments.

use super::*;

#[test]
fn fast_args_keep_ignore_config_and_add_one_client() {
    let a = fast_args(&vid("abcdefghijk"));
    let text: Vec<String> = a.iter().map(|s| s.to_string_lossy().into_owned()).collect();
    assert_eq!(text[0], "--ignore-config");
    let at = text
        .iter()
        .position(|s| s == "--extractor-args")
        .expect("has extractor args");
    assert_eq!(text[at + 1], FAST_EXTRACTOR_ARGS);
    assert!(
        !text
            .iter()
            .any(|s| s.to_ascii_lowercase().contains("cookie"))
    );
    let sep = text.iter().position(|s| s == "--").expect("has --");
    assert_eq!(sep, text.len() - 2);
    assert!(
        !args(&vid("abcdefghijk"))
            .iter()
            .any(|s| s == "--extractor-args")
    );
}

#[test]
fn a_failed_open_sends_the_video_to_the_full_extraction() {
    let resolver = YtDlpResolver::with_program("x");
    let v = vid("abcdefghijk");
    assert!(resolver.job_for(&v).fast);
    resolver.stream_failed(&v);
    assert!(!resolver.job_for(&v).fast);
    assert!(resolver.job_for(&vid("bbbbbbbbbbb")).fast);
    assert!(
        !YtDlpResolver::with_program("x")
            .with_fast_args(false)
            .job_for(&v)
            .fast
    );
}

fn vid(s: &str) -> VideoId {
    VideoId::parse(s).expect("test id is valid")
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime builds")
}

#[test]
fn args_are_a_list_with_url_after_double_dash() {
    let a = args(&vid("abcdefghijk"));
    let text: Vec<String> = a.iter().map(|s| s.to_string_lossy().into_owned()).collect();
    assert!(text.contains(&"--dump-single-json".to_owned()));
    assert!(text.contains(&"--no-playlist".to_owned()));
    assert!(text.contains(&"--no-warnings".to_owned()));
    assert!(text.contains(&"bestaudio".to_owned()));
    assert!(!text.iter().any(|s| s.contains("cookie")));
    let sep = text.iter().position(|s| s == "--").expect("has --");
    assert_eq!(sep, text.len() - 2);
    assert_eq!(
        text[sep + 1],
        "https://music.youtube.com/watch?v=abcdefghijk"
    );
}

#[test]
fn id_starting_with_dash_cannot_be_an_option() {
    let a = args(&vid("-abcdefghij"));
    let last = a.last().expect("non-empty").to_string_lossy().into_owned();
    assert!(last.starts_with("https://"));
    assert!(a.iter().all(|s| s.to_string_lossy() != "-abcdefghij"));
    // Everything after `--` is the single URL, never parsed as an option.
    let sep = a.iter().position(|s| s == "--").expect("has --");
    assert_eq!(a.len() - sep - 1, 1);
}

#[test]
fn missing_program_gives_resolver_missing() {
    let resolver = YtDlpResolver::with_program("nemo-no-such-program-xyz");
    let id = vid("abcdefghijk");
    let result = runtime().block_on(resolver.resolve(&id));
    assert_eq!(result.unwrap_err(), StreamError::ResolverMissing);
}

/// Needs the real `yt-dlp` program and the network. Uses a made-up id, so the expected
/// result is an error, not a stream.
#[test]
#[ignore = "needs yt-dlp installed and network access"]
fn real_yt_dlp_runs() {
    let resolver = YtDlpResolver::new();
    let id = vid("abcdefghijk");
    let result = runtime().block_on(resolver.resolve(&id));
    assert!(result.is_err());
}

#[test]
fn builder_keeps_the_allowed_codecs() {
    let resolver = YtDlpResolver::with_program("x").with_allowed_codecs(&[Codec::Opus]);
    assert_eq!(resolver.job().allowed, Some(vec![Codec::Opus]));
    assert_eq!(YtDlpResolver::new().job().allowed, None);
}

/// A resolve that loses a race is dropped; its `yt-dlp` process must not run on.
#[cfg(target_os = "linux")]
#[test]
fn a_dropped_resolve_kills_its_yt_dlp_process() {
    use std::os::unix::fs::PermissionsExt;

    let dir = std::env::temp_dir().join(format!("nemo-ytdlp-drop-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let (script, pid_file) = (dir.join("fake-yt-dlp"), dir.join("pid"));
    let text = format!(
        "#!/bin/sh\necho $$ > '{}'\nexec sleep 30\n",
        pid_file.display()
    );
    std::fs::write(&script, text).expect("script");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("runtime");
    let resolver = YtDlpResolver::with_program(&script).with_handle(rt.handle().clone());
    let v = vid("abcdefghijk");
    let outcome = rt.block_on(async {
        tokio::time::timeout(Duration::from_millis(500), resolver.resolve(&v)).await
    });
    assert!(outcome.is_err(), "the fake program was still running");
    let pid = std::fs::read_to_string(&pid_file).expect("the script ran");
    let stat = std::path::PathBuf::from("/proc")
        .join(pid.trim())
        .join("stat");
    // Gone, or killed and waiting to be reaped (state Z).
    let ended = || {
        std::fs::read_to_string(&stat).map_or(true, |text| {
            text.rsplit(')')
                .next()
                .is_some_and(|rest| rest.trim_start().starts_with('Z'))
        })
    };
    for _ in 0..100 {
        if ended() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(ended(), "the process was killed with the dropped resolve");
    let _ = std::fs::remove_dir_all(&dir);
}
