//! The SoundCloud web API v2 client: client id, pacing, backoff and retries.
//!
//! Every API call carries the web app's `client_id` (see [`crate::client_id`]). Calls go one at
//! a time and start at least [`Pacing::min_gap`] apart. A 429 stops all calls for its
//! `Retry-After`. Reads that fail with a 5xx or a connection error are retried twice. A 401 or
//! 403 makes the client look the id up again once. Nothing here logs an address, an id or a
//! response body; errors are fixed texts (`soundcloud-full` criteria 4 and 6).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use nemo_core::secret::Secret;

use crate::auth::Auth;
use crate::client_id::{MAX_SCRIPTS, find_client_id, script_urls};
use crate::http::{MAX_JSON_BYTES, MAX_SCRIPT_BYTES, ScAnswer, ScHttp, ScHttpError, ScRequest};
use crate::records::TrackRecords;

/// Where the client reads from. [`Endpoints::production`] is the real service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    /// The home page, which lists the scripts. No trailing slash.
    pub site: String,
    /// The API base. No trailing slash.
    pub api: String,
    /// A script address must start with this to be read.
    pub script_prefix: String,
}

impl Endpoints {
    /// The real SoundCloud hosts (`HOSTS`).
    #[must_use]
    pub fn production() -> Self {
        Self {
            site: "https://soundcloud.com".to_owned(),
            api: "https://api-v2.soundcloud.com".to_owned(),
            script_prefix: "https://a-v2.sndcdn.com/".to_owned(),
        }
    }
}

/// How fast the client may go. The defaults are the human-paced rules of the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pacing {
    /// Least time between the starts of two requests.
    pub min_gap: Duration,
    /// Waits before the first and the second retry of a read.
    pub retry_delays: [Duration; 2],
    /// How long a 429 without `Retry-After` stops all calls.
    pub default_backoff: Duration,
    /// The longest a 429 may stop all calls, whatever `Retry-After` says.
    pub max_backoff: Duration,
}

impl Default for Pacing {
    fn default() -> Self {
        Self {
            min_gap: Duration::from_millis(250),
            retry_delays: [Duration::from_secs(1), Duration::from_secs(3)],
            default_backoff: Duration::from_mins(1),
            max_backoff: Duration::from_mins(15),
        }
    }
}

/// A call that did not give an answer the caller can use. The texts are fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ClientError {
    /// SoundCloud asked to slow down; no request is made until the backoff ends.
    #[error("too many requests; try again later")]
    RateLimited,
    /// The page does not exist, or is private or removed.
    #[error("not found")]
    NotFound,
    /// SoundCloud refused the request (401 or 403 after a new client id, or another 4xx).
    #[error("SoundCloud refused the request")]
    Refused,
    /// The network failed or SoundCloud answered with a server error.
    #[error("could not reach SoundCloud")]
    Network,
    /// The request took too long.
    #[error("the request timed out")]
    Timeout,
    /// The answer was not what the client expects.
    #[error("unexpected answer from SoundCloud")]
    Parse,
    /// No script of the home page holds a client id: the web app changed.
    #[error("could not find the SoundCloud web app's client id")]
    NoClientId,
    /// SoundCloud rejected the signed-in session (401 twice, with a fresh client id). The token
    /// is no longer sent.
    #[error("SoundCloud ended this sign-in")]
    SessionEnded,
    /// The user switched SoundCloud off: no request is made.
    #[error("SoundCloud is switched off")]
    Off,
}

impl From<ScHttpError> for ClientError {
    fn from(e: ScHttpError) -> Self {
        match e {
            ScHttpError::Timeout => Self::Timeout,
            ScHttpError::Failed | ScHttpError::TooLarge | ScHttpError::Cancelled => Self::Network,
        }
    }
}

/// The client. Share it in an `Arc` between the catalog and the stream lookup.
pub struct ScClient {
    http: Arc<dyn ScHttp>,
    endpoints: Endpoints,
    pacing: Pacing,
    /// The client id. Held across a lookup, so a second caller waits for the first.
    id: tokio::sync::Mutex<Option<Secret<String>>>,
    /// Held across a whole request: one at a time. Holds when the last one started.
    gate: tokio::sync::Mutex<Option<Instant>>,
    /// Until when a 429 stops all calls.
    blocked_until: Mutex<Option<Instant>>,
    /// What the catalog learned about tracks, for the stream lookup.
    records: TrackRecords,
    /// Calls a listener is waiting for. Background calls (warming the next tracks) wait until
    /// there is none, so a click is never queued behind a hint.
    interactive: AtomicUsize,
    /// Told when a listener's call starts (the stream reader uses it to keep its connection
    /// to the media host open).
    activity: Mutex<Option<Activity>>,
    /// The session token of the signed-in account, sent to the API host only.
    auth: Auth,
    /// Cleared when the user switches SoundCloud off: every later request fails at once.
    enabled: AtomicBool,
}

/// A callback for the start of a listener's call.
pub type Activity = Arc<dyn Fn() + Send + Sync>;

/// Counts a listener's call while it is pending.
struct Pending<'a>(&'a AtomicUsize);

