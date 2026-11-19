//! The player script: which one is current, its signature timestamp and what the solver made of
//! it. It is downloaded once (about 2.7 MB), solved into a much smaller script, and kept in
//! memory and in a private file, so a later start needs no download and no parse.
//!
//! The current player is read from `https://www.youtube.com/iframe_api` (what `yt-dlp` does) and
//! checked again after [`PLAYER_CHECK_EVERY`]. The preparation runs as its own task on the app
//! runtime: a caller that is dropped (a lost race against `yt-dlp`) does not stop it, so the
//! first song's work is never wasted.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use nemo_core::BoxFuture;
use nemo_core::stream::StreamError;
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;

use super::solver::{ChallengeSolver, Challenges, Script};
use crate::http::{InitError, https_redirects};

/// How long a player id is trusted before `iframe_api` is read again.
pub const PLAYER_CHECK_EVERY: Duration = Duration::from_hours(6);
/// Longest time one request for a player script may take.
const FETCH_TIMEOUT: Duration = Duration::from_secs(30);
/// Largest player script read, in bytes.
const MAX_SCRIPT_BYTES: usize = 8 * 1024 * 1024;
/// Time limit of the one-time solve that preprocesses a whole player script.
const PREPARE_LIMIT: Duration = Duration::from_secs(40);
/// Least time between two forced refreshes ([`PlayerStore::refresh`]).
const REFRESH_EVERY: Duration = Duration::from_mins(10);
/// Format version of the cache file.
const VERSION: u32 = 1;

/// A prepared player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerInfo {
    /// The player's 8-character id.
    pub id: String,
    /// Its signature timestamp, sent with the `player` request.
    pub sts: u32,
    /// The script after the solver's preprocessing.
    pub preprocessed: Arc<String>,
}

/// Where the player comes from. [`HttpPlayerSource`] is the real one; tests give a fake.
pub trait PlayerSource: Send + Sync {
    /// The id of the current player.
    fn latest_id(&self) -> BoxFuture<'_, Result<String, StreamError>>;
    /// The script of player `id`.
    fn script<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<String, StreamError>>;
}

/// The real source: `iframe_api` and the player script, both from `www.youtube.com`.
#[derive(Debug, Clone)]
pub struct HttpPlayerSource {
    client: reqwest::Client,
}

impl HttpPlayerSource {
    /// Build the HTTP client.
    ///
    /// # Errors
    /// [`InitError::Client`] when it cannot be built.
    pub fn new() -> Result<Self, InitError> {
        let client = reqwest::Client::builder()
            .connect_timeout(FETCH_TIMEOUT)
            .timeout(FETCH_TIMEOUT)
            .redirect(https_redirects())
            .https_only(true)
            .build()
            .map_err(|_| InitError::Client)?;
        Ok(Self { client })
    }

    async fn get(&self, url: String) -> Result<String, StreamError> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| StreamError::Network("player request failed".to_owned()))?;
        if !response.status().is_success() {
            return Err(StreamError::Network(format!(
                "player request: HTTP {}",
                response.status().as_u16()
            )));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_| StreamError::Network("player answer was cut off".to_owned()))?;
        if bytes.len() > MAX_SCRIPT_BYTES {
            return Err(StreamError::Parse("player script too large".to_owned()));
        }
        String::from_utf8(bytes.to_vec()).map_err(|_| StreamError::Parse("not UTF-8".to_owned()))
    }
}

impl PlayerSource for HttpPlayerSource {
    fn latest_id(&self) -> BoxFuture<'_, Result<String, StreamError>> {
        Box::pin(async move {
            let text = self
                .get("https://www.youtube.com/iframe_api".to_owned())
                .await?;
            player_id_in(&text).ok_or_else(|| StreamError::Parse("no player id".to_owned()))
        })
    }

    fn script<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<String, StreamError>> {
        Box::pin(async move {
            // The id is checked, so it cannot change the address.
            if !is_player_id(id) {
                return Err(StreamError::Parse("bad player id".to_owned()));
            }
            self.get(format!(
                "https://www.youtube.com/s/player/{id}/player_ias.vflset/en_US/base.js"
            ))
            .await
        })
    }
}

