//! Native stream resolution: one innertube `player` request, no subprocess.
//!
//! The request shape is copied from yt-dlp, `yt_dlp/extractor/youtube/_base.py`
//! (`INNERTUBE_CLIENTS`, `_call_api`, `generate_api_headers`) and
//! `yt_dlp/extractor/youtube/_video.py` (`_extract_player_response`,
//! `_generate_player_context`, `_get_checkok_params`), master branch read on 2026-11-11:
//! `POST https://www.youtube.com/youtubei/v1/player?prettyPrint=false` with a JSON body
//! `{context: {client: {...}}, videoId, playbackContext: {contentPlaybackContext:
//! {html5Preference: "HTML5_PREF_WANTS"}}, contentCheckOk: true, racyCheckOk: true}` and the
//! headers `X-YouTube-Client-Name`, `X-YouTube-Client-Version`, `Origin` and `User-Agent`.
//!
//! Only clients that yt-dlp marks `REQUIRE_JS_PLAYER: False` are used, so the answer carries
//! plain format URLs (no signature to decipher). Formats that still carry a `signatureCipher`
//! are skipped. No cookies are sent (the client has no cookie store) and no URL is logged.
//! Same Terms of Service risk as the `yt-dlp` path (`coc.md`).

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use nemo_core::BoxFuture;
use nemo_core::audio::{AudioFormat, Codec, Origin, StreamInfo};
use nemo_core::secret::Secret;
use nemo_core::stream::{ResolvedStream, StreamError, StreamResolver};
use nemo_core::track::VideoId;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use crate::http::{InitError, is_https};
use crate::ytdlp::expiry;

/// Endpoint of the player request.
const PLAYER_URL: &str = "https://www.youtube.com/youtubei/v1/player?prettyPrint=false";
/// `Origin` header yt-dlp sends for these clients (`INNERTUBE_HOST` default).
const ORIGIN: &str = "https://www.youtube.com";
/// Total time one player request may take.
pub const PLAYER_TIMEOUT: Duration = Duration::from_secs(8);
/// Largest player answer read, in bytes.
const MAX_ANSWER_BYTES: usize = 4 * 1024 * 1024;

/// One innertube client identity, as yt-dlp's `INNERTUBE_CLIENTS` table defines it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientProfile {
    /// Short name, for logs and reports.
    pub label: &'static str,
    /// `context.client.clientName`.
    pub name: &'static str,
    /// `context.client.clientVersion` and `X-YouTube-Client-Version`.
    pub version: &'static str,
    /// `X-YouTube-Client-Name` (`INNERTUBE_CONTEXT_CLIENT_NAME`).
    pub header_id: u32,
    /// `User-Agent` of the request.
    pub user_agent: &'static str,
    /// `deviceMake`.
    pub device_make: &'static str,
    /// `deviceModel`.
    pub device_model: &'static str,
    /// `osName`.
    pub os_name: &'static str,
    /// `osVersion`.
    pub os_version: &'static str,
    /// `androidSdkVersion`, Android clients only.
    pub android_sdk: Option<u32>,
}

/// The `android_vr` client (yt-dlp `_base.py`). yt-dlp notes that since 2026-08-17 every
/// format URL of version 1.65.10 may answer 403 without a PO token; kept as a profile so the
/// owner's request can be measured, not as the default.
pub const ANDROID_VR: ClientProfile = ClientProfile {
    label: "android_vr",
    name: "ANDROID_VR",
    version: "1.65.10",
    header_id: 28,
    user_agent: "com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip",
    device_make: "Oculus",
    device_model: "Quest 3",
    os_name: "Android",
    os_version: "12L",
    android_sdk: Some(32),
};

/// The `visionos` client (yt-dlp `_base.py`, one of yt-dlp's default clients in 2026-11).
pub const VISIONOS: ClientProfile = ClientProfile {
    label: "visionos",
    name: "VISIONOS",
    version: "1.02",
    header_id: 101,
    user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15",
    device_make: "Apple",
    device_model: "RealityDevice17,1",
    os_name: "visionOS",
    os_version: "26.5.23O471",
    android_sdk: None,
};

