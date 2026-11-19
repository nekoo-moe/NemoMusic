//! The sign-in window runner against fake helper programs (small scripts that print one line and
//! exit), and the token rules. No real token appears anywhere.

use std::path::Path;

use super::*;

const TOKEN: &str = "2-293232-1234567-SyntheticToken";

/// A fake helper that records its arguments in `args.txt`, prints `stdout` (one line) and exits
/// with `code`.
fn script(stdout: Option<&str>, code: i32, sleep: bool) -> String {
    if cfg!(windows) {
        let echo = stdout
            .map(|line| format!("@echo {line}\r\n"))
            .unwrap_or_default();
        let wait = if sleep {
            "@ping -n 30 127.0.0.1 >nul\r\n"
        } else {
            ""
        };
        format!("@echo %* > \"%~dp0args.txt\"\r\n{wait}{echo}@exit /b {code}\r\n")
    } else {
        let echo = stdout
            .map(|line| format!("printf '%s\\n' '{line}'\n"))
            .unwrap_or_default();
        let wait = if sleep { "sleep 30\n" } else { "" };
        format!(
            "#!/bin/sh\necho \"$@\" > \"$(dirname \"$0\")/args.txt\"\n{wait}{echo}exit {code}\n"
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
    let dir = std::env::temp_dir().join(format!("nemo-sc-signin-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let program = dir.join(if cfg!(windows) {
        "helper.cmd"
    } else {
        "helper.sh"
    });
    std::fs::write(&program, script(stdout, code, sleep)).expect("script");
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