fn is_player_id(id: &str) -> bool {
    id.len() == 8 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The id in `.../player/<id>/...` (`yt-dlp`: `player\\?/([0-9a-fA-F]{8})\\?/`).
#[must_use]
pub(super) fn player_id_in(text: &str) -> Option<String> {
    let mut rest = text;
    while let Some(at) = rest.find("player") {
        rest = &rest[at + "player".len()..];
        let after = rest.trim_start_matches('\\');
        if let Some(tail) = after.strip_prefix('/') {
            let id: String = tail.chars().take(8).collect();
            let next = tail.chars().nth(8);
            if is_player_id(&id) && matches!(next, Some('/' | '\\')) {
                return Some(id.to_ascii_lowercase());
            }
        }
    }
    None
}

/// The signature timestamp in a player script (`yt-dlp`:
/// `(?:signatureTimestamp|sts)\s*:\s*([0-9]{5})`).
#[must_use]
pub(super) fn sts_in(script: &str) -> Option<u32> {
    for key in ["signatureTimestamp", "sts"] {
        let mut from = 0;
        while let Some(at) = script[from..].find(key) {
            let tail = script[from + at + key.len()..].trim_start();
            if let Some(tail) = tail.strip_prefix(':') {
                let digits: String = tail
                    .trim_start()
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect();
                if digits.len() == 5 {
                    return digits.parse().ok();
                }
            }
            from += at + key.len();
        }
    }
    None
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheFile {
    version: u32,
    id: String,
    sts: u32,
    /// Seconds since the Unix epoch when the id was last confirmed current.
    checked: u64,
    preprocessed: String,
}

struct State {
    current: Option<(PlayerInfo, Instant)>,
    /// A stream built from this player was refused: read the player id again before trusting it.
    must_check: bool,
    refreshed: Option<Instant>,
}

/// The current prepared player, kept fresh.
pub struct PlayerStore {
    source: Arc<dyn PlayerSource>,
    solver: Arc<dyn ChallengeSolver>,
    handle: Handle,
    file: Option<PathBuf>,
    state: Mutex<State>,
    /// One preparation at a time.
    gate: tokio::sync::Mutex<()>,
}

impl PlayerStore {
    /// A store that keeps its file at `file` (`None`: memory only).
    #[must_use]
    pub fn new(
        source: Arc<dyn PlayerSource>,
        solver: Arc<dyn ChallengeSolver>,
        handle: Handle,
        file: Option<PathBuf>,
    ) -> Arc<Self> {
        Arc::new(Self {
            source,
            solver,
            handle,
            file,
            state: Mutex::new(State {
                current: None,
                must_check: false,
                refreshed: None,
            }),
            gate: tokio::sync::Mutex::new(()),
        })
    }

    /// `true` when a cache file exists, so [`get`](Self::get) can answer without a download.
    #[must_use]
    pub fn has_file(&self) -> bool {
        self.file.as_deref().is_some_and(Path::is_file)
    }

    /// The prepared current player. The work runs as a task of its own: dropping the returned
    /// future does not stop it.
    ///
    /// # Errors
    /// [`StreamError::Network`] or [`StreamError::Parse`] when no player can be got and none is
    /// kept.
    pub async fn get(self: &Arc<Self>) -> Result<PlayerInfo, StreamError> {
        if let Some(info) = self.fresh() {
            return Ok(info);
        }
        let store = Arc::clone(self);
        self.handle
            .spawn(async move { store.prepare().await })
            .await
            .unwrap_or_else(|_| Err(StreamError::Network("player task ended".to_owned())))
    }

    fn fresh(&self) -> Option<PlayerInfo> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state
            .current
            .as_ref()
            .filter(|(_, checked)| !state.must_check && checked.elapsed() < PLAYER_CHECK_EVERY)
            .map(|(info, _)| info.clone())
    }

    fn kept(&self) -> Option<PlayerInfo> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.current.as_ref().map(|(info, _)| info.clone())
    }

    async fn prepare(self: Arc<Self>) -> Result<PlayerInfo, StreamError> {
        let _one_at_a_time = self.gate.lock().await;
        if let Some(info) = self.fresh() {
            return Ok(info);
        }
        // A refused address means the id is read again, whatever the file says.
        if !self.must_check()
            && let Some(info) = self.read_file()
        {
            return Ok(info);
        }
        let id = match self.source.latest_id().await {
            Ok(id) => id,
            // The kept player stays usable when the id cannot be read now.
            Err(e) => return self.kept().ok_or(e),
        };
        if let Some(info) = self.kept().filter(|k| k.id == id) {
            self.remember(info.clone());
            self.write_file(&info);
            return Ok(info);
        }
        let script = self.source.script(&id).await?;
        let sts = sts_in(&script).ok_or_else(|| StreamError::Parse("no timestamp".to_owned()))?;
        let engine = Arc::clone(&self.solver);
        let named = id.clone();
        let done = tokio::task::spawn_blocking(move || {
            let whole = Script::Whole {
                id: &named,
                text: &script,
            };
            engine.solve(whole, &Challenges::default(), PREPARE_LIMIT)
        })
        .await
        .map_err(|_| StreamError::Network("solver task ended".to_owned()))?
        .map_err(|e| StreamError::Parse(e.to_string()))?;
        let preprocessed = done
            .preprocessed
            .ok_or_else(|| StreamError::Parse("no preprocessed script".to_owned()))?;
        let info = PlayerInfo {
            id,
            sts,
            preprocessed: Arc::new(preprocessed),
        };
        self.remember(info.clone());
        self.write_file(&info);
        Ok(info)
    }

    fn remember(&self, info: PlayerInfo) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.current = Some((info, Instant::now()));
        state.must_check = false;
    }

    fn must_check(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .must_check
    }

    /// The player of the cache file, when it was confirmed current within the check interval.
    fn read_file(&self) -> Option<PlayerInfo> {
        let text = std::fs::read_to_string(self.file.as_ref()?).ok()?;
        let file: CacheFile = serde_json::from_str(&text).ok()?;
        let age = SystemTime::now()
            .duration_since(UNIX_EPOCH.checked_add(Duration::from_secs(file.checked))?)
            .ok()?;
        if file.version != VERSION || !is_player_id(&file.id) {
            return None;
        }
        let info = PlayerInfo {
            id: file.id,
            sts: file.sts,
            preprocessed: Arc::new(file.preprocessed),
        };
        if age >= PLAYER_CHECK_EVERY {
            // Old, but a fallback for when the network is not there; the id is checked again.
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            state.current = Instant::now()
                .checked_sub(PLAYER_CHECK_EVERY)
                .map(|checked| (info, checked));
            return None;
        }
        self.remember(info.clone());
        Some(info)
    }

    fn write_file(&self, info: &PlayerInfo) {
        let Some(path) = &self.file else { return };
        let file = CacheFile {
            version: VERSION,
            id: info.id.clone(),
            sts: info.sts,
            checked: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
            preprocessed: info.preprocessed.as_str().to_owned(),
        };
        if write_private(path, &file).is_err() {
            tracing::warn!("the player cache file could not be written");
        }
    }

    /// A stream built from this player could not be opened (a CDN 403): the table may be stale.
    /// The next [`get`](Self::get) reads the current player id again, and prepares the new
    /// player when the id changed; the same id keeps the player it has (a refused address is
    /// often the address's own fault, not the script's). At most once in ten minutes.
    pub fn refresh(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state
            .refreshed
            .is_some_and(|at| at.elapsed() < REFRESH_EVERY)
        {
            return;
        }
        state.refreshed = Some(Instant::now());
        state.must_check = true;
    }

    /// The solver could not use this player (no function found, the script failed): drop it and
    /// its file, so the next [`get`](Self::get) downloads and prepares the player again. At most
    /// once in ten minutes, so a script the solver cannot read does not cause a download per click.
    pub fn rebuild(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state
            .refreshed
            .is_some_and(|at| at.elapsed() < REFRESH_EVERY)
        {
            return;
        }
        state.refreshed = Some(Instant::now());
        state.current = None;
        state.must_check = false;
        drop(state);
        if let Some(path) = &self.file {
            // Already gone is the same as deleted.
            let _ = std::fs::remove_file(path);
        }
    }

    /// Sign-out and cache wipes: forget the player and delete its file.
    pub fn wipe(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.current = None;
        state.must_check = false;
        state.refreshed = None;
        drop(state);
        if let Some(path) = &self.file {
            // Already gone is the same as deleted.
            let _ = std::fs::remove_file(path);
        }
    }
}

fn write_private(path: &Path, file: &CacheFile) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut out = options.open(&tmp)?;
    out.write_all(&serde_json::to_vec(file).map_err(std::io::Error::other)?)?;
    out.sync_data()?;
    drop(out);
    std::fs::rename(&tmp, path)
}
