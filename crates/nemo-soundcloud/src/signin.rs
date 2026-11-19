//! The sign-in window of SoundCloud (soundcloud-full criteria 7 and 8, ADR 0005).
//!
//! The window is the app executable started again as
//! `<program> --signin --site soundcloud --data-dir <folder>`. It shows SoundCloud's own sign-in
//! page (the app never sees the password), and prints one line `{"token":"…"}` on stdout when the
//! page has the `oauth_token` cookie. This module starts it, reads that line and nothing else,
//! and turns its exit code into a plain error. The token only travels in memory, as a [`Secret`].

use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::Duration;

use nemo_core::BoxFuture;
use nemo_core::secret::Secret;
use nemo_core::source_account::SourceAccountError;
use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

/// How long the sign-in window may stay open.
pub const SIGN_IN_TIMEOUT: Duration = Duration::from_mins(10);
/// What the user is told when the helper reports that its webview cannot start (exit code 3).
#[cfg(windows)]
pub const WEBVIEW_MISSING: &str = "the WebView2 runtime is missing";
/// What the user is told when the helper reports that its webview cannot start (exit code 3).
#[cfg(not(windows))]
pub const WEBVIEW_MISSING: &str = "the WebKitGTK webview is not available";

/// Most bytes read from the helper's stdout.
const MAX_OUTPUT: u64 = 64 * 1024;
/// Longest token accepted, in bytes. Real ones are about 40 characters.
pub const MAX_TOKEN_BYTES: usize = 512;
/// Tries to start the helper while its file is busy (Linux `ETXTBSY` right after a write).
const START_TRIES: u32 = 5;

/// Where a sign-in token comes from. The real one is the sign-in window; tests use a fake.
pub trait TokenSource: Send + Sync {
    /// Wait for the user to sign in and hand back the token.
    fn token(&self) -> BoxFuture<'_, Result<Secret<String>, SourceAccountError>>;
}

/// Whether `token` looks like a SoundCloud token: printable characters of the sets such tokens
/// use, nothing that could break a header.
#[must_use]
pub fn is_token_shaped(token: &str) -> bool {
    (8..=MAX_TOKEN_BYTES).contains(&token.len())
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.~+/=%".contains(&b))
}

/// The token in a value the user pasted or the window printed. Accepts the bare value, the
/// `oauth_token=<value>` pair as the browser's cookie list shows it, and a copy with spaces,
/// quotes or an `OAuth ` prefix around it. `None` when the rest is not shaped like a token.
#[must_use]
pub fn normalize_token(raw: &str) -> Option<String> {
    let mut text = raw.trim();
    text = text.strip_prefix("OAuth ").unwrap_or(text).trim();
    text = text.strip_prefix("oauth_token=").unwrap_or(text);
    text = text.trim_matches(|c| c == '"' || c == '\'').trim();
    is_token_shaped(text).then(|| text.to_owned())
}

/// The token in the helper's one output line.
fn parse_output(output: &str) -> Option<Secret<String>> {
    let value: Value = serde_json::from_str(output.lines().next()?.trim()).ok()?;
    let token = value.get("token")?.as_str()?;
    is_token_shaped(token).then(|| Secret::new(token.to_owned()))
}

/// The sign-in window as a child process.
#[derive(Debug, Clone)]
pub struct SignInWindow {
    /// The program that shows the window: the app executable itself.
    pub program: PathBuf,
    /// Arguments before `--site soundcloud --data-dir <folder>`: the app passes `--signin`.
    pub args: Vec<OsString>,
    /// The window's own webview profile folder. Sign-out deletes it.
    pub data_dir: PathBuf,
    /// How long the window may stay open.
    pub timeout: Duration,
}

impl SignInWindow {
    /// A window with the default time limit.
    #[must_use]
    pub fn new(program: PathBuf, args: Vec<OsString>, data_dir: PathBuf) -> Self {
        Self {
            program,
            args,
            data_dir,
            timeout: SIGN_IN_TIMEOUT,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .args(["--site", "soundcloud", "--data-dir"])
            .arg(&self.data_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        command
    }

    async fn run(&self) -> Result<Secret<String>, SourceAccountError> {
        let cannot_start =
            || SourceAccountError::Unavailable("the sign-in helper cannot be started".to_owned());
        let mut command = self.command();
        let mut child = start(&mut command).await.map_err(|_| cannot_start())?;
        let Some(stdout) = child.stdout.take() else {
            return Err(SourceAccountError::Unavailable(
                "no output from the sign-in helper".to_owned(),
            ));
        };
        let work = async {
            let mut text = String::new();
            let read = stdout.take(MAX_OUTPUT).read_to_string(&mut text).await;
            let status = child.wait().await;
            (read, status, text)
        };
        let Ok((read, status, text)) = tokio::time::timeout(self.timeout, work).await else {
            // The child is killed when it is dropped here.
            return Err(SourceAccountError::Cancelled);
        };
        let status = status
            .map_err(|_| SourceAccountError::Unavailable("the sign-in helper failed".to_owned()))?;
        match status.code() {
            Some(0) if read.is_ok() => parse_output(&text).ok_or_else(|| {
                SourceAccountError::Unavailable("the sign-in helper gave no session".to_owned())
            }),
            Some(2) => Err(SourceAccountError::Cancelled),
            Some(3) => Err(SourceAccountError::Unavailable(WEBVIEW_MISSING.to_owned())),
            _ => Err(SourceAccountError::Unavailable(
                "the sign-in window failed".to_owned(),
            )),
        }
    }
}

/// Start `command`, trying again a few times only while the program file is busy.
async fn start(command: &mut Command) -> std::io::Result<tokio::process::Child> {
    let mut tries = 1;
    loop {
        match command.spawn() {
            Err(e) if e.kind() == ErrorKind::ExecutableFileBusy && tries < START_TRIES => {
                tries += 1;
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            other => return other,
        }
    }
}

impl TokenSource for SignInWindow {
    fn token(&self) -> BoxFuture<'_, Result<Secret<String>, SourceAccountError>> {
        Box::pin(self.run())
    }
}

#[cfg(test)]
mod tests;