/// The JSON body of a player request.
#[must_use]
pub fn player_body(profile: &ClientProfile, video: &VideoId) -> Value {
    let mut client = json!({
        "clientName": profile.name,
        "clientVersion": profile.version,
        "deviceMake": profile.device_make,
        "deviceModel": profile.device_model,
        "userAgent": profile.user_agent,
        "osName": profile.os_name,
        "osVersion": profile.os_version,
        "hl": "en",
        "timeZone": "UTC",
        "utcOffsetMinutes": 0,
    });
    if let (Some(sdk), Some(map)) = (profile.android_sdk, client.as_object_mut()) {
        map.insert("androidSdkVersion".to_owned(), json!(sdk));
    }
    json!({
        "context": { "client": client },
        "videoId": video.as_str(),
        "playbackContext": {
            "contentPlaybackContext": { "html5Preference": "HTML5_PREF_WANTS" }
        },
        "contentCheckOk": true,
        "racyCheckOk": true,
    })
}

fn parse_err(msg: &str) -> StreamError {
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
        info: StreamInfo {
            origin: Origin::YoutubeMusic,
            codec: best.codec,
            bitrate_kbps: kbps,
            format,
        },
    })
}

/// Resolves streams with one innertube player request per client profile, in order.
#[derive(Debug, Clone)]
pub struct InnertubeResolver {
    client: reqwest::Client,
    handle: Handle,
    profiles: Vec<ClientProfile>,
    allowed: Option<Vec<Codec>>,
    visitor: Visitor,
}

impl InnertubeResolver {
    /// A resolver that runs on `handle` and tries `profiles` in order.
    ///
    /// # Errors
    /// [`InitError::Client`] when the HTTP client cannot be built.
    pub fn new(handle: Handle, profiles: &[ClientProfile]) -> Result<Self, InitError> {
        let client = reqwest::Client::builder()
            .connect_timeout(PLAYER_TIMEOUT)
            .timeout(PLAYER_TIMEOUT)
            .redirect(crate::http::https_redirects())
            .https_only(true)
            .build()
            .map_err(|_| InitError::Client)?;
        Ok(Self {
            client,
            handle,
            profiles: profiles.to_vec(),
            allowed: None,
            visitor: Arc::default(),
        })
    }

    /// Only pick formats whose codec the audio engine can decode.
    #[must_use]
    pub fn with_allowed_codecs(mut self, codecs: &[Codec]) -> Self {
        self.allowed = Some(codecs.to_vec());
        self
    }

    /// Development aid: one player request per profile (with the visitor retry of `resolve`),
    /// described by [`describe_player`].
    pub async fn describe(&self, video: &VideoId) -> Vec<String> {
        let mut out = Vec::new();
        for profile in self.profiles.clone() {
            let (client, visitor, video) = (
                self.client.clone(),
                Arc::clone(&self.visitor),
                video.clone(),
            );
            let task = self.handle.spawn(async move {
                match send(&client, &profile, &video, &visitor).await {
                    Ok((_, true)) => send(&client, &profile, &video, &visitor).await.map_or_else(
                        |e| e.to_string(),
                        |(t, _)| format!("retried with visitor data: {}", describe_player(&t)),
                    ),
                    Ok((text, false)) => describe_player(&text),
                    Err(e) => e.to_string(),
                }
            });
            let line = task.await.unwrap_or_else(|_| "cancelled".to_owned());
            out.push(format!("{}: {line}", profile.label));
        }
        out
    }
}

/// The anonymous visitor id YouTube hands out in `responseContext.visitorData`, shared by the
/// requests of one resolver. It is not an account cookie and is never logged.
type Visitor = Arc<Mutex<Option<String>>>;

/// `responseContext.visitorData` of an answer, when it looks like one.
fn visitor_of(json: &Value) -> Option<String> {
    json.pointer("/responseContext/visitorData")
        .and_then(Value::as_str)
        .filter(|v| {
            !v.is_empty()
                && v.len() <= 256
                && v.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"%_-=".contains(&b))
        })
        .map(str::to_owned)
}

