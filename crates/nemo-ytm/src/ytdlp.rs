//! Stream resolution through the `yt-dlp` subprocess (ADR 0003, tech.md).
//!
//! Resolution is anonymous: no cookies and no config files reach `yt-dlp` (ADR 0001).
//! Option names were checked against the official yt-dlp README on 2026-11-10:
//! `-J/--dump-single-json`, `--no-playlist`, `--no-warnings`, `-f/--format`,
//! `--ignore-config`, and format fields `acodec`, `vcodec`, `abr`, `asr`,
//! `audio_channels`, `url`, `format_id`. The `expire` URL parameter is a YouTube convention
//! that the README does not document; it is optional here.

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::future::Future;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use nemo_core::BoxFuture;
use nemo_core::audio::Codec;
use nemo_core::stream::{ResolvedStream, StreamError, StreamRequest, StreamResolver};
use nemo_core::track::VideoId;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::runtime::Handle;

pub use crate::ytdlp_install::{InstallError, install_onedir};

mod locate;
mod parse;
mod tracks;

pub use locate::{ONEDIR_DIR, locate_yt_dlp};
pub(crate) use parse::expiry;
use parse::{classify_stderr, parse_err};
pub use parse::{parse, parse_for, parse_with};

/// How long one `yt-dlp` run may take.
pub const RESOLVE_TIMEOUT: Duration = Duration::from_secs(20);
/// Windows process flag that starts a console program without a console window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Largest stdout accepted from `yt-dlp`, in bytes.
pub const MAX_OUTPUT_BYTES: u64 = 8 * 1024 * 1024;
/// Largest part of stderr that is read for error mapping, in bytes. The text is never logged.
pub const MAX_STDERR_BYTES: u64 = 64 * 1024;

/// Build the `yt-dlp` argument list for one video. Never a shell string.
///
/// The URL comes after `--`, so nothing after it can be read as an option. The id is already
/// validated by [`VideoId`], and the URL starts with `https://`, so a leading `-` in the id
/// cannot start an option either.
#[must_use]
pub fn args(video: &VideoId) -> Vec<OsString> {
    build_args(video, None)
}

/// Extractor arguments of the fast first try: one player client, no watch page, no player
/// configs. The owner's plan named `android_vr`, but yt-dlp 2026.10.26 drops every
/// `android_vr` format without a PO token ("Requested format is not available", measured
/// 2026-11-11), so the fast try uses `visionos`, one of yt-dlp's default clients
/// (3.7 s with the onedir build, against 10-13 s for the full extraction).
pub const FAST_EXTRACTOR_ARGS: &str = "youtube:player_client=visionos;player_skip=webpage,configs";

/// Like [`args`], with [`FAST_EXTRACTOR_ARGS`]. Still `--ignore-config` and no cookies.
#[must_use]
pub fn fast_args(video: &VideoId) -> Vec<OsString> {
    build_args(video, Some(FAST_EXTRACTOR_ARGS))
}

fn build_args(video: &VideoId, extractor_args: Option<&str>) -> Vec<OsString> {
    let mut list: Vec<OsString> = [
        "--ignore-config",
        "--dump-single-json",
        "--no-playlist",
        "--no-warnings",
        "--format",
        "bestaudio",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    if let Some(extra) = extractor_args {
        list.push(OsString::from("--extractor-args"));
        list.push(OsString::from(extra));
    }
    list.push(OsString::from("--"));
    list.push(OsString::from(format!(
        "https://music.youtube.com/watch?v={}",
        video.as_str()
    )));
    list
}

/// Resolves streams by running `yt-dlp` as a subprocess, without a shell.
///
/// Each resolve first runs with [`fast_args`]; when that fails (or a stream it gave could not
/// be opened, see [`StreamResolver::stream_failed`]) the full extraction ([`args`]) is used.
#[derive(Debug, Clone)]
pub struct YtDlpResolver {
    program: PathBuf,
    allowed: Option<Vec<Codec>>,
    handle: Option<Handle>,
    fast: bool,
    /// Videos whose fast-path stream could not be opened: full extraction only.
    slow_only: Arc<Mutex<HashSet<String>>>,
}

/// Everything one resolve run needs, owned, so it can run as a spawned task.
#[derive(Debug, Clone)]
struct Job {
    program: PathBuf,
    allowed: Option<Vec<Codec>>,
    fast: bool,
    /// The preferred audio language (BCP 47); `None` plays the original track.
    language: Option<String>,
}

impl YtDlpResolver {
    /// Use the program named `yt-dlp`, found through `PATH`.
    #[must_use]
    pub fn new() -> Self {
        Self::with_program("yt-dlp")
    }

    /// Use a specific executable, for example a bundled copy or the result of [`locate_yt_dlp`].
    #[must_use]
    pub fn with_program(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            allowed: None,
            handle: None,
            fast: true,
            slow_only: Arc::default(),
        }
    }

    /// Try [`fast_args`] first (the default) or go straight to the full extraction.
    #[must_use]
    pub fn with_fast_args(mut self, fast: bool) -> Self {
        self.fast = fast;
        self
    }

    /// Only pick formats whose codec the audio engine can decode (default: every codec).
    #[must_use]
    pub fn with_allowed_codecs(mut self, codecs: &[Codec]) -> Self {
        self.allowed = Some(codecs.to_vec());
        self
    }

    /// Run the subprocess on this runtime. The future returned by `resolve` can then be polled
    /// by any executor. Without a handle the future needs a tokio runtime on the polling thread.
    #[must_use]
    pub fn with_handle(mut self, handle: Handle) -> Self {
        self.handle = Some(handle);
        self
    }

    /// The configured executable path.
    #[must_use]
    pub fn program(&self) -> &OsStr {
        self.program.as_os_str()
    }

    fn job(&self) -> Job {
        Job {
            program: self.program.clone(),
            allowed: self.allowed.clone(),
            fast: self.fast,
            language: None,
        }
    }

    fn job_for(&self, video: &VideoId) -> Job {
        let mut job = self.job();
        job.fast = job.fast
            && !self
                .slow_only
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contains(video.as_str());
        job
    }
}

