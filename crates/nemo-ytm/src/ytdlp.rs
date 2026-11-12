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
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nemo_core::BoxFuture;
use nemo_core::audio::{AudioFormat, Codec, Origin, StreamInfo};
use nemo_core::secret::Secret;
use nemo_core::stream::{ResolvedStream, StreamError, StreamResolver};
use nemo_core::track::VideoId;
use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::runtime::Handle;

pub use crate::ytdlp_install::{InstallError, install_onedir};

/// How long one `yt-dlp` run may take.
pub const RESOLVE_TIMEOUT: Duration = Duration::from_secs(20);
/// Windows process flag that starts a console program without a console window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Largest stdout accepted from `yt-dlp`, in bytes.
pub const MAX_OUTPUT_BYTES: u64 = 8 * 1024 * 1024;
/// Largest part of stderr that is read for error mapping, in bytes. The text is never logged.
pub const MAX_STDERR_BYTES: u64 = 64 * 1024;

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

/// Decide what a failed `yt-dlp` run means from the start of its stderr.
///
/// `NotAvailable`: region, age, private, removed or members-only videos. `Network`: cannot
/// resolve, connect or time out, and any unknown text. `Parse`: the extractor could not read the
/// page (YouTube changed). The text is only inspected, never stored or returned.
fn classify_stderr(stderr: &str) -> StreamError {
    let text = stderr.to_ascii_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| text.contains(n));
    if has(&[
        "video unavailable",
        "this video is not available",
        "private video",
        "confirm your age",
        "age-restricted",
        "age restricted",
        "not available in your country",
        "not made this video available",
        "blocked it in your country",
        "blocked it on copyright",
        "members-only",
        "join this channel",
        "this video has been removed",
        "account associated with this video has been terminated",
        "requires payment",
        "premium",
    ]) {
        StreamError::NotAvailable
    } else if has(&[
        "unable to download",
        "failed to resolve",
        "getaddrinfo",
        "name or service not known",
        "temporary failure in name resolution",
        "no address associated",
        "connection refused",
        "connection reset",
        "connection aborted",
        "network is unreachable",
        "timed out",
        "timeout",
        "urlopen error",
        "ssl:",
        "http error 5",
        "http error 429",
        "too many requests",
    ]) {
        StreamError::Network("the resolver could not reach YouTube".to_owned())
    } else if has(&[
        "unable to extract",
        "unsupported url",
        "unexpected response",
        "unable to parse",
        "json",
    ]) {
        parse_err("the resolver could not read the page")
    } else {
        StreamError::Network("the resolver failed".to_owned())
    }
}

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

fn parse_err(msg: &str) -> StreamError {
    StreamError::Parse(msg.to_owned())
}

fn codec_of(acodec: &str) -> Codec {
    if acodec == "opus" {
        Codec::Opus
    } else if acodec.starts_with("mp4a") {
        Codec::Aac
    } else {
        Codec::Unknown
    }
}

/// `expire=<unix seconds>` from the URL query, if present and numeric.
pub(crate) fn expiry(url: &str) -> Option<SystemTime> {
    let query = url.split_once('?')?.1;
    let query = query.split('#').next()?;
    let secs = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("expire="))?
        .parse::<u64>()
        .ok()?;
    UNIX_EPOCH.checked_add(Duration::from_secs(secs))
}

/// An audio-only candidate read from one entry of `formats[]`.
struct Candidate {
    url: String,
    acodec: String,
    abr: Option<f64>,
    asr: Option<u32>,
    channels: Option<u16>,
}

fn candidate(entry: &Value) -> Option<Candidate> {
    if entry.get("vcodec")?.as_str()? != "none" {
        return None;
    }
    let acodec = entry.get("acodec")?.as_str()?;
    if acodec.is_empty() || acodec == "none" {
        return None;
    }
    let url = entry.get("url")?.as_str()?;
    if url.is_empty() {
        return None;
    }
    let abr = entry
        .get("abr")
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite() && *v > 0.0);
    Some(Candidate {
        url: url.to_owned(),
        acodec: acodec.to_owned(),
        abr,
        asr: entry
            .get("asr")
            .and_then(Value::as_u64)
            .and_then(|v| u32::try_from(v).ok()),
        channels: entry
            .get("audio_channels")
            .and_then(Value::as_u64)
            .and_then(|v| u16::try_from(v).ok()),
    })
}

/// Parse the single JSON document printed by `yt-dlp --dump-single-json`.
///
/// Picks the audio-only format (`vcodec` is `none`, `acodec` known) with the highest `abr`.
/// A format without `abr` ranks below any format that has one.
///
/// # Errors
/// [`StreamError::Parse`] when the text is not JSON or has no `formats` array;
/// [`StreamError::NotAvailable`] when no audio-only format with a URL exists.
pub fn parse(json: &str) -> Result<ResolvedStream, StreamError> {
    parse_with(json, None)
}

