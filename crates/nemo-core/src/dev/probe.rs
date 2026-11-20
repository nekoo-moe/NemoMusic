//! The read-only contract between the app and the hidden developer panel.
//!
//! Every type holds plain numbers, enums and short texts that the app has already redacted. A
//! token, a cookie or an account value has no field here: the account facts are a yes or no, a
//! length class and the names of cookies.

/// What the panel reads. The app implements it over the capture ring and the status calls of the
/// other crates; `None` or [`DevLenClass::Unknown`] means the app has no status call for a fact.
pub trait DevProbe: Send + Sync {
    /// Everything the panel shows, now. Cheap enough to call once a second.
    fn snapshot(&self) -> DevSnapshot;

    /// Empty the in-memory ring of events. Changes nothing else.
    fn clear(&self);

    /// The capture-time redaction pass, applied to the report text a second time.
    fn redact(&self, text: &str) -> String;
}

/// Everything the developer panel shows at one moment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DevSnapshot {
    /// A number that changes whenever the event ring changes.
    pub generation: u64,
    /// When the snapshot was taken, milliseconds since the Unix epoch.
    pub taken_ms: u64,
    /// Provider requests, newest first.
    pub trace: Vec<DevTraceRow>,
    /// Log lines, newest first.
    pub log: Vec<DevLogLine>,
    /// One card per provider seen so far.
    pub providers: Vec<DevProvider>,
    /// The stream resolver, its tiers and caches.
    pub resolver: DevResolver,
    /// Sign-in diagnostics, one entry per site.
    pub accounts: Vec<DevAccount>,
    /// Version, OS, config, hosts and the audio output.
    pub system: DevSystem,
}

/// One provider request of the trace.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DevTraceRow {
    /// When it ended, milliseconds since the Unix epoch.
    pub at_ms: u64,
    /// `ytm`, `soundcloud`, `lyrics:<name>`, `sponsorblock`, `ytdlp`, `resolver`, and so on.
    pub provider: String,
    /// A fixed short operation name.
    pub op: String,
    /// The host only, or empty.
    pub host: String,
    /// An HTTP status or a fixed word (`ok`, `timeout`, `refused`, ...).
    pub status: String,
    /// How long it took.
    pub ms: Option<u64>,
    /// `native`, `in_process` or `yt_dlp` for a stream lookup.
    pub tier: Option<String>,
    /// Bytes read.
    pub bytes: Option<u64>,
    /// The try number.
    pub attempt: Option<u64>,
    /// Backoff or rest seconds that started with this event.
    pub rest_s: Option<u64>,
    /// Whether the call failed (a 4xx or 5xx status or an error word).
    pub failed: bool,
}

/// How loud a log line is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum DevLevel {
    /// A failure the user sees.
    Error,
    /// A recoverable problem.
    Warn,
    /// A lifecycle line.
    #[default]
    Info,
    /// Development detail.
    Debug,
    /// Finest detail.
    Trace,
}

/// One redacted log line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DevLogLine {
    /// When it was logged, milliseconds since the Unix epoch.
    pub at_ms: u64,
    /// Its level.
    pub level: DevLevel,
    /// The module path.
    pub target: String,
    /// The message and its allow-listed fields as `key=value` text.
    pub text: String,
}

/// The card of one provider.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DevProvider {
    /// The provider name, as in the trace.
    pub name: String,
    /// Calls seen (cache lookups and rest notes do not count).
    pub calls: u64,
    /// Failed calls seen.
    pub errors: u64,
    /// When the last call worked.
    pub last_ok_ms: Option<u64>,
    /// The status word or code of the last failed call.
    pub last_error_kind: Option<String>,
    /// The median time of the last 50 calls.
    pub p50_ms: Option<u64>,
    /// The 95th percentile time of the last 50 calls.
    pub p95_ms: Option<u64>,
    /// The provider rests or backs off until this time, if its last note said so.
    pub resting_until_ms: Option<u64>,
}

/// Where the `yt-dlp` program comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DevPathKind {
    /// The app has no status call for it.
    #[default]
    Unknown,
    /// No `yt-dlp` was found.
    Missing,
    /// The app's own copy.
    Managed,
    /// The path the user set.
    User,
    /// Found on the `PATH`.
    Path,
}

/// The in-process stream resolver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DevSolverState {
    /// The app has no status call for it.
    #[default]
    Unknown,
    /// Switched off in the settings.
    Disabled,
    /// It switched itself off for a day after its streams were refused.
    AutoOff,
    /// In use.
    Active,
}

