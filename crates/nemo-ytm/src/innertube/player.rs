//! Reading a player answer: the best audio format and a one-line description for diagnostics.

use nemo_core::audio::{AudioFormat, Codec, Origin, StreamInfo};
use nemo_core::secret::Secret;
use nemo_core::stream::{ResolvedStream, StreamError, Transport};
use serde_json::Value;

use crate::http::is_https;
use crate::ytdlp::expiry;

pub(super) fn parse_err(msg: &str) -> StreamError {
    StreamError::Parse(msg.to_owned())
}

/// Codec from a `mimeType` such as `audio/mp4; codecs="mp4a.40.2"`. `None` when not audio.
fn audio_codec(mime: &str) -> Option<Codec> {
    let mime = mime.to_ascii_lowercase();
    if !mime.starts_with("audio/") {
        return None;
    }
    Some(if mime.contains("mp4a") {
        Codec::Aac
    } else if mime.contains("opus") {
        Codec::Opus
    } else {
        Codec::Unknown
    })
}

struct Candidate<'a> {
    url: &'a str,
    codec: Codec,
    bitrate: u64,
    rate: Option<u32>,
    channels: Option<u16>,
}

fn candidate(entry: &Value) -> Option<Candidate<'_>> {
    let codec = audio_codec(entry.get("mimeType")?.as_str()?)?;
    // A format with `signatureCipher` needs the JS player; skip it.
    let url = entry.get("url")?.as_str().filter(|u| is_https(u))?;
    // Prefer the plain track: skip "stable volume" (DRC) and non-default dubbed tracks.
    if entry.get("isDrc").and_then(Value::as_bool) == Some(true)
        || entry
            .pointer("/audioTrack/audioIsDefault")
            .and_then(Value::as_bool)
            == Some(false)
    {
        return None;
    }
    let bitrate = entry
        .get("averageBitrate")
        .or_else(|| entry.get("bitrate"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let rate = entry
        .get("audioSampleRate")
        .and_then(|v| v.as_str().and_then(|s| s.parse().ok()).or(v.as_u64()))
        .and_then(|v| u32::try_from(v).ok());
    let channels = entry
        .get("audioChannels")
        .and_then(Value::as_u64)
        .and_then(|v| u16::try_from(v).ok());
    Some(Candidate {
        url,
        codec,
        bitrate,
        rate,
        channels,
    })
}

/// One line about a player answer for diagnostics: playability status and reason, and how many
/// audio formats carry a plain URL or a cipher. Never a URL.
#[must_use]
pub fn describe_player(json: &str) -> String {
    let Ok(root) = serde_json::from_str::<Value>(json) else {
        return "not JSON".to_owned();
    };
    let status = root
        .pointer("/playabilityStatus/status")
        .and_then(Value::as_str)
        .unwrap_or("-");
    let reason: String = root
        .pointer("/playabilityStatus/reason")
        .and_then(Value::as_str)
        .unwrap_or("")
        .chars()
        .take(80)
        .collect();
    let formats = root
        .pointer("/streamingData/adaptiveFormats")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let audio = formats.iter().filter(|f| {
        f.get("mimeType")
            .and_then(Value::as_str)
            .is_some_and(|m| m.starts_with("audio/"))
    });
    let (mut plain, mut cipher) = (0, 0);
    for f in audio {
        if f.get("url").is_some() {
            plain += 1;
        } else {
            cipher += 1;
        }
    }
    let sabr = root
        .pointer("/streamingData/serverAbrStreamingUrl")
        .is_some();
    format!(
        "status {status} reason '{reason}', audio formats: {plain} plain, {cipher} ciphered, sabr {sabr}"
    )
}

/// Read a player answer: the audio-only format with the highest bitrate among `allowed`
/// codecs (`None` allows all).
///
/// # Errors
/// [`StreamError::Parse`] for text that is not a player answer, [`StreamError::NotAvailable`]
/// when the video is not playable for this client or no allowed audio format has a plain URL.
pub fn parse_player(json: &str, allowed: Option<&[Codec]>) -> Result<ResolvedStream, StreamError> {
    let root: Value = serde_json::from_str(json).map_err(|_| parse_err("not valid JSON"))?;
    let status = root
        .pointer("/playabilityStatus/status")
        .and_then(Value::as_str)
        .ok_or_else(|| parse_err("no playability status"))?;
    if status != "OK" {
        return Err(StreamError::NotAvailable);
    }
    let formats = root
        .pointer("/streamingData/adaptiveFormats")
        .and_then(Value::as_array)
        .ok_or(StreamError::NotAvailable)?;
    let best = formats
        .iter()
        .filter_map(candidate)
        .filter(|c| allowed.is_none_or(|list| list.contains(&c.codec)))
        .reduce(|best, next| {
            if next.bitrate > best.bitrate {
                next
            } else {
                best
            }
        })
        .ok_or(StreamError::NotAvailable)?;
    let format = match (best.rate, best.channels) {
        (Some(sample_rate), Some(channels)) if sample_rate > 0 && channels > 0 => {
            Some(AudioFormat {
                sample_rate,
                channels,
                bit_depth: None,
            })
        }
        _ => None,
    };
    let kbps = u32::try_from((best.bitrate + 500) / 1000)
        .ok()
        .filter(|k| *k > 0);
    Ok(ResolvedStream {
        url: Secret::new(best.url.to_owned()),
        expires_at: expiry(best.url),
        transport: Transport::File,
        preview: false,
        audio_language: None,
        audio_tracks: Vec::new(),
        info: StreamInfo {
            origin: Origin::YoutubeMusic,
            codec: best.codec,
            bitrate_kbps: kbps,
            format,
        },
    })
}
