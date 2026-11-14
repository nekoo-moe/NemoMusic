//! Service contracts that connect the UI, the player and the network crates.
//!
//! `nemo-app` supplies the real implementations. Every async method is runtime-agnostic: the
//! implementation runs its work on its own runtime and returns a future that any executor can
//! poll.

use std::io::{Read, Seek};
use std::sync::Arc;
use std::time::Duration;

use crate::BoxFuture;
use crate::account::AccountService;
use crate::catalog::Catalog;
use crate::integrations::IntegrationsService;
use crate::lyrics::{
    Granularity, LyricsCandidate, LyricsCredit, LyricsDoc, LyricsQuery, ProviderId,
};
use crate::lyrics_plus::LineMeta;
use crate::notify::OsNotifier;
use crate::player::{PlayerCommand, PlayerEvent, PlayerSnapshot};
use crate::stats::StatsService;
use crate::stream::{ResolvedStream, StreamError};
use crate::track::Artwork;

/// A stream of bytes, for example an HTTP resource read with range requests. Most sources seek;
/// a live stream reports [`is_seekable`](Self::is_seekable) `false`.
/// Reads may block: the decode thread calls them, never the audio callback.
pub trait ByteSource: Read + Seek + Send + Sync {
    /// Total size in bytes, if the source knows it.
    fn byte_len(&self) -> Option<u64>;

    /// Whether the decoder may seek this source. `true` unless the source says otherwise; a live
    /// stream returns `false` so it is never seeked.
    fn is_seekable(&self) -> bool {
        true
    }
}

/// Opens a resolved stream for reading.
pub trait StreamOpener: Send + Sync {
    /// Open the stream. This call may block; call it from a worker thread.
    ///
    /// # Errors
    /// Returns a [`StreamError`] when the stream cannot be opened.
    fn open(&self, stream: &ResolvedStream) -> Result<Box<dyn ByteSource>, StreamError>;
}

/// Receives player events without blocking. The UI polls it once per frame.
pub trait EventReceiver: Send {
    /// The next event, or `None` when there is none right now.
    fn try_next(&mut self) -> Option<PlayerEvent>;
}

/// Control and read access to the player.
pub trait PlayerControl: Send + Sync {
    /// Send a command. Never blocks.
    fn send(&self, command: PlayerCommand);

    /// Position in the current track at the speakers: frames written minus output latency.
    /// A cheap atomic read, safe to call every frame.
    fn position(&self) -> Duration;

    /// The player state now.
    fn snapshot(&self) -> PlayerSnapshot;

    /// A new event receiver. Each receiver gets every event from the moment it is created.
    fn subscribe(&self) -> Box<dyn EventReceiver>;
}

/// The result of a lyrics lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsLookup {
    /// The best lyrics found, if any.
    pub doc: Option<LyricsDoc>,
    /// Providers that were asked, in chain order. Shown when nothing was found.
    pub tried: Vec<ProviderId>,
}

/// Finds lyrics through the provider chain and keeps per-track offsets.
pub trait LyricsService: Send + Sync {
    /// Look up lyrics. Failures of single providers do not fail the lookup.
    fn lookup<'a>(&'a self, query: &'a LyricsQuery) -> BoxFuture<'a, LyricsLookup>;

    /// The user's timing offset for a track, in milliseconds (0 when none is stored).
    fn offset_ms(&self, track_key: &str) -> i64;

    /// Store the user's timing offset for a track.
    fn set_offset_ms(&self, track_key: &str, offset_ms: i64);

    /// Every provider's result for the track, best first (granularity, then score, then chain
    /// order). Use it for the "rotate providers" picker. Providers that have nothing do not
    /// appear. The default wraps [`lookup`](Self::lookup) into at most one candidate.
    fn lookup_all<'a>(&'a self, query: &'a LyricsQuery) -> BoxFuture<'a, Vec<LyricsCandidate>> {
        Box::pin(async move {
            self.lookup(query)
                .await
                .doc
                .map(|doc| LyricsCandidate {
                    provider: doc.provider,
                    granularity: doc.granularity,
                    score: 100,
                    source_note: None,
                    credit: None,
                    doc,
                })
                .into_iter()
                .collect()
        })
    }

    /// Remember the user's pick of provider for a track. [`lookup`](Self::lookup) returns that
    /// provider's lyrics first from now on. The default does nothing.
    fn choose(&self, _track_key: &str, _provider: ProviderId) {}

