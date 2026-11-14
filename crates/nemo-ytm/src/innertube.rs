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
use nemo_core::audio::Codec;
use nemo_core::stream::{ResolvedStream, StreamError, StreamResolver};
use nemo_core::track::VideoId;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use crate::http::InitError;

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

mod player;

use player::parse_err;
pub use player::{describe_player, parse_player};

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
mod tests;