/// How often one resolver tier answered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DevTier {
    /// `native`, `in_process` or `yt_dlp`.
    pub name: String,
    /// Streams it gave.
    pub ok: u64,
    /// Lookups that failed.
    pub failed: u64,
}

/// The stream resolver as the trace shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DevResolver {
    /// The tiers, in the order the chain tries them.
    pub tiers: Vec<DevTier>,
    /// The in-process solver.
    pub solver: DevSolverState,
    /// Lookups answered from memory.
    pub cache_hits: u64,
    /// Lookups answered from the file of an earlier run.
    pub cache_disk_hits: u64,
    /// Lookups that went to a tier.
    pub cache_misses: u64,
    /// Streams kept in memory, if the app can tell.
    pub cache_size: Option<u64>,
    /// Where `yt-dlp` comes from.
    pub ytdlp_path: DevPathKind,
    /// The `yt-dlp` version, if the app can tell.
    pub ytdlp_version: Option<String>,
    /// A short prefix of the managed copy's build hash, if there is one.
    pub ytdlp_build: Option<String>,
}

/// A size class of a stored secret. The length is all that is ever read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DevLenClass {
    /// The app could not look.
    #[default]
    Unknown,
    /// Nothing is stored.
    Absent,
    /// 1 to 31 characters.
    Short,
    /// 32 to 255 characters.
    Medium,
    /// 256 to 2047 characters.
    Long,
    /// 2048 characters or more.
    VeryLong,
}

impl DevLenClass {
    /// The class of a stored value of `len` characters; 0 is [`DevLenClass::Absent`].
    #[must_use]
    pub const fn of(len: usize) -> Self {
        match len {
            0 => Self::Absent,
            1..=31 => Self::Short,
            32..=255 => Self::Medium,
            256..=2047 => Self::Long,
            _ => Self::VeryLong,
        }
    }
}

/// Sign-in facts of one site. No token, cookie or account value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DevAccount {
    /// `YouTube Music` or `SoundCloud`.
    pub site: String,
    /// Whether an account is signed in.
    pub signed_in: Option<bool>,
    /// Whether a token or cookie is stored.
    pub token_present: Option<bool>,
    /// The size class of the stored value.
    pub token_len: DevLenClass,
    /// The names (never the values) of the stored cookies.
    pub cookie_names: Vec<String>,
    /// The last sign-in stage (`signin_window`, `signin_check`, `signin_store`).
    pub last_stage: Option<String>,
    /// How that stage ended.
    pub last_outcome: Option<String>,
    /// When it ended.
    pub last_at_ms: Option<u64>,
}

/// One config key that differs from its default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DevConfigKey {
    /// The key.
    pub key: String,
    /// The value for a plain, non-secret setting; `None` when the key is set but hidden.
    pub value: Option<String>,
}

/// How often a host was contacted this run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DevEgress {
    /// The host.
    pub host: String,
    /// The requests seen.
    pub count: u64,
}

/// Facts about this run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DevSystem {
    /// The app version.
    pub version: String,
    /// The operating system and CPU.
    pub os: String,
    /// `debug` or `release`.
    pub profile: String,
    /// The config keys that are set.
    pub config: Vec<DevConfigKey>,
    /// The hosts contacted, most requests first.
    pub egress: Vec<DevEgress>,
    /// Seconds since the app started.
    pub uptime_s: u64,
    /// The output device and format of the current signal path, if a song plays.
    pub audio: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_classes_have_fixed_edges() {
        assert_eq!(DevLenClass::of(0), DevLenClass::Absent);
        assert_eq!(DevLenClass::of(1), DevLenClass::Short);
        assert_eq!(DevLenClass::of(31), DevLenClass::Short);
        assert_eq!(DevLenClass::of(32), DevLenClass::Medium);
        assert_eq!(DevLenClass::of(255), DevLenClass::Medium);
        assert_eq!(DevLenClass::of(256), DevLenClass::Long);
        assert_eq!(DevLenClass::of(2047), DevLenClass::Long);
        assert_eq!(DevLenClass::of(2048), DevLenClass::VeryLong);
    }

    #[test]
    fn levels_order_from_error_to_trace() {
        assert!(DevLevel::Error < DevLevel::Warn);
        assert!(DevLevel::Debug < DevLevel::Trace);
    }

    #[test]
    fn an_empty_snapshot_shows_nothing_known() {
        let snap = DevSnapshot::default();
        assert!(snap.trace.is_empty() && snap.log.is_empty() && snap.providers.is_empty());
        assert_eq!(snap.resolver.solver, DevSolverState::Unknown);
        assert_eq!(snap.resolver.ytdlp_path, DevPathKind::Unknown);
    }
}
