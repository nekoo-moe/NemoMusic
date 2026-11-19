//! The `yt-dlp` fallback for pages that are not YouTube videos: a [`UrlResolver`] that asks
//! `yt-dlp` for the formats of a SoundCloud track page (soundcloud-full task 14).
//!
//! `nemo-app` puts it behind the native SoundCloud resolver and asks it only when the native one
//! could not read SoundCloud's answer (the web app changed). It never replaces a refusal: an
//! encrypted-only track, a preview and a blocked track stay what they are, and nothing is tried
//! against them. `yt-dlp` runs anonymously (no cookies, no config file), one process per call,
//! with the page address after `--`. The formats are ranked with [`rank_formats`] like the native
//! ones, so the choice is the same.

use std::ffi::OsString;
use std::time::SystemTime;

use nemo_core::BoxFuture;
use nemo_core::audio::{Codec, Origin, StreamInfo};
use nemo_core::secret::Secret;
use nemo_core::stream::{ResolvedStream, StreamError, Transport};
use nemo_core::track::MediaUrl;
use nemo_core::url_resolver::{
    FormatCandidate, FormatPolicy, MediaEntry, MediaQuery, ResolverState, UrlResolver, rank_formats,
};
use serde_json::Value;

use crate::ytdlp::{ProgramSlot, YtDlpResolver};

/// Hosts a stream address may have: the SoundCloud hosts of the README egress list.
const HOSTS: &[&str] = &[
    "soundcloud.com",
    "api-v2.soundcloud.com",
    "a-v2.sndcdn.com",
    "i1.sndcdn.com",
    "playback.media-streaming.soundcloud.cloud",
    "*.sndcdn.com",
];

/// How long the address `yt-dlp` gave is worth keeping (the player asks again after that).
const KEEP: std::time::Duration = std::time::Duration::from_mins(3);

/// Resolves the page of a SoundCloud track with `yt-dlp`.
#[derive(Debug, Clone)]
pub struct YtDlpUrlResolver {
    runner: YtDlpResolver,
}

impl YtDlpUrlResolver {
    /// Share the program (and the installed-copy slot) of `runner`.
    #[must_use]
    pub const fn new(runner: YtDlpResolver) -> Self {
        Self { runner }
    }

    /// The slot that switches the program to a newly installed copy.
    #[must_use]
    pub fn program_slot(&self) -> ProgramSlot {
        self.runner.program_slot()
    }
}

/// The argument list for one page: no config file, no playlist, no warnings, JSON, and the
/// address after `--`. Never a shell string.
fn arguments(page: &MediaUrl) -> Vec<OsString> {
    [
        "--ignore-config",
        "--no-warnings",
        "--no-playlist",
        "-J",
        "--",
        page.as_str(),
    ]
    .iter()
    .map(OsString::from)
    .collect()
}

/// The codec a format names: `mp4a.40.2` or `aac` is AAC, `mp3`, `opus`; the extension decides
/// when the codec field is empty.
fn codec_of(format: &Value) -> Option<Codec> {
    let acodec = format.get("acodec").and_then(Value::as_str).unwrap_or("");
    let ext = format.get("ext").and_then(Value::as_str).unwrap_or("");
    match (acodec, ext) {
        (a, _) if a.starts_with("mp4a") || a == "aac" => Some(Codec::Aac),
        (a, _) if a.starts_with("mp3") => Some(Codec::Mp3),
        (a, _) if a.starts_with("opus") => Some(Codec::Opus),
        ("" | "none", "m4a" | "aac") => Some(Codec::Aac),
        ("" | "none", "mp3") => Some(Codec::Mp3),
        ("" | "none", "opus") => Some(Codec::Opus),
        _ => None,
    }
}

/// One format that could be played.
struct Offer {
    url: String,
    candidate: FormatCandidate,
}

fn offers(info: &Value) -> Vec<Offer> {
    let Some(formats) = info.get("formats").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for format in formats {
        let (Some(url), Some(codec)) =
            (format.get("url").and_then(Value::as_str), codec_of(format))
        else {
            continue;
        };
        // The address is fetched without any credential: it must be https on a SoundCloud host.
        if MediaUrl::parse(url, HOSTS).is_err() {
            continue;
        }
        let id = format
            .get("format_id")
            .and_then(Value::as_str)
            .unwrap_or("");
        let protocol = format
            .get("protocol")
            .and_then(Value::as_str)
            .unwrap_or("http");
        let bitrate = format
            .get("abr")
            .and_then(Value::as_f64)
            .filter(|abr| (8.0..=512.0).contains(abr));
        out.push(Offer {
            url: url.to_owned(),
            candidate: FormatCandidate {
                index: out.len(),
                codec,
                // The value is a small positive number; rounding to whole kbit/s is the intent.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                bitrate_kbps: bitrate.map(|b| b.round() as u32),
                hls: protocol.starts_with("m3u8"),
                // yt-dlp names the encrypted kinds `hls-aes` in the format id.
                encrypted: id.starts_with("hls-aes") || id.contains("encrypted"),
                preview: id.contains("preview"),
            },
        });
    }
    out
}

/// The stream `yt-dlp`'s answer offers, ranked as the native resolver ranks.
///
/// # Errors
/// [`StreamError::Parse`] when the text is not `yt-dlp` JSON, [`StreamError::NotAvailable`] when
/// no format can be played (encrypted only, or a codec the decoder cannot read).
pub fn parse_page(text: &str, policy: &FormatPolicy) -> Result<ResolvedStream, StreamError> {
    let info: Value = serde_json::from_str(text)
        .map_err(|_| StreamError::Parse("unexpected resolver answer".to_owned()))?;
    let offers = offers(&info);
    let candidates: Vec<FormatCandidate> = offers.iter().map(|o| o.candidate).collect();
    let pick = rank_formats(&candidates, policy).ok_or(StreamError::NotAvailable)?;
    let offer = offers.get(pick).ok_or(StreamError::NotAvailable)?;
    let c = &offer.candidate;
    Ok(ResolvedStream {
        url: Secret::new(offer.url.clone()),
        info: StreamInfo {
            origin: Origin::SoundCloud,
            codec: c.codec,
            bitrate_kbps: c.bitrate_kbps,
            format: None,
        },
        expires_at: SystemTime::now().checked_add(KEEP),
        transport: if c.hls {
            Transport::Hls { live: false }
        } else {
            Transport::File
        },
        preview: c.preview,
        audio_language: None,
        audio_tracks: Vec::new(),
    })
}

impl UrlResolver for YtDlpUrlResolver {
    fn resolve<'a>(
        &'a self,
        url: &'a MediaUrl,
        policy: &'a FormatPolicy,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        Box::pin(async move {
            let text = self.runner.run_json(arguments(url)).await?;
            parse_page(&text, policy)
        })
    }

    /// Searches and pages come from the native catalog, not from `yt-dlp`.
    fn list<'a>(
        &'a self,
        _query: &'a MediaQuery,
    ) -> BoxFuture<'a, Result<Vec<MediaEntry>, StreamError>> {
        Box::pin(async { Err(StreamError::NotAvailable) })
    }

    fn state(&self) -> ResolverState {
        ResolverState::Ready {
            version: "yt-dlp".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests;