/// Read at most [`MAX_STDERR_BYTES`] of `stderr`, then drain the rest so the child never blocks.
async fn read_stderr(mut stderr: tokio::process::ChildStderr) -> String {
    let mut head = Vec::new();
    let _ = (&mut stderr)
        .take(MAX_STDERR_BYTES)
        .read_to_end(&mut head)
        .await;
    let _ = tokio::io::copy(&mut stderr, &mut tokio::io::sink()).await;
    String::from_utf8_lossy(&head).into_owned()
}

impl Job {
    /// Fast arguments first, the full extraction when they fail.
    async fn run(self, video: VideoId) -> Result<ResolvedStream, StreamError> {
        if self.fast {
            match Self::timed(self.run_with(fast_args(&video))).await {
                Err(StreamError::ResolverMissing) => return Err(StreamError::ResolverMissing),
                Ok(stream) => return Ok(stream),
                Err(e) => tracing::debug!(error = %e, "fast yt-dlp run failed, retrying"),
            }
        }
        Self::timed(self.run_with(args(&video))).await
    }

    async fn timed(
        run: impl Future<Output = Result<ResolvedStream, StreamError>>,
    ) -> Result<ResolvedStream, StreamError> {
        tokio::time::timeout(RESOLVE_TIMEOUT, run)
            .await
            .unwrap_or_else(|_| Err(StreamError::Network("resolver timed out".to_owned())))
    }

    async fn run_with(&self, arguments: Vec<OsString>) -> Result<ResolvedStream, StreamError> {
        let mut command = Command::new(&self.program);
        command
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        // The app has no console, so Windows would open a new console window for every
        // yt-dlp run. CREATE_NO_WINDOW keeps it hidden.
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);
        let mut child = command.spawn().map_err(|e| match e.kind() {
            ErrorKind::NotFound => StreamError::ResolverMissing,
            _ => StreamError::Network("could not start the resolver".to_owned()),
        })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| parse_err("no resolver output"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| parse_err("no resolver output"))?;
        let read_stdout = async {
            let mut out = Vec::new();
            // Read one byte past the cap so an oversized answer is detected, not truncated silently.
            stdout
                .take(MAX_OUTPUT_BYTES + 1)
                .read_to_end(&mut out)
                .await
                .map(|_| out)
        };
        let (out, err_text) = tokio::join!(read_stdout, read_stderr(stderr));
        let out =
            out.map_err(|_| StreamError::Network("could not read resolver output".to_owned()))?;
        let status = child
            .wait()
            .await
            .map_err(|_| StreamError::Network("could not wait for the resolver".to_owned()))?;
        if !status.success() {
            // The stderr text is only classified, never kept or logged.
            return Err(classify_stderr(&err_text));
        }
        if out.len() as u64 > MAX_OUTPUT_BYTES {
            return Err(parse_err("resolver output too large"));
        }
        let text = std::str::from_utf8(&out).map_err(|_| parse_err("output is not UTF-8"))?;
        parse_for(text, self.allowed.as_deref(), self.language.as_deref())
    }
}

impl Default for YtDlpResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl YtDlpResolver {
    /// Run one resolve with `language` (`None`: the original track).
    fn start(
        &self,
        video: &VideoId,
        language: Option<String>,
    ) -> BoxFuture<'static, Result<ResolvedStream, StreamError>> {
        let mut job = self.job_for(video);
        job.language = language;
        let timed = job.run(video.clone());
        match &self.handle {
            // Spawned on the app runtime; the returned future only awaits the join handle.
            Some(handle) => {
                let task = handle.spawn(timed);
                Box::pin(async move {
                    task.await.unwrap_or_else(|_| {
                        Err(StreamError::Network("resolve was cancelled".to_owned()))
                    })
                })
            }
            None => Box::pin(timed),
        }
    }
}

impl StreamResolver for YtDlpResolver {
    fn resolve<'a>(
        &'a self,
        video: &'a VideoId,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        self.start(video, None)
    }

    /// Resolve with the request's audio language: the matching track when the video has one,
    /// otherwise the original track. Premium and the codec hint are not used here yet.
    fn resolve_with<'a>(
        &'a self,
        video: &'a VideoId,
        request: &'a StreamRequest,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        self.start(video, request.audio_language.clone())
    }

    fn stream_failed(&self, video: &VideoId) {
        if self.fast {
            let mut set = self
                .slow_only
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if set.len() >= 256 {
                set.clear();
            }
            set.insert(video.as_str().to_owned());
        }
    }
}

#[cfg(test)]
mod tests;
