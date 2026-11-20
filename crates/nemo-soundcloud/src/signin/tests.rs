//! The sign-in window runner against fake helper programs (small scripts that print one line and
//! exit), and the token rules. No real token appears anywhere.

use std::path::Path;

use super::*;

const TOKEN: &str = "2-293232-1234567-SyntheticToken";

/// A fake helper that records its arguments in `args.txt`, prints `stdout` (one line), writes
/// `stderr` (one line) and exits with `code`.
fn script(stdout: Option<&str>, stderr: Option<&str>, code: i32, sleep: bool) -> String {
    if cfg!(windows) {
        let echo = stdout
            .map(|line| format!("@echo {line}\r\n"))
            .unwrap_or_default();
        let err = stderr
            .map(|line| format!("@echo {line} 1>&2\r\n"))
            .unwrap_or_default();
        let wait = if sleep {
            "@ping -n 30 127.0.0.1 >nul\r\n"
        } else {
            ""
        };
        format!("@echo %* > \"%~dp0args.txt\"\r\n{wait}{err}{echo}@exit /b {code}\r\n")
    } else {
        let echo = stdout
            .map(|line| format!("printf '%s\\n' '{line}'\n"))
            .unwrap_or_default();
        let err = stderr
            .map(|line| format!("printf '%s\\n' '{line}' >&2\n"))
            .unwrap_or_default();
        let wait = if sleep { "sleep 30\n" } else { "" };
        format!(
            "#!/bin/sh\necho \"$@\" > \"$(dirname \"$0\")/args.txt\"\n{wait}{err}{echo}exit {code}\n"
        )
    }
}

struct Helper {
    dir: PathBuf,
    window: SignInWindow,
}