/// Like [`parse`], but only formats whose codec is in `allowed` are considered
/// (`None` allows every codec). A format with an unrecognised codec is [`Codec::Unknown`] and is
/// accepted only when `allowed` is `None` or lists [`Codec::Unknown`].
///
/// # Errors
/// Same as [`parse`]; [`StreamError::NotAvailable`] also when every format is filtered out.
pub fn parse_with(json: &str, allowed: Option<&[Codec]>) -> Result<ResolvedStream, StreamError> {
    let root: Value = serde_json::from_str(json).map_err(|_| parse_err("not valid JSON"))?;
    let formats = root
        .get("formats")
        .and_then(Value::as_array)
        .ok_or_else(|| parse_err("missing formats list"))?;
    let best = formats
        .iter()
        .filter_map(candidate)
        .filter(|c| allowed.is_none_or(|list| list.contains(&codec_of(&c.acodec))))
        .reduce(|best, next| {
            // Strictly greater, so on a tie the earlier format stays.
            if next.abr.unwrap_or(-1.0) > best.abr.unwrap_or(-1.0) {
                next
            } else {
                best
            }
        })
        .ok_or(StreamError::NotAvailable)?;

    let format = match (best.asr, best.channels) {
        (Some(sample_rate), Some(channels)) if sample_rate > 0 && channels > 0 => {
            Some(AudioFormat {
                sample_rate,
                channels,
                bit_depth: None,
            })
        }
        _ => None,
    };
    // Rounded and range-checked: abr is a small positive number of kbps.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let bitrate_kbps = best
        .abr
        .filter(|v| *v < f64::from(u32::MAX))
        .map(|v| v.round() as u32);
    Ok(ResolvedStream {
        expires_at: expiry(&best.url),
        info: StreamInfo {
            origin: Origin::YoutubeMusic,
            codec: codec_of(&best.acodec),
            bitrate_kbps,
            format,
        },
        url: Secret::new(best.url),
    })
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
        parse_with(text, self.allowed.as_deref())
    }
}

impl Default for YtDlpResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamResolver for YtDlpResolver {
    fn resolve<'a>(
        &'a self,
        video: &'a VideoId,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        let job = self.job_for(video);
        let video = video.clone();
        let timed = job.run(video);
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
mod tests {
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

    const URL_A: &str = "https://a.example.invalid/audio?expire=1900000000&sig=FAKE";
    const URL_B: &str = "https://b.example.invalid/audio?sig=FAKE2";

    fn fmt(id: &str, vcodec: &str, acodec: &str, abr: &str, url: &str) -> String {
        format!(
            r#"{{"format_id":"{id}","vcodec":"{vcodec}","acodec":"{acodec}",{abr}"asr":48000,"audio_channels":2,"url":"{url}"}}"#
        )
    }

    fn doc(formats: &[String]) -> String {
        format!(
            r#"{{"id":"fakeid00000","formats":[{}]}}"#,
            formats.join(",")
        )
    }