/// Send one player request. Returns the answer text and `true` when the answer refused the
/// request (bot check) while no visitor data was sent yet but now is known: worth one retry.
async fn send(
    client: &reqwest::Client,
    profile: &ClientProfile,
    video: &VideoId,
    visitor: &Visitor,
) -> Result<(String, bool), StreamError> {
    let known = visitor
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let mut body = player_body(profile, video);
    if let Some(v) = &known {
        body["context"]["client"]["visitorData"] = json!(v);
    }
    let mut request = client
        .post(PLAYER_URL)
        .header("Content-Type", "application/json")
        .header("User-Agent", profile.user_agent)
        .header("X-YouTube-Client-Name", profile.header_id.to_string())
        .header("X-YouTube-Client-Version", profile.version)
        .header("Origin", ORIGIN);
    if let Some(v) = &known {
        request = request.header("X-Goog-Visitor-Id", v.as_str());
    }
    let response = request
        .body(body.to_string())
        .send()
        .await
        .map_err(|_| StreamError::Network("player request failed".to_owned()))?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(StreamError::Network(format!(
            "player request: HTTP {status}"
        )));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|_| StreamError::Network("player answer was cut off".to_owned()))?;
    if bytes.len() > MAX_ANSWER_BYTES {
        return Err(parse_err("player answer too large"));
    }
    let text = String::from_utf8(bytes.to_vec()).map_err(|_| parse_err("answer is not UTF-8"))?;
    let mut retry = false;
    if let Ok(root) = serde_json::from_str::<Value>(&text)
        && let Some(fresh) = visitor_of(&root)
    {
        let refused = root
            .pointer("/playabilityStatus/status")
            .and_then(Value::as_str)
            == Some("LOGIN_REQUIRED");
        retry = refused && known.is_none();
        let mut slot = visitor.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.is_none() {
            *slot = Some(fresh);
        }
    }
    Ok((text, retry))
}

async fn ask(
    client: reqwest::Client,
    profile: ClientProfile,
    video: VideoId,
    visitor: Visitor,
    allowed: Option<Vec<Codec>>,
) -> Result<ResolvedStream, StreamError> {
    let (mut text, retry) = send(&client, &profile, &video, &visitor).await?;
    if retry {
        text = send(&client, &profile, &video, &visitor).await?.0;
    }
    parse_player(&text, allowed.as_deref())
}

