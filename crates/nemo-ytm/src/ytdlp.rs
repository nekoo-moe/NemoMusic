//! Stream resolution through the `yt-dlp` subprocess (ADR 0003, tech.md).
//!
//! Resolution is anonymous: no cookies and no config files reach `yt-dlp` (ADR 0001).
//! Option names were checked against the official yt-dlp README on 2026-11-10:
//! `-J/--dump-single-json`, `--no-playlist`, `--no-warnings`, `-f/--format`,
//! `--ignore-config`, and format fields `acodec`, `vcodec`, `abr`, `asr`,
//! `audio_channels`, `url`, `format_id`. The `expire` URL parameter is a YouTube convention
//! that the README does not document; it is optional here.

use std::ffi::{OsStr, OsString};
use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nemo_core::BoxFuture;
use nemo_core::audio::{AudioFormat, Codec, Origin, StreamInfo};
use nemo_core::secret::Secret;
use nemo_core::stream::{ResolvedStream, StreamError, StreamResolver};
use nemo_core::track::VideoId;
use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

/// How long one `yt-dlp` run may take.
pub const RESOLVE_TIMEOUT: Duration = Duration::from_secs(20);
/// Largest stdout accepted from `yt-dlp`, in bytes.
pub const MAX_OUTPUT_BYTES: u64 = 8 * 1024 * 1024;

/// Build the `yt-dlp` argument list for one video. Never a shell string.
///
/// The URL comes after `--`, so nothing after it can be read as an option. The id is already
/// validated by [`VideoId`], and the URL starts with `https://`, so a leading `-` in the id
/// cannot start an option either.
#[must_use]
pub fn args(video: &VideoId) -> Vec<OsString> {
    let mut list: Vec<OsString> = [
        "--ignore-config",
        "--dump-single-json",
        "--no-playlist",
        "--no-warnings",
        "--format",
        "bestaudio",
        "--",
    ]
    .iter()
    .map(OsString::from)
    .collect();
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
fn expiry(url: &str) -> Option<SystemTime> {
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
    let root: Value = serde_json::from_str(json).map_err(|_| parse_err("not valid JSON"))?;
    let formats = root
        .get("formats")
        .and_then(Value::as_array)
        .ok_or_else(|| parse_err("missing formats list"))?;
    let best = formats
        .iter()
        .filter_map(candidate)
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
#[derive(Debug, Clone)]
pub struct YtDlpResolver {
    program: PathBuf,
}

impl YtDlpResolver {
    /// Use the program named `yt-dlp`, found through `PATH`.
    #[must_use]
    pub fn new() -> Self {
        Self::with_program("yt-dlp")
    }

    /// Use a specific executable, for example a bundled copy.
    #[must_use]
    pub fn with_program(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
        }
    }

    /// The configured executable path.
    #[must_use]
    pub fn program(&self) -> &OsStr {
        self.program.as_os_str()
    }

    async fn run(&self, video: &VideoId) -> Result<ResolvedStream, StreamError> {
        let mut child = Command::new(&self.program)
            .args(args(video))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| match e.kind() {
                ErrorKind::NotFound => StreamError::ResolverMissing,
                _ => StreamError::Network("could not start the resolver".to_owned()),
            })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| parse_err("no resolver output"))?;
        let mut out = Vec::new();
        // Read one byte past the cap so an oversized answer is detected, not truncated silently.
        stdout
            .take(MAX_OUTPUT_BYTES + 1)
            .read_to_end(&mut out)
            .await
            .map_err(|_| StreamError::Network("could not read resolver output".to_owned()))?;
        if out.len() as u64 > MAX_OUTPUT_BYTES {
            return Err(parse_err("resolver output too large"));
        }
        let status = child
            .wait()
            .await
            .map_err(|_| StreamError::Network("could not wait for the resolver".to_owned()))?;
        if !status.success() {
            // stderr is not read, so a region block and a network failure look alike here.
            return Err(StreamError::NotAvailable);
        }
        let text = std::str::from_utf8(&out).map_err(|_| parse_err("output is not UTF-8"))?;
        parse(text)
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
        Box::pin(async move {
            tokio::time::timeout(RESOLVE_TIMEOUT, self.run(video))
                .await
                .unwrap_or_else(|_| Err(StreamError::Network("resolver timed out".to_owned())))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