    #[test]
    fn best_abr_wins_and_maps_opus() {
        let json = doc(&[
            fmt("lo", "none", "mp4a.40.2", r#""abr":128.5,"#, URL_B),
            fmt("hi", "none", "opus", r#""abr":160.0,"#, URL_A),
        ]);
        let r = parse(&json).expect("parses");
        assert_eq!(r.info.codec, Codec::Opus);
        assert_eq!(r.info.bitrate_kbps, Some(160));
        assert_eq!(r.info.origin, Origin::YoutubeMusic);
        assert_eq!(
            r.info.quality_class(),
            nemo_core::audio::QualityClass::Lossy
        );
        assert_eq!(r.url.expose(), URL_A);
        assert_eq!(
            r.info.format,
            Some(AudioFormat {
                sample_rate: 48_000,
                channels: 2,
                bit_depth: None
            })
        );
        assert_eq!(
            r.expires_at,
            Some(UNIX_EPOCH + Duration::from_secs(1_900_000_000))
        );
    }

    #[test]
    fn aac_and_unknown_codecs() {
        let aac = parse(&doc(&[fmt(
            "a",
            "none",
            "mp4a.40.2",
            r#""abr":128.0,"#,
            URL_B,
        )]))
        .expect("parses");
        assert_eq!(aac.info.codec, Codec::Aac);
        assert_eq!(aac.expires_at, None);
        let other = parse(&doc(&[fmt(
            "a",
            "none",
            "vorbis",
            r#""abr":128.0,"#,
            URL_B,
        )]))
        .expect("parses");
        assert_eq!(other.info.codec, Codec::Unknown);
    }

    #[test]
    fn missing_abr_ranks_last_but_is_usable() {
        let only = parse(&doc(&[fmt("a", "none", "opus", "", URL_B)])).expect("parses");
        assert_eq!(only.info.bitrate_kbps, None);
        let both = parse(&doc(&[
            fmt("a", "none", "opus", "", URL_A),
            fmt("b", "none", "mp4a.40.2", r#""abr":48.0,"#, URL_B),
        ]))
        .expect("parses");
        assert_eq!(both.info.codec, Codec::Aac);
    }

    #[test]
    fn video_only_and_empty_lists_are_not_available() {
        let video = doc(&[
            fmt("v", "avc1.4d401f", "none", "", URL_A),
            fmt("m", "avc1.4d401f", "mp4a.40.2", r#""abr":128.0,"#, URL_B),
        ]);
        assert_eq!(parse(&video).unwrap_err(), StreamError::NotAvailable);
        assert_eq!(parse(&doc(&[])).unwrap_err(), StreamError::NotAvailable);
        let no_acodec = doc(&[fmt("a", "none", "none", r#""abr":1.0,"#, URL_A)]);
        assert_eq!(parse(&no_acodec).unwrap_err(), StreamError::NotAvailable);
    }

    #[test]
    fn garbage_gives_typed_parse_errors() {
        for bad in [
            "",
            "not json",
            "[]",
            "null",
            "{}",
            r#"{"formats":"x"}"#,
            r#"{"formats":[1,null,"a",{"vcodec":5}]}"#,
        ] {
            let r = parse(bad);
            assert!(
                matches!(r, Err(StreamError::Parse(_) | StreamError::NotAvailable)),
                "{bad:?}"
            );
        }
        assert!(matches!(parse("not json"), Err(StreamError::Parse(_))));
        assert!(matches!(parse("{}"), Err(StreamError::Parse(_))));
    }

    #[test]
    fn debug_output_hides_the_url() {
        let r = parse(&doc(&[fmt("a", "none", "opus", r#""abr":160.0,"#, URL_A)])).expect("ok");
        let shown = format!("{r:?}");
        assert!(!shown.contains("example.invalid"));
        assert!(!shown.contains("sig=FAKE"));
        assert!(shown.contains("[redacted]"));
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
    fn allowed_codecs_filter_the_choice() {
        let json = doc(&[
            fmt("opus", "none", "opus", r#""abr":160.0,"#, URL_A),
            fmt("aac", "none", "mp4a.40.2", r#""abr":128.0,"#, URL_B),
        ]);
        let aac_only = parse_with(&json, Some(&[Codec::Aac])).expect("parses");
        assert_eq!(aac_only.info.codec, Codec::Aac);
        assert_eq!(aac_only.url.expose(), URL_B);
        let both = parse_with(&json, Some(&[Codec::Aac, Codec::Opus])).expect("parses");
        assert_eq!(both.info.codec, Codec::Opus);
        assert_eq!(
            parse_with(&json, None).expect("parses").info.codec,
            Codec::Opus
        );
        assert_eq!(
            parse_with(&json, Some(&[Codec::Flac])).unwrap_err(),
            StreamError::NotAvailable
        );
        assert_eq!(
            parse_with(&json, Some(&[])).unwrap_err(),
            StreamError::NotAvailable
        );
        // An unknown codec is accepted only when the list names `Unknown`.
        let vorbis = doc(&[fmt("v", "none", "vorbis", r#""abr":96.0,"#, URL_A)]);
        assert!(parse_with(&vorbis, Some(&[Codec::Opus])).is_err());
        assert!(parse_with(&vorbis, Some(&[Codec::Unknown])).is_ok());
    }

    #[test]
    fn builder_keeps_the_allowed_codecs() {
        let resolver = YtDlpResolver::with_program("x").with_allowed_codecs(&[Codec::Opus]);
        assert_eq!(resolver.job().allowed, Some(vec![Codec::Opus]));
        assert_eq!(YtDlpResolver::new().job().allowed, None);
    }

    #[test]
    fn stderr_is_mapped_to_typed_errors() {
        let not_available = [
            "ERROR: [youtube] abc: Video unavailable",
            "ERROR: [youtube] abc: Private video. Sign in if you've been granted access",
            "ERROR: [youtube] abc: Sign in to confirm your age. This video may be inappropriate",
            "ERROR: The uploader has not made this video available in your country",
            "ERROR: [youtube] abc: This video is available to Music Premium members only",
            "ERROR: This video is not available",
        ];
        for text in not_available {
            assert_eq!(classify_stderr(text), StreamError::NotAvailable, "{text}");
        }
        let network = [
            "ERROR: [youtube] abc: Unable to download webpage: <urlopen error [Errno 11001] getaddrinfo failed>",
            "ERROR: Unable to download JSON metadata: HTTP Error 503",
            "ERROR: <urlopen error timed out>",
            "ERROR: [Errno 10061] Connection refused",
            "ERROR: HTTP Error 429: Too Many Requests",
            "",
            "some text nobody has seen before",
        ];
        for text in network {
            assert!(
                matches!(classify_stderr(text), StreamError::Network(_)),
                "{text}"
            );
        }
        let parse = [
            "ERROR: [youtube] abc: Unable to extract initial player response",
            "ERROR: Unsupported URL: https://example.invalid/",
        ];
        for text in parse {
            assert!(
                matches!(classify_stderr(text), StreamError::Parse(_)),
                "{text}"
            );
        }
    }

    #[test]
    fn mapped_errors_never_echo_stderr() {
        let secret = "ERROR: Unable to download https://host.invalid/v?sig=SECRETSIG timed out";
        let shown = format!("{:?}", classify_stderr(secret));
        assert!(!shown.contains("SECRETSIG") && !shown.contains("host.invalid"));
    }

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

    mod props {
        use super::super::parse;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn parse_never_panics(s in ".*") {
                let _ = parse(&s);
            }
        }
    }
}
