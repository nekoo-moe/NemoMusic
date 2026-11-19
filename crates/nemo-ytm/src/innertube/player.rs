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

/// The signature of a `signatureCipher`: what to solve and the query parameter that takes the
/// answer.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Cipher {
    challenge: String,
    param: String,
}

struct Candidate {
    url: String,
    cipher: Option<Cipher>,
    codec: Codec,
    bitrate: u64,
    rate: Option<u32>,
    channels: Option<u16>,
}

/// The address and signature of a `signatureCipher` (an `s=...&sp=...&url=...` query).
fn cipher_parts(entry: &Value) -> Option<(String, Cipher)> {
    let raw = entry.get("signatureCipher")?.as_str()?;
    let parsed = reqwest::Url::parse(&format!("https://cipher.invalid/?{raw}")).ok()?;
    let (mut url, mut challenge, mut param) = (None, None, None);
    for (key, value) in parsed.query_pairs() {
        match &*key {
            "url" => url = Some(value.into_owned()),
            "s" => challenge = Some(value.into_owned()),
            "sp" => param = Some(value.into_owned()),
            _ => {}
        }
    }
    Some((
        url.filter(|u| is_https(u))?,
        Cipher {
            challenge: challenge.filter(|c| !c.is_empty())?,
            param: param
                .filter(|p| !p.is_empty())
                .unwrap_or_else(|| "signature".to_owned()),
        },
    ))
}

/// The audio format of one entry. With `ciphers`, an entry whose address needs a solved
/// signature counts too; without, only a plain address does.
fn candidate(entry: &Value, ciphers: bool) -> Option<Candidate> {
    let codec = audio_codec(entry.get("mimeType")?.as_str()?)?;
    // A protected format cannot be played; nothing is done about it.
    if entry.get("drmFamilies").is_some() {
        return None;
    }
    let (url, cipher) = match entry.get("url").and_then(Value::as_str) {
        Some(url) if is_https(url) => (url.to_owned(), None),
        Some(_) => return None,
        None if ciphers => {
            let (url, cipher) = cipher_parts(entry)?;
            (url, Some(cipher))
        }
        None => return None,
    };
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
        cipher,
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

/// The audio-only format with the highest bitrate among `allowed` codecs (`None` allows all).
fn best_candidate(
    json: &str,
    allowed: Option<&[Codec]>,
    ciphers: bool,
) -> Result<Candidate, StreamError> {
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
    formats
        .iter()
        .filter_map(|entry| candidate(entry, ciphers))
        .filter(|c| allowed.is_none_or(|list| list.contains(&c.codec)))
        .reduce(|best, next| {
            if next.bitrate > best.bitrate {
                next
            } else {
                best
            }
        })
        .ok_or(StreamError::NotAvailable)
}

/// The stream of `best`, reachable at `url`.
fn stream_of(best: &Candidate, url: String) -> ResolvedStream {
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
    ResolvedStream {
        expires_at: expiry(&url),
        url: Secret::new(url),
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
    }
}

/// Read a player answer: the audio-only format with the highest bitrate among `allowed`
/// codecs (`None` allows all).
///
/// # Errors
/// [`StreamError::Parse`] for text that is not a player answer, [`StreamError::NotAvailable`]
/// when the video is not playable for this client or no allowed audio format has a plain URL.
pub fn parse_player(json: &str, allowed: Option<&[Codec]>) -> Result<ResolvedStream, StreamError> {
    let best = best_candidate(json, allowed, false)?;
    let url = best.url.clone();
    Ok(stream_of(&best, url))
}

/// A format picked from a player answer whose address may still need solving: a signature
/// (`signatureCipher`) and the `n` parameter of the address.
#[derive(Debug)]
pub struct Pending {
    best: Candidate,
}

impl std::fmt::Debug for Candidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The address is a secret.
        f.debug_struct("Candidate").finish_non_exhaustive()
    }
}

/// Read a player answer like [`parse_player`], also taking formats that carry a
/// `signatureCipher`. A format with DRM is never taken.
///
/// # Errors
/// As [`parse_player`].
pub fn parse_player_ciphered(
    json: &str,
    allowed: Option<&[Codec]>,
) -> Result<Pending, StreamError> {
    best_candidate(json, allowed, true).map(|best| Pending { best })
}

impl Pending {
    /// What has to be solved: the `n` value of the address and the signature, if any.
    #[must_use]
    pub fn challenges(&self) -> crate::cipher::Challenges {
        let n = reqwest::Url::parse(&self.best.url).ok().and_then(|url| {
            url.query_pairs()
                .find(|(key, _)| key == "n")
                .map(|(_, value)| value.into_owned())
        });
        crate::cipher::Challenges {
            n: n.into_iter().collect(),
            sig: self
                .best
                .cipher
                .iter()
                .map(|c| c.challenge.clone())
                .collect(),
        }
    }

    /// The stream, with the solved signature added and the solved `n` in place of the asked one.
    ///
    /// # Errors
    /// [`StreamError::Parse`] when a challenge has no answer in `solved`.
    pub fn finish(&self, solved: &crate::cipher::Solved) -> Result<ResolvedStream, StreamError> {
        let mut url = self.best.url.clone();
        if let Some(cipher) = &self.best.cipher {
            let sig = solved
                .sig
                .get(&cipher.challenge)
                .filter(|s| {
                    !s.is_empty()
                        && s.bytes()
                            .all(|b| b.is_ascii_graphic() && b != b'&' && b != b'#')
                })
                .ok_or_else(|| parse_err("no signature"))?;
            url = format!("{url}&{}={sig}", cipher.param);
        }
        if let Some(n) = self.challenges().n.first() {
            let solved_n = solved.n.get(n).ok_or_else(|| parse_err("no n answer"))?;
            let mut parsed =
                reqwest::Url::parse(&url).map_err(|_| parse_err("address is not valid"))?;
            let pairs: Vec<(String, String)> = parsed
                .query_pairs()
                .map(|(k, v)| {
                    let v = if k == "n" {
                        solved_n.clone()
                    } else {
                        v.into_owned()
                    };
                    (k.into_owned(), v)
                })
                .collect();
            parsed.query_pairs_mut().clear().extend_pairs(pairs);
            url = parsed.into();
        }
        Ok(stream_of(&self.best, url))
    }
}