impl StreamResolver for InnertubeResolver {
    fn resolve<'a>(
        &'a self,
        video: &'a VideoId,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        let (client, profiles, allowed, visitor) = (
            self.client.clone(),
            self.profiles.clone(),
            self.allowed.clone(),
            Arc::clone(&self.visitor),
        );
        let video = video.clone();
        let task = self.handle.spawn(async move {
            let mut last = StreamError::NotAvailable;
            for profile in profiles {
                let asked = ask(
                    client.clone(),
                    profile,
                    video.clone(),
                    Arc::clone(&visitor),
                    allowed.clone(),
                )
                .await;
                match asked {
                    Ok(stream) => return Ok(stream),
                    Err(e) => {
                        tracing::debug!(client = profile.label, error = %e, "player request");
                        last = e;
                    }
                }
            }
            Err(last)
        });
        Box::pin(async move {
            task.await
                .unwrap_or_else(|_| Err(StreamError::Network("resolve was cancelled".to_owned())))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vid() -> VideoId {
        VideoId::parse("abcdefghijk").expect("valid")
    }

    const URL_AAC: &str = "https://r1.example.invalid/videoplayback?expire=1900000000&itag=140";
    const URL_OPUS: &str = "https://r1.example.invalid/videoplayback?itag=251";

    fn answer(formats: &Value) -> String {
        json!({
            "playabilityStatus": { "status": "OK" },
            "streamingData": { "adaptiveFormats": formats }
        })
        .to_string()
    }

    #[test]
    fn visitor_data_is_taken_only_when_it_looks_like_one() {
        let ok = json!({ "responseContext": { "visitorData": "Cgt4eXo%3D_-" } });
        assert_eq!(visitor_of(&ok).as_deref(), Some("Cgt4eXo%3D_-"));
        for bad in [
            json!({}),
            json!({ "responseContext": { "visitorData": "a b" } }),
            json!({ "responseContext": { "visitorData": "" } }),
        ] {
            assert_eq!(visitor_of(&bad), None);
        }
        let line = describe_player(
            &json!({ "playabilityStatus": { "status": "LOGIN_REQUIRED", "reason": "bot" } })
                .to_string(),
        );
        assert!(
            line.contains("LOGIN_REQUIRED") && line.contains("0 plain"),
            "{line}"
        );
    }

    #[test]
    fn body_matches_the_yt_dlp_shape() {
        let body = player_body(&ANDROID_VR, &vid());
        assert_eq!(body["videoId"], "abcdefghijk");
        assert_eq!(body["context"]["client"]["clientName"], "ANDROID_VR");
        assert_eq!(body["context"]["client"]["androidSdkVersion"], 32);
        assert_eq!(body["contentCheckOk"], true);
        assert_eq!(body["racyCheckOk"], true);
        assert_eq!(
            body["playbackContext"]["contentPlaybackContext"]["html5Preference"],
            "HTML5_PREF_WANTS"
        );
        let vision = player_body(&VISIONOS, &vid());
        assert!(
            vision["context"]["client"]
                .get("androidSdkVersion")
                .is_none()
        );
        assert!(!body.to_string().to_ascii_lowercase().contains("cookie"));
    }

    #[test]
    fn best_aac_with_plain_url_is_picked() {
        let json = answer(&json!([
            { "itag": 137, "mimeType": "video/mp4; codecs=\"avc1\"", "bitrate": 4_000_000, "url": URL_AAC },
            { "itag": 251, "mimeType": "audio/webm; codecs=\"opus\"", "bitrate": 160_000, "url": URL_OPUS },
            { "itag": 140, "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "bitrate": 130_000,
              "averageBitrate": 129_500, "audioSampleRate": "44100", "audioChannels": 2, "url": URL_AAC },
            { "itag": 141, "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "bitrate": 260_000,
              "signatureCipher": "s=x&url=y" },
            { "itag": 140, "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "bitrate": 140_000,
              "isDrc": true, "url": URL_OPUS }
        ]));
        let s = parse_player(&json, Some(&[Codec::Aac])).expect("parses");
        assert_eq!(s.url.expose(), URL_AAC);
        assert_eq!(s.info.codec, Codec::Aac);
        assert_eq!(s.info.bitrate_kbps, Some(130));
        assert_eq!(
            s.info.format,
            Some(AudioFormat {
                sample_rate: 44_100,
                channels: 2,
                bit_depth: None
            })
        );
        assert!(s.expires_at.is_some());
        assert!(!format!("{s:?}").contains("example.invalid"));
        let any = parse_player(&json, None).expect("parses");
        assert_eq!(any.info.codec, Codec::Opus);
    }

    #[test]
    fn unplayable_and_garbage() {
        let login = json!({ "playabilityStatus": { "status": "LOGIN_REQUIRED" } }).to_string();
        assert_eq!(
            parse_player(&login, None).unwrap_err(),
            StreamError::NotAvailable
        );
        let no_audio = answer(&json!([{ "mimeType": "video/mp4", "url": URL_AAC }]));
        assert_eq!(
            parse_player(&no_audio, None).unwrap_err(),
            StreamError::NotAvailable
        );
        let http = answer(
            &json!([{ "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "url": "http://x.invalid/a" }]),
        );
        assert_eq!(
            parse_player(&http, None).unwrap_err(),
            StreamError::NotAvailable
        );
        for bad in ["", "null", "{}", "[1]", "{\"playabilityStatus\":5}"] {
            assert!(parse_player(bad, None).is_err(), "{bad:?}");
        }
        let ok_no_data = json!({ "playabilityStatus": { "status": "OK" } }).to_string();
        assert_eq!(
            parse_player(&ok_no_data, None).unwrap_err(),
            StreamError::NotAvailable
        );
    }

    mod props {
        use super::super::parse_player;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn never_panics(s in ".*") {
                let _ = parse_player(&s, None);
            }
        }
    }
}