impl<'a> Pending<'a> {
    fn new(counter: &'a AtomicUsize) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self(counter)
    }
}

impl Drop for Pending<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// How often a background call looks again while a listener's call is pending.
const BACKGROUND_POLL: Duration = Duration::from_millis(20);

impl std::fmt::Debug for ScClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScClient").finish_non_exhaustive()
    }
}

impl ScClient {
    /// A client for the real service with the default pacing.
    #[must_use]
    pub fn new(http: Arc<dyn ScHttp>) -> Self {
        Self::with(http, Endpoints::production(), Pacing::default())
    }

    /// A client with its own endpoints and pacing (tests, loopback servers).
    #[must_use]
    pub fn with(http: Arc<dyn ScHttp>, endpoints: Endpoints, pacing: Pacing) -> Self {
        Self {
            http,
            endpoints,
            pacing,
            id: tokio::sync::Mutex::new(None),
            gate: tokio::sync::Mutex::new(None),
            blocked_until: Mutex::new(None),
            records: TrackRecords::default(),
            interactive: AtomicUsize::new(0),
            activity: Mutex::new(None),
            auth: Auth::default(),
            enabled: AtomicBool::new(true),
        }
    }

    /// Switch the client on or off. While off, every request fails with [`ClientError::Off`]
    /// before anything is sent (ADR 0004, opt-in rule 6: requests stop at once).
    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::SeqCst);
    }

    /// Whether requests may be sent.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    /// Whether `url` is an address on the API host. Only such addresses are followed (`next_href`).
    #[must_use]
    pub fn is_api_url(&self, url: &str) -> bool {
        self.api_path(url).is_ok()
    }

    /// The session token slot. The account sets it after a sign-in and clears it on sign-out.
    #[must_use]
    pub const fn auth(&self) -> &Auth {
        &self.auth
    }

    /// The tracks the catalog has mapped, by page address.
    #[must_use]
    pub const fn records(&self) -> &TrackRecords {
        &self.records
    }

    /// Look the client id up now, so the first search or play does not pay for it. A no-op when
    /// the id is known.
    ///
    /// # Errors
    /// [`ClientError`] when the home page or its scripts cannot be read.
    pub async fn warm_up(&self) -> Result<(), ClientError> {
        self.client_id().await.map(|_| ())
    }

    /// `GET <api><path>?<params>&client_id=…` and the body of a 200 answer.
    ///
    /// # Errors
    /// [`ClientError`] for a 429 ([`ClientError::RateLimited`], also while the backoff lasts), a
    /// 404 or 410, a refusal, a network failure or a missing client id.
    pub async fn get(&self, path: &str, params: &[(&str, &str)]) -> Result<Vec<u8>, ClientError> {
        let _pending = Pending::new(&self.interactive);
        self.notify_activity();
        self.fetch(path, params).await
    }

    /// Like [`ScClient::get`] for a hint nobody waits for (warming the next track): it starts
    /// only when no listener's call is pending.
    ///
    /// # Errors
    /// As [`ScClient::get`].
    pub async fn get_background(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<Vec<u8>, ClientError> {
        while self.interactive.load(Ordering::SeqCst) > 0 {
            tokio::time::sleep(BACKGROUND_POLL).await;
        }
        self.fetch(path, params).await
    }

    /// Set the callback told when a listener's call starts.
    pub fn set_activity(&self, activity: Activity) {
        *self.activity.lock().unwrap_or_else(PoisonError::into_inner) = Some(activity);
    }

    fn notify_activity(&self) {
        let hook = self
            .activity
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(hook) = hook {
            hook();
        }
    }

    /// The call with the session token, when signed in. A rejected session ends here (see
    /// [`ScClient::get_as`]).
    async fn fetch(&self, path: &str, params: &[(&str, &str)]) -> Result<Vec<u8>, ClientError> {
        let session = self.auth.current();
        let token = session.as_ref().map(|(token, _)| token);
        let result = self.get_as(path, params, token).await;
        if let (Err(ClientError::SessionEnded), Some((_, epoch))) = (&result, &session) {
            self.auth.reject(*epoch);
        }
        result
    }

    /// Like [`ScClient::get`] with an explicit token (`None` sends none). The sign-in uses it to
    /// check a token before keeping it. A 401 or 403 makes the client look the id up again once
    /// and repeat the call once. With a token, a second 401 means SoundCloud rejected the session
    /// ([`ClientError::SessionEnded`]); nothing is retried after that, so a dead token costs one
    /// extra request, not a storm.
    ///
    /// # Errors
    /// As [`ScClient::get`]; this call never changes the stored session.
    pub async fn get_as(
        &self,
        path: &str,
        params: &[(&str, &str)],
        token: Option<&Secret<String>>,
    ) -> Result<Vec<u8>, ClientError> {
        let mut refreshed = false;
        loop {
            self.check_blocked()?;
            let id = self.client_id().await?;
            let url = self.api_url(path, params, &id)?;
            let answer = self.request(url, MAX_JSON_BYTES, token).await?;
            match answer.status {
                200 => return Ok(answer.body),
                401 | 403 if !refreshed => {
                    refreshed = true;
                    self.forget_id(&id).await;
                }
                401 if token.is_some() => return Err(ClientError::SessionEnded),
                404 | 410 => return Err(ClientError::NotFound),
                _ => return Err(ClientError::Refused),
            }
        }
    }

    /// `GET <url>?<params>&client_id=…` for an address the API gave (a transcoding address). The
    /// address must start with the API base, so the client id never goes to another host.
    ///
    /// # Errors
    /// [`ClientError::Refused`] for an address on another host, otherwise as [`ScClient::get`].
    pub async fn get_api_url(
        &self,
        url: &str,
        params: &[(&str, &str)],
    ) -> Result<Vec<u8>, ClientError> {
        let path = self.api_path(url)?;
        self.get(path, params).await
    }

    /// [`ScClient::get_api_url`] for a hint nobody waits for.
    ///
    /// # Errors
    /// As [`ScClient::get_api_url`].
    pub async fn get_api_url_background(
        &self,
        url: &str,
        params: &[(&str, &str)],
    ) -> Result<Vec<u8>, ClientError> {
        let path = self.api_path(url)?;
        self.get_background(path, params).await
    }

    fn api_path<'a>(&self, url: &'a str) -> Result<&'a str, ClientError> {
        url.strip_prefix(self.endpoints.api.as_str())
            .filter(|rest| rest.starts_with('/'))
            .ok_or(ClientError::Refused)
    }

    fn api_url(
        &self,
        path: &str,
        params: &[(&str, &str)],
        id: &Secret<String>,
    ) -> Result<String, ClientError> {
        let base = format!("{}{path}", self.endpoints.api);
        let with_id = params
            .iter()
            .copied()
            .chain([("client_id", id.expose().as_str())]);
        reqwest::Url::parse_with_params(&base, with_id)
            .map(String::from)
            .map_err(|_| ClientError::Parse)
    }

    fn check_blocked(&self) -> Result<(), ClientError> {
        let until = *self
            .blocked_until
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match until {
            Some(t) if Instant::now() < t => Err(ClientError::RateLimited),
            _ => Ok(()),
        }
    }

    fn block(&self, retry_after: Option<Duration>) {
        let wait = retry_after
            .unwrap_or(self.pacing.default_backoff)
            .min(self.pacing.max_backoff);
        *self
            .blocked_until
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(Instant::now() + wait);
    }

    /// One paced request, one at a time. A 429 starts the backoff and ends the call; a 5xx or a
    /// connection error is tried again after the pacing delays, then fails with `Network`.
    async fn request(
        &self,
        url: String,
        max_bytes: usize,
        token: Option<&Secret<String>>,
    ) -> Result<ScAnswer, ClientError> {
        let mut gate = self.gate.lock().await;
        let mut retries = self.pacing.retry_delays.iter();
        loop {
            self.check_blocked()?;
            if !self.is_enabled() {
                return Err(ClientError::Off);
            }
            if let Some(last) = *gate {
                tokio::time::sleep_until((last + self.pacing.min_gap).into()).await;
            }
            *gate = Some(Instant::now());
            let sent = self
                .http
                .send(ScRequest {
                    url: url.clone(),
                    max_bytes,
                    authorization: token.cloned(),
                })
                .await;
            let failure = match sent {
                Ok(answer) if answer.status == 429 => {
                    self.block(answer.retry_after);
                    return Err(ClientError::RateLimited);
                }
                Ok(answer) if answer.status < 500 => return Ok(answer),
                Ok(_) | Err(ScHttpError::Failed) => ClientError::Network,
                Err(e) => return Err(e.into()),
            };
            match retries.next() {
                Some(delay) => tokio::time::sleep(*delay).await,
                None => return Err(failure),
            }
        }
    }

    async fn client_id(&self) -> Result<Secret<String>, ClientError> {
        let mut slot = self.id.lock().await;
        if let Some(id) = slot.as_ref() {
            return Ok(id.clone());
        }
        let id = self.discover().await?;
        *slot = Some(id.clone());
        Ok(id)
    }

    /// Drop `stale` unless another caller has already replaced it.
    async fn forget_id(&self, stale: &Secret<String>) {
        let mut slot = self.id.lock().await;
        if slot.as_ref() == Some(stale) {
            *slot = None;
        }
    }

    /// Read the home page, then its scripts from last to first, until one holds the id.
    async fn discover(&self) -> Result<Secret<String>, ClientError> {
        let page = self
            .request(format!("{}/", self.endpoints.site), MAX_JSON_BYTES, None)
            .await?;
        if page.status != 200 {
            return Err(ClientError::Refused);
        }
        let html = String::from_utf8_lossy(&page.body);
        let scripts = script_urls(&html, &self.endpoints.script_prefix);
        for url in scripts.into_iter().rev().take(MAX_SCRIPTS) {
            let script = self.request(url, MAX_SCRIPT_BYTES, None).await?;
            if script.status != 200 {
                continue;
            }
            if let Some(id) = find_client_id(&String::from_utf8_lossy(&script.body)) {
                return Ok(Secret::new(id));
            }
        }
        Err(ClientError::NoClientId)
    }
}

#[cfg(test)]
mod tests;