    /// The remembered pick for a track, if any. The default has none.
    fn chosen(&self, _track_key: &str) -> Option<ProviderId> {
        None
    }

    /// Forget the remembered pick, so the best result wins again. The default does nothing.
    fn clear_choice(&self, _track_key: &str) {}

    /// The credit the source of the lyrics asks for (Unison must be credited). `None` when the
    /// source asks for none or nothing is cached. Reads the local cache only.
    fn credit(&self, _track_key: &str, _provider: ProviderId) -> Option<LyricsCredit> {
        None
    }

    /// The providers in chain order with their on/off state. The default lists none.
    fn providers(&self) -> Vec<LyricsProviderStatus> {
        Vec::new()
    }

    /// Switch a provider on or off; the choice is stored. Returns false for an unknown id.
    /// The default does nothing.
    fn set_provider_enabled(&self, _provider: ProviderId, _enabled: bool) -> bool {
        false
    }

    /// Per-line side data (singer, background vocals, provider romanization) of the cached
    /// lyrics of `provider` for a track, same index as `LyricsDoc.lines`. Empty when the source
    /// gave none or nothing is cached. Reads the local cache only. The default is empty.
    fn line_meta(&self, _track_key: &str, _provider: ProviderId) -> Vec<LineMeta> {
        Vec::new()
    }

    /// Keep the cached lyrics of a track from expiring (`keep` true) or let them expire again
    /// (`keep` false). The offline cache calls it for every saved song. The default does nothing.
    fn keep_offline(&self, _track_key: &str, _keep: bool) {}
}

/// A lyrics provider as the settings screen shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsProviderStatus {
    /// Name shown in the UI.
    pub id: ProviderId,
    /// Whether it is asked now.
    pub enabled: bool,
    /// Whether it is on before the user changes anything.
    pub default_enabled: bool,
    /// One-line risk note for opt-in sources.
    pub risk_note: Option<&'static str>,
    /// Hosts it contacts.
    pub hosts: &'static [&'static str],
    /// The finest timing this provider can give (a single track may get less). The settings
    /// screen shows it as a tag and sorts by it.
    pub best: Granularity,
}

/// Loading cover art failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImageError {
    /// There is no artwork to load.
    #[error("no artwork")]
    Missing,
    /// The network request failed.
    #[error("network error: {0}")]
    Network(String),
    /// A local file could not be read.
    #[error("could not read artwork: {0}")]
    Read(String),
}

/// Loads cover art bytes (JPEG, PNG or WebP). Implementations cache on disk.
pub trait ImageSource: Send + Sync {
    /// Load the image for a slot `target_px` wide, picking the size with
    /// [`pick_thumbnail`](crate::track::pick_thumbnail) so a cover is never upscaled.
    fn image<'a>(
        &'a self,
        artwork: &'a Artwork,
        target_px: u32,
    ) -> BoxFuture<'a, Result<Vec<u8>, ImageError>>;
}

/// Everything the UI needs from the rest of the app.
#[derive(Clone)]
pub struct Backend {
    /// Search and browse.
    pub catalog: Arc<dyn Catalog>,
    /// Playback control.
    pub player: Arc<dyn PlayerControl>,
    /// Lyrics.
    pub lyrics: Arc<dyn LyricsService>,
    /// Cover art.
    pub images: Arc<dyn ImageSource>,
    /// Google account sign-in.
    pub account: Arc<dyn AccountService>,
    /// OS media controls, Last.fm and Discord presence. [`NoIntegrations`](crate::integrations::NoIntegrations) until the app builds
    /// the real service.
    pub integrations: Arc<dyn IntegrationsService>,
    /// OS notifications. [`NoNotifier`](crate::notify::NoNotifier) until the app builds the real
    /// notifier.
    pub notifier: Arc<dyn OsNotifier>,
    /// The SoundCloud catalog of public tracks. `None` until the app builds it (the opt-in
    /// switch is on and confirmed), so the UI shows no SoundCloud entry.
    pub soundcloud: Option<Arc<dyn Catalog>>,
    /// Listening stats. [`NoStats`](crate::stats::NoStats) while recording is off or the store
    /// could not open.
    pub stats: Arc<dyn StatsService>,
}
