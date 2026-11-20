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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use nemo_core::BoxFuture;
use nemo_core::secret::Secret;
use nemo_core::source_account::SourceAccountError;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

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
/// Shortest token accepted, in bytes.
pub const MIN_TOKEN_BYTES: usize = 8;
/// Longest token accepted, in bytes. Real ones are about 40 characters.
pub const MAX_TOKEN_BYTES: usize = 2048;
/// The environment variable that makes the sign-in window explain what it sees (names, flags and
/// lengths of cookies, never values) on its stderr, which the app copies to its log.
pub const DEBUG_ENV: &str = "NEMO_SIGNIN_DEBUG";
/// Longest diagnostic line copied to the log, in bytes, and most lines per sign-in.
const MAX_DEBUG_LINE: usize = 300;
const MAX_DEBUG_LINES: usize = 400;
/// Tries to start the helper while its file is busy (Linux `ETXTBSY` right after a write).
const START_TRIES: u32 = 5;

/// Where a sign-in token comes from. The real one is the sign-in window; tests use a fake.
pub trait TokenSource: Send + Sync {
    /// Wait for the user to sign in and hand back the token.
    fn token(&self) -> BoxFuture<'_, Result<Secret<String>, SourceAccountError>>;

    /// The window waited long without finding a token: the app then offers "I am signed in,
    /// check again" and the token paste.
    fn hint(&self) -> bool {
        false
    }

    /// Ask the window to look for the token now.
    fn recheck(&self) {}
}

/// Whether `token` looks like a SoundCloud token: [`MIN_TOKEN_BYTES`] to [`MAX_TOKEN_BYTES`] of
/// visible ASCII that cannot break a header (no space, quote, comma, semicolon, backslash or
/// angle bracket).
#[must_use]
pub fn is_token_shaped(token: &str) -> bool {
    (MIN_TOKEN_BYTES..=MAX_TOKEN_BYTES).contains(&token.len())
        && token
            .bytes()
            .all(|b| b.is_ascii_graphic() && !matches!(b, b'"' | b';' | b',' | b'\\' | b'<' | b'>'))
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// `value` with its `%XX` escapes decoded; `None` when it has none, a bad one, or the result is
/// not text.
fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let (mut out, mut i, mut any) = (Vec::with_capacity(bytes.len()), 0, false);
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let high = bytes.get(i + 1).copied().and_then(hex)?;
            let low = bytes.get(i + 2).copied().and_then(hex)?;
            out.push(high * 16 + low);
            i += 3;
            any = true;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    if any {
        String::from_utf8(out).ok()
    } else {
        None
    }
}

/// The token in a value the user pasted or the window printed. Accepts the bare value, the
/// `oauth_token=<value>` pair as the browser's cookie list shows it, and a copy with spaces,
/// quotes or an `OAuth ` prefix around it; a value with `%XX` escapes is decoded when the
/// decoded form is shaped like a token. `None` when the rest is not shaped like a token.
#[must_use]
pub fn normalize_token(raw: &str) -> Option<String> {
    let mut text = raw.trim();
    text = text.strip_prefix("OAuth ").unwrap_or(text).trim();
    text = text.strip_prefix("oauth_token=").unwrap_or(text);
    text = text.trim_matches(|c| c == '"' || c == '\'').trim();
    if let Some(decoded) = percent_decode(text)
        && is_token_shaped(&decoded)
    {
        return Some(decoded);
    }
    is_token_shaped(text).then(|| text.to_owned())
}

/// What one line of the helper's stdout says.
enum Line {
    /// The token.
    Token(Secret<String>),
    /// The window waited long without a token.
    NoToken,
    /// Anything else.
    Other,
}

/// Read one stdout line of the helper.
fn parse_line(line: &str) -> Line {
    let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
        return Line::Other;
    };
    if let Some(token) = value.get("token").and_then(Value::as_str) {
        return is_token_shaped(token)
            .then(|| Secret::new(token.to_owned()))
            .map_or(Line::Other, Line::Token);
    }
    if value.get("hint").and_then(Value::as_str) == Some("no_token") {
        return Line::NoToken;
    }
    Line::Other
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
    /// Copy the window's diagnostic lines to the log ([`DEBUG_ENV`]).
    debug: bool,
    /// What the running window tells the app, and what the app tells it.
    link: Arc<Link>,
}

