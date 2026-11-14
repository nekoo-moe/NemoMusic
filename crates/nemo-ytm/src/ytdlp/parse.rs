//! Reading `yt-dlp` output: the stream JSON and the meaning of a failed run.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nemo_core::audio::{AudioFormat, Codec, Origin, StreamInfo};
use nemo_core::secret::Secret;
use nemo_core::stream::{ResolvedStream, StreamError, Transport};
use serde_json::Value;

/// Decide what a failed `yt-dlp` run means from the start of its stderr.
///
/// `NotAvailable`: region, age, private, removed or members-only videos. `Network`: cannot
/// resolve, connect or time out, and any unknown text. `Parse`: the extractor could not read the
/// page (YouTube changed). The text is only inspected, never stored or returned.
pub(super) fn classify_stderr(stderr: &str) -> StreamError {
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

pub(super) fn parse_err(msg: &str) -> StreamError {
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
        transport: Transport::File,
        preview: false,
        audio_language: None,
        audio_tracks: Vec::new(),
        info: StreamInfo {
            origin: Origin::YoutubeMusic,
            codec: codec_of(&best.acodec),
            bitrate_kbps,
            format,
        },
        url: Secret::new(best.url),
    })
}

#[cfg(test)]
mod tests;
