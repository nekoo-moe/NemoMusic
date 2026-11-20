//! The `DevProbe` the app gives the hidden developer panel (dev-tools spec).
//!
//! It reads the capture ring, aggregates it into rows and cards, and gathers the facts the other
//! crates already expose. A fact that has no status call stays `Unknown`. Stored sessions are
//! reduced inside their source closure to a yes or no, a size class and cookie names, so no
//! value ever reaches the snapshot.

mod aggregate;
mod facts;
mod wiring;

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use nemo_core::dev::{DevAccount, DevPathKind, DevProbe, DevSnapshot, DevSolverState, DevSystem};
use serde_json::Value;

use crate::dev_capture::{DevRing, Redactor};
pub(crate) use facts::SecretFacts;
pub(crate) use wiring::for_app;

/// The most trace rows in a snapshot.
const TRACE_ROWS: usize = 1000;

/// The most log lines in a snapshot.
const LOG_LINES: usize = 1000;

/// How long config and credential-store facts are kept before they are read again.
const FACT_TTL: Duration = Duration::from_secs(5);

/// A source of one fact.
type Source<T> = Box<dyn Fn() -> T + Send + Sync>;

/// Where the probe reads the facts that the ring does not hold.
pub(crate) struct Sources {
    /// Whether the YouTube Music account is signed in.
    pub(crate) ytm_signed_in: Source<Option<bool>>,
    /// Whether the SoundCloud account is signed in, or `None` when SoundCloud is off.
    pub(crate) sc_signed_in: Source<Option<bool>>,
    /// The stored YouTube Music session, reduced to facts.
    pub(crate) ytm_secret: Source<SecretFacts>,
    /// The stored SoundCloud token, reduced to facts.
    pub(crate) sc_secret: Source<SecretFacts>,
    /// The output device and format of the current signal path.
    pub(crate) audio: Source<Option<String>>,
    /// The current settings as JSON.
    pub(crate) config_now: Source<Value>,
    /// The default settings as JSON.
    pub(crate) config_default: Value,
    /// The in-process solver state.
    pub(crate) solver: Source<DevSolverState>,
    /// Where `yt-dlp` comes from.
    pub(crate) ytdlp_path: Source<DevPathKind>,
    /// A short prefix of the managed `yt-dlp` build hash.
    pub(crate) ytdlp_build: Source<Option<String>>,
}

/// A value kept for [`FACT_TTL`].
struct Ttl<T> {
    slot: Mutex<Option<(Instant, T)>>,
}

impl<T: Clone> Ttl<T> {
    fn new() -> Self {
        Self {
            slot: Mutex::new(None),
        }
    }

    fn get(&self, make: impl FnOnce() -> T) -> T {
        let mut slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((at, value)) = slot.as_ref()
            && at.elapsed() < FACT_TTL
        {
            return value.clone();
        }
        let value = make();
        *slot = Some((Instant::now(), value.clone()));
        value
    }
}

/// The probe over the ring and the sources.
pub(crate) struct AppProbe {
    ring: DevRing,
    redactor: Redactor,
    sources: Sources,
    clock: fn() -> u64,
    secrets: Ttl<(SecretFacts, SecretFacts)>,
    config: Ttl<Value>,
    build: Ttl<Option<String>>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| u64::try_from(d.as_millis()).ok())
        .unwrap_or(0)
}

impl AppProbe {
    /// A probe over `ring` that reads its facts from `sources`.
    pub(crate) fn new(ring: DevRing, sources: Sources) -> Self {
        Self::with_parts(ring, Redactor::new(), sources, now_ms)
    }

    /// A probe with its own redactor and clock (tests).
    pub(crate) fn with_parts(
        ring: DevRing,
        redactor: Redactor,
        sources: Sources,
        clock: fn() -> u64,
    ) -> Self {
        Self {
            ring,
            redactor,
            sources,
            clock,
            secrets: Ttl::new(),
            config: Ttl::new(),
            build: Ttl::new(),
        }
    }

    fn accounts(&self, events: &[crate::dev_capture::CapturedEvent]) -> Vec<DevAccount> {
        let (ytm, sc) = self
            .secrets
            .get(|| ((self.sources.ytm_secret)(), (self.sources.sc_secret)()));
        let mut ytm_account = account("YouTube Music", (self.sources.ytm_signed_in)(), ytm);
        aggregate::last_signin(events, "ytm", &mut ytm_account);
        let mut sc_account = account("SoundCloud", (self.sources.sc_signed_in)(), sc);
        aggregate::last_signin(events, "soundcloud", &mut sc_account);
        vec![ytm_account, sc_account]
    }

    fn system(&self) -> DevSystem {
        let now = self.config.get(|| (self.sources.config_now)());
        let mut hosts = self.ring.hosts();
        hosts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        DevSystem {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            profile: if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
            .to_owned(),
            config: facts::set_keys(&now, &self.sources.config_default, &self.redactor),
            egress: hosts
                .into_iter()
                .map(|(host, count)| nemo_core::dev::DevEgress { host, count })
                .collect(),
            uptime_s: self.ring.uptime().as_secs(),
            audio: (self.sources.audio)().map(|text| self.redactor.redact(&text)),
        }
    }
}

/// The account facts of one site.
fn account(site: &str, signed_in: Option<bool>, secret: SecretFacts) -> DevAccount {
    DevAccount {
        site: site.to_owned(),
        signed_in,
        token_present: secret.present,
        token_len: secret.len,
        cookie_names: secret.cookie_names,
        ..DevAccount::default()
    }
}

impl DevProbe for AppProbe {
    fn snapshot(&self) -> DevSnapshot {
        let events = self.ring.snapshot();
        let mut resolver = aggregate::resolver(&events);
        resolver.solver = (self.sources.solver)();
        resolver.ytdlp_path = (self.sources.ytdlp_path)();
        resolver.ytdlp_build = self.build.get(|| (self.sources.ytdlp_build)());
        DevSnapshot {
            generation: self.ring.generation(),
            taken_ms: (self.clock)(),
            trace: aggregate::trace_rows(&events, TRACE_ROWS),
            log: aggregate::log_lines(&events, LOG_LINES),
            providers: aggregate::providers(&events),
            resolver,
            accounts: self.accounts(&events),
            system: self.system(),
        }
    }

    fn clear(&self) {
        self.ring.clear();
    }

    fn redact(&self, text: &str) -> String {
        self.redactor.redact(text)
    }
}

/// Sources that know nothing, so every fact shows `Unknown`.
#[cfg(test)]
pub(crate) fn unknown_sources() -> Sources {
    Sources {
        ytm_signed_in: Box::new(|| None),
        sc_signed_in: Box::new(|| None),
        ytm_secret: Box::new(SecretFacts::default),
        sc_secret: Box::new(SecretFacts::default),
        audio: Box::new(|| None),
        config_now: Box::new(|| Value::Null),
        config_default: Value::Null,
        solver: Box::new(|| DevSolverState::Unknown),
        ytdlp_path: Box::new(|| DevPathKind::Unknown),
        ytdlp_build: Box::new(|| None),
    }
}

#[cfg(test)]
mod tests;