/// The two-way link to the running window.
#[derive(Debug, Default)]
struct Link {
    /// The window found no token for a long time.
    hint: AtomicBool,
    /// Sends "check now" lines to the window's stdin.
    recheck: Mutex<Option<UnboundedSender<()>>>,
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
            debug: std::env::var_os(DEBUG_ENV).is_some_and(|v| v == "1"),
            link: Arc::default(),
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .args(["--site", "soundcloud", "--data-dir"])
            .arg(&self.data_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(if self.debug {
                std::process::Stdio::piped()
            } else {
                std::process::Stdio::null()
            })
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
        let session = Session::open(&self.link, &mut child);
        let work = async {
            let (token, read_ok) = read_lines(stdout, &self.link).await;
            let status = child.wait().await;
            (token, read_ok, status)
        };
        let outcome = tokio::time::timeout(self.timeout, work).await;
        drop(session);
        // On a timeout the child is killed when it is dropped here.
        let Ok((token, read_ok, status)) = outcome else {
            return Err(SourceAccountError::Cancelled);
        };
        let status = status
            .map_err(|_| SourceAccountError::Unavailable("the sign-in helper failed".to_owned()))?;
        match status.code() {
            Some(0) if read_ok => token.ok_or_else(|| {
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

/// Read the helper's stdout line by line: the token (the first one) and whether reading went
/// well. A "no token" line raises the hint on `link`.
async fn read_lines(
    stdout: tokio::process::ChildStdout,
    link: &Link,
) -> (Option<Secret<String>>, bool) {
    let mut reader = BufReader::new(stdout.take(MAX_OUTPUT));
    let (mut found, mut text) = (None, String::new());
    loop {
        text.clear();
        match reader.read_line(&mut text).await {
            Ok(0) => return (found, true),
            Ok(_) => match parse_line(&text) {
                Line::Token(token) => found = found.or(Some(token)),
                Line::NoToken => link.hint.store(true, Ordering::Relaxed),
                Line::Other => {}
            },
            Err(_) => return (found, false),
        }
    }
}

/// What lives next to a running window: the hint, the recheck channel and the two small tasks that
/// feed the window's stdin and copy its stderr to the log. Dropping it ends all of them.
struct Session {
    link: Arc<Link>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Session {
    /// Take the window's stdin and stderr and start the tasks for them.
    fn open(link: &Arc<Link>, child: &mut tokio::process::Child) -> Self {
        link.hint.store(false, Ordering::Relaxed);
        let mut tasks = Vec::new();
        if let Some(stdin) = child.stdin.take() {
            let (tx, rx) = unbounded_channel();
            *link.recheck.lock().unwrap_or_else(PoisonError::into_inner) = Some(tx);
            tasks.push(tokio::spawn(forward_rechecks(stdin, rx)));
        }
        if let Some(stderr) = child.stderr.take() {
            tasks.push(tokio::spawn(copy_debug(stderr, |line| {
                tracing::info!(target: "nemo_signin", "{line}");
            })));
        }
        Self {
            link: Arc::clone(link),
            tasks,
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        *self
            .link
            .recheck
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
        self.link.hint.store(false, Ordering::Relaxed);
        for task in &self.tasks {
            task.abort();
        }
    }
}

/// Send one `check` line to the window for every request, until the window or the app is gone.
async fn forward_rechecks(
    mut stdin: tokio::process::ChildStdin,
    mut requests: tokio::sync::mpsc::UnboundedReceiver<()>,
) {
    while requests.recv().await.is_some() {
        if stdin.write_all(b"check\n").await.is_err() || stdin.flush().await.is_err() {
            return;
        }
    }
}

/// Hand every diagnostic line of the window (names, flags and lengths, never values) to `emit`,
/// cut to [`MAX_DEBUG_LINE`] bytes, up to [`MAX_DEBUG_LINES`] lines. The pipe is read to its end
/// so the window never blocks on it.
async fn copy_debug<R>(stderr: R, mut emit: impl FnMut(&str))
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut reader = BufReader::new(stderr);
    let (mut buf, mut lines) = (Vec::new(), 0_usize);
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf).await {
            Ok(0) | Err(_) => return,
            Ok(_) if lines >= MAX_DEBUG_LINES => {}
            Ok(_) => {
                lines += 1;
                let text = String::from_utf8_lossy(&buf);
                emit(clip(text.trim_end(), MAX_DEBUG_LINE));
            }
        }
    }
}

/// `text` cut to at most `max` bytes at a character boundary.
fn clip(text: &str, max: usize) -> &str {
    let mut end = text.len().min(max);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
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

    fn hint(&self) -> bool {
        self.link.hint.load(Ordering::Relaxed)
    }

    fn recheck(&self) {
        let sender = self
            .link
            .recheck
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(sender) = sender {
            let _ = sender.send(());
        }
    }
}

#[cfg(test)]
mod tests;