impl Drop for Helper {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn helper(name: &str, stdout: Option<&str>, code: i32, sleep: bool) -> Helper {
    helper_with(name, &script(stdout, None, code, sleep))
}

/// A helper that runs `body` (a complete script file).
fn helper_with(name: &str, body: &str) -> Helper {
    let dir = std::env::temp_dir().join(format!("nemo-sc-signin-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let program = dir.join(if cfg!(windows) {
        "helper.cmd"
    } else {
        "helper.sh"
    });
    std::fs::write(&program, body).expect("script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    let window = SignInWindow::new(
        program,
        vec![OsString::from("--signin")],
        dir.join("profile"),
    );
    Helper { dir, window }
}

fn line(token: &str) -> String {
    format!("{{\"token\":\"{token}\"}}")
}

#[tokio::test]
async fn the_window_hands_back_only_the_token() {
    let h = helper("ok", Some(&line(TOKEN)), 0, false);
    let token = h.window.token().await.expect("token");
    assert_eq!(token.expose(), TOKEN);
    assert_eq!(format!("{token:?}"), "[redacted]");
}

#[tokio::test]
async fn the_window_is_started_for_soundcloud_with_its_own_profile_folder() {
    let h = helper("args", Some(&line(TOKEN)), 0, false);
    h.window.token().await.expect("token");
    let args = std::fs::read_to_string(h.dir.join("args.txt")).expect("args");
    let args: Vec<&str> = args.split_whitespace().collect();
    assert_eq!(&args[..3], ["--signin", "--site", "soundcloud"]);
    assert_eq!(args[3], "--data-dir");
    // A Windows batch file gets its arguments quoted.
    assert!(Path::new(args[4].trim_matches('"')).ends_with("profile"));
    assert!(!args.iter().any(|a| a.contains(TOKEN)));
}

#[tokio::test]
async fn exit_codes_become_plain_errors() {
    let closed = helper("closed", None, 2, false);
    assert_eq!(
        closed.window.token().await,
        Err(SourceAccountError::Cancelled)
    );
    let missing = helper("missing", None, 3, false);
    assert_eq!(
        missing.window.token().await,
        Err(SourceAccountError::Unavailable(WEBVIEW_MISSING.to_owned()))
    );
    let failed = helper("failed", None, 64, false);
    assert!(matches!(
        failed.window.token().await,
        Err(SourceAccountError::Unavailable(_))
    ));
}

#[tokio::test]
async fn a_line_that_is_not_a_token_gives_no_session_and_never_echoes_it() {
    for (name, out) in [
        ("empty", None),
        ("junk", Some("not json".to_owned())),
        ("cookie", Some("{\"cookie\":\"a=b\"}".to_owned())),
        ("short", Some(line("abc"))),
        ("spaces", Some(line("has spaces in the token"))),
    ] {
        let h = helper(name, out.as_deref(), 0, false);
        let err = h.window.token().await.expect_err(name);
        assert!(matches!(err, SourceAccountError::Unavailable(_)), "{name}");
        let text = err.to_string();
        assert!(
            !text.contains("spaces in") && !text.contains("a=b"),
            "{text}"
        );
    }
}

#[tokio::test]
async fn a_window_left_open_too_long_is_cancelled() {
    let mut h = helper("slow", Some(&line(TOKEN)), 0, true);
    h.window.timeout = Duration::from_millis(300);
    assert_eq!(h.window.token().await, Err(SourceAccountError::Cancelled));
}

#[tokio::test]
async fn a_missing_program_is_unavailable_with_a_fixed_text() {
    let window = SignInWindow::new(
        PathBuf::from("nemo-no-such-program-xyz"),
        Vec::new(),
        PathBuf::from("profile"),
    );
    let err = window.token().await.expect_err("no program");
    assert_eq!(
        err,
        SourceAccountError::Unavailable("the sign-in helper cannot be started".to_owned())
    );
}

#[test]
fn pasted_values_are_cleaned_and_checked() {
    for raw in [
        TOKEN.to_owned(),
        format!("  {TOKEN}\n"),
        format!("oauth_token={TOKEN}"),
        format!("OAuth {TOKEN}"),
        format!("\"{TOKEN}\""),
        format!("'{TOKEN}'"),
    ] {
        assert_eq!(normalize_token(&raw).as_deref(), Some(TOKEN), "{raw:?}");
    }
    for raw in [
        "",
        "short",
        "has a space inside it",
        "a;b=c-1234567",
        "line\nbreak-1234567",
        "ключ-1234567890",
    ] {
        assert_eq!(normalize_token(raw), None, "{raw:?}");
    }
    assert_eq!(normalize_token(&"a".repeat(MAX_TOKEN_BYTES + 1)), None);
    assert!(normalize_token(&"a".repeat(MAX_TOKEN_BYTES)).is_some());
}

#[test]
fn escaped_values_are_decoded_when_the_result_is_a_token() {
    // The page may store the cookie value percent-encoded: `2-293232-1234567-Abc%2Bdef`.
    assert_eq!(
        normalize_token("2-293232-1234567-Abc%2Bdef").as_deref(),
        Some("2-293232-1234567-Abc+def")
    );
    assert_eq!(
        normalize_token("\"2-293232-1234567-Abc%2Bdef\"").as_deref(),
        Some("2-293232-1234567-Abc+def")
    );
    // A bad escape keeps the raw text (a percent sign is an allowed character); a decoded space or
    // semicolon is not a token, so the raw form (which is shaped) stays.
    assert_eq!(
        normalize_token("2-293232-1234567-100%").as_deref(),
        Some("2-293232-1234567-100%")
    );
    assert_eq!(
        normalize_token("2-293232-1234567-a%20b").as_deref(),
        Some("2-293232-1234567-a%20b")
    );
    assert_eq!(
        normalize_token("2-293232-1234567-a%3Bb").as_deref(),
        Some("2-293232-1234567-a%3Bb")
    );
    // A decoded value over the limit is not taken either, only the raw one if it is short enough.
    let long = "%41".repeat(MAX_TOKEN_BYTES / 2);
    assert_eq!(
        normalize_token(&long).as_deref(),
        Some("A".repeat(MAX_TOKEN_BYTES / 2).as_str())
    );
}

#[test]
fn a_token_line_and_a_hint_line_are_told_apart() {
    assert!(matches!(parse_line(&line(TOKEN)), Line::Token(t) if t.expose() == TOKEN));
    assert!(matches!(
        parse_line("{\"hint\":\"no_token\"}\n"),
        Line::NoToken
    ));
    assert!(matches!(parse_line("{\"hint\":\"other\"}"), Line::Other));
    assert!(matches!(parse_line(&line("short")), Line::Other));
    assert!(matches!(parse_line("{\"token\":5}"), Line::Other));
    assert!(matches!(parse_line(""), Line::Other));
    // A long token that real pages can have is taken up to the limit.
    let long = "a".repeat(MAX_TOKEN_BYTES);
    assert!(matches!(parse_line(&line(&long)), Line::Token(_)));
    assert!(matches!(
        parse_line(&line(&format!("{long}a"))),
        Line::Other
    ));
}

#[tokio::test]
async fn a_hint_before_the_token_does_not_stop_the_wait() {
    let body = if cfg!(windows) {
        script(Some(&line(TOKEN)), None, 0, false)
    } else {
        format!(
            "#!/bin/sh\nprintf '%s\\n' '{{\"hint\":\"no_token\"}}'\nprintf '%s\\n' '{}'\nexit 0\n",
            line(TOKEN)
        )
    };
    let h = helper_with("hint-then-token", &body);
    let token = h.window.token().await.expect("token");
    assert_eq!(token.expose(), TOKEN);
    // The session is over: no hint and no recheck remain.
    assert!(!h.window.hint());
    h.window.recheck();
}

/// A window that says it found nothing and then waits for the app's `check` line.
#[cfg(unix)]
#[tokio::test]
async fn the_hint_is_shown_and_recheck_reaches_the_window() {
    let body = format!(
        "#!/bin/sh\nprintf '%s\\n' '{{\"hint\":\"no_token\"}}'\nread answer\nif [ \"$answer\" = check ]; then printf '%s\\n' '{}'; exit 0; fi\nexit 64\n",
        line(TOKEN)
    );
    let h = helper_with("recheck", &body);
    let window = h.window.clone();
    assert!(!window.hint());
    let waiting = tokio::spawn({
        let window = window.clone();
        async move { window.token().await }
    });
    let mut waited = 0;
    while !window.hint() {
        assert!(waited < 100, "the hint never arrived");
        waited += 1;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    window.recheck();
    let token = waiting.await.expect("task").expect("token");
    assert_eq!(token.expose(), TOKEN);
    assert!(!window.hint());
}

#[cfg(unix)]
#[tokio::test]
async fn a_dropped_wait_stops_the_window_and_clears_the_hint() {
    let body = "#!/bin/sh\nprintf '%s\\n' '{\"hint\":\"no_token\"}'\nsleep 30\n";
    let h = helper_with("dropped", body);
    let window = h.window.clone();
    let waiting = tokio::spawn({
        let window = window.clone();
        async move { window.token().await }
    });
    let mut waited = 0;
    while !window.hint() {
        assert!(waited < 100, "the hint never arrived");
        waited += 1;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    waiting.abort();
    let _ = waiting.await;
    assert!(!window.hint());
    window.recheck();
}

#[tokio::test]
async fn diagnostic_lines_are_copied_one_by_one_and_cut() {
    let long = format!("signin {}", "x".repeat(MAX_DEBUG_LINE * 2));
    let input = format!("signin cookie name=a len=40\n{long}\nsignin last");
    let mut seen = Vec::new();
    copy_debug(input.as_bytes(), |line| seen.push(line.to_owned())).await;
    assert_eq!(seen.len(), 3);
    assert_eq!(seen[0], "signin cookie name=a len=40");
    assert_eq!(seen[1].len(), MAX_DEBUG_LINE);
    assert_eq!(seen[2], "signin last");

    let many = "signin line\n".repeat(MAX_DEBUG_LINES + 50);
    let mut count = 0;
    copy_debug(many.as_bytes(), |_| count += 1).await;
    assert_eq!(count, MAX_DEBUG_LINES);

    // Cutting never splits a character.
    assert_eq!(clip("aé", 2), "a");
    assert_eq!(clip("abc", 10), "abc");
}

#[tokio::test]
async fn a_window_in_debug_mode_still_hands_back_the_token_and_its_stderr_is_read() {
    let body = script(
        Some(&line(TOKEN)),
        Some("signin cookie name=oauth_token len=40"),
        0,
        false,
    );
    let mut h = helper_with("debug", &body);
    h.window.debug = true;
    let token = h.window.token().await.expect("token");
    assert_eq!(token.expose(), TOKEN);
}
