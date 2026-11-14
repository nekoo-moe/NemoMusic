//! [`LyricsHub`]: the [`LyricsService`] the app wires in (ADR 0002).
//!
//! Lookup order: the user's remembered pick (cached), then the provider chain, then store.
//! - A provider with a fresh cache row (hit, miss or error) is not asked again; cached hits still
//!   compete in the final pick. The local provider is never cached (the files can change).
//! - After the chain runs, hits are stored for 30 days, misses and wrong matches for 7 days,
//!   errors for 15 minutes (or the provider's `Retry-After`). Timeouts are not stored.
//! - Offsets, the per-track provider pick and provider on/off choices live in SQLite. Offsets are
//!   clamped to +-10 s, as the ADR allows.
//! - [`LyricsService::lookup_all`] asks every enabled provider (local file included) and returns
//!   all acceptable answers for the picker.
//!
//! The service futures are runtime-agnostic: the work runs on the tokio [`Handle`] given at
//! construction and the returned future only awaits it.

use std::fmt::Write as _;
use std::path::Path;
use std::sync::{Arc, Mutex};

use nemo_core::BoxFuture;
use nemo_core::lyrics::{
    LyricsCandidate, LyricsCredit, LyricsError, LyricsProvider, LyricsQuery, ProviderId,
};
use nemo_core::services::{LyricsLookup, LyricsProviderStatus, LyricsService};
use sha2::{Digest, Sha256};
use tokio::runtime::Handle;

use crate::blapi::BetterLyricsProvider;
use crate::cache::{CacheError, LyricsCache};
use crate::chain::{ChainEntry, ChainOptions};
use crate::local::LocalProvider;
use crate::lrclib::LrclibProvider;
use crate::netease::NeteaseProvider;
use crate::normalize::normalize;
use crate::simpmusic::SimpMusicProvider;
use crate::unison::UnisonProvider;

use inner::Inner;

/// Largest stored offset in either direction, in milliseconds.
pub const MAX_OFFSET_MS: i64 = 10_000;

/// The hub could not be built.
#[derive(Debug, thiserror::Error)]
pub enum HubError {
    /// The cache database could not be opened.
    #[error(transparent)]
    Cache(#[from] CacheError),
    /// A built-in provider could not be created.
    #[error("could not create a lyrics provider: {0}")]
    Provider(#[from] LyricsError),
}

/// The key lyrics and offsets are stored under: the YouTube video id when there is one,
/// otherwise a hash of normalized artist, title, album and duration (whole seconds).
#[must_use]
pub fn track_key(query: &LyricsQuery) -> String {
    if let Some(id) = &query.ytm_video_id {
        return id.as_str().to_owned();
    }
    let artist = query
        .artists
        .first()
        .map(|a| normalize(a))
        .unwrap_or_default();
    let album = query.album.as_deref().map(normalize).unwrap_or_default();
    let secs = query.duration_ms.map_or(0, |ms| (ms + 500) / 1000);
    let mut hasher = Sha256::new();
    for part in [artist.as_str(), &normalize(&query.title), &album] {
        hasher.update(part.as_bytes());
        hasher.update([0x1f]);
    }
    hasher.update(secs.to_be_bytes());
    let digest = hasher.finalize();
    let mut key = String::from("h:");
    for byte in digest.iter().take(16) {
        let _ = write!(key, "{byte:02x}");
    }
    key
}

/// Finds lyrics through the provider chain, caches them and keeps per-track offsets.
pub struct LyricsHub {
    inner: Arc<Inner>,
    handle: Handle,
}

impl std::fmt::Debug for LyricsHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LyricsHub").finish_non_exhaustive()
    }
}

impl LyricsHub {
    /// Build the hub. The default chain is: local file, then `providers` in the order given
    /// (the app adds the YouTube Music provider here), then Unison, LRCLIB, Better Lyrics API,
    /// and the opt-in providers (`SimpMusic`, `NetEase`), which stay off until the user turns
    /// them on. Every syllable- or word-capable source is on by default (owner decision
    /// 2026-11-11). A provider's on/off state is read from the cache database only when the
    /// user stored a choice, so the user's choice survives restarts and a changed default
    /// reaches users who never touched the switch; with no choice stored,
    /// [`LyricsProvider::info`] decides.
    ///
    /// The cache lives in `data_dir`. Work runs on `handle`.
    ///
    /// # Errors
    /// [`HubError`] if the cache cannot be opened or a provider's HTTP client cannot be built.
    pub fn new(
        data_dir: &Path,
        handle: Handle,
        providers: Vec<Arc<dyn LyricsProvider>>,
    ) -> Result<Self, HubError> {
        let cache = LyricsCache::open(data_dir)?;
        let mut all: Vec<Arc<dyn LyricsProvider>> = vec![Arc::new(LocalProvider::new())];
        all.extend(providers);
        all.push(Arc::new(UnisonProvider::new()?));
        all.push(Arc::new(LrclibProvider::new()?));
        all.push(Arc::new(BetterLyricsProvider::new()?));
        all.push(Arc::new(SimpMusicProvider::new()?));
        all.push(Arc::new(NeteaseProvider::new()?));
        let entries = all
            .into_iter()
            .map(|provider| {
                let stored = cache.provider_enabled(provider.id()).ok().flatten();
                let enabled = stored.unwrap_or_else(|| provider.info().default_enabled);
                ChainEntry { provider, enabled }
            })
            .collect();
        Ok(Self::with_chain(
            cache,
            handle,
            entries,
            ChainOptions::default(),
        ))
    }

    /// Build the hub from an explicit cache and chain (tests, custom setups).
    #[must_use]
    pub fn with_chain(
        cache: LyricsCache,
        handle: Handle,
        entries: Vec<ChainEntry>,
        options: ChainOptions,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                cache,
                chain: Mutex::new(entries),
                options,
            }),
            handle,
        }
    }

    /// The chain in order, with each provider's enabled flag.
    #[must_use]
    pub fn chain_config(&self) -> Vec<(ProviderId, bool)> {
        self.inner
            .chain()
            .iter()
            .map(|e| (e.provider.id(), e.enabled))
            .collect()
    }

    /// Switch a provider on or off for this run. Returns false if no provider has that id.
    /// [`LyricsService::set_provider_enabled`] also stores the choice.
    #[must_use = "false means no provider has that id"]
    pub fn set_enabled(&self, id: ProviderId, enabled: bool) -> bool {
        let mut chain = self.inner.chain();
        match chain.iter_mut().find(|e| e.provider.id() == id) {
            Some(entry) => {
                entry.enabled = enabled;
                true
            }
            None => false,
        }
    }

    /// Reorder the chain. Ids not known are ignored; providers not listed keep their relative
    /// order after the listed ones.
    pub fn set_order(&self, order: &[ProviderId]) {
        let mut chain = self.inner.chain();
        let mut rest = std::mem::take(&mut *chain);
        for id in order {
            if let Some(pos) = rest.iter().position(|e| e.provider.id() == *id) {
                chain.push(rest.remove(pos));
            }
        }
        chain.extend(rest);
    }

    /// Forget cached lyrics for one track ("refetch lyrics"). The offset and the pick stay.
    pub fn refetch(&self, track_key: &str) {
        if let Err(e) = self.inner.cache.remove_track(track_key) {
            tracing::warn!(error = %e, "lyrics cache clear failed");
        }
    }

    /// Clear the whole lyrics cache (settings). Offsets, picks and provider choices stay.
    pub fn clear_cache(&self) {
        if let Err(e) = self.inner.cache.clear() {
            tracing::warn!(error = %e, "lyrics cache clear failed");
        }
    }
}

impl LyricsService for LyricsHub {
    fn lookup<'a>(&'a self, query: &'a LyricsQuery) -> BoxFuture<'a, LyricsLookup> {
        let inner = Arc::clone(&self.inner);
        let query = query.clone();
        let task = self.handle.spawn(async move { inner.lookup(query).await });
        Box::pin(async move {
            task.await.unwrap_or_else(|_| LyricsLookup {
                doc: None,
                tried: Vec::new(),
            })
        })
    }

    fn lookup_all<'a>(&'a self, query: &'a LyricsQuery) -> BoxFuture<'a, Vec<LyricsCandidate>> {
        let inner = Arc::clone(&self.inner);
        let query = query.clone();
        let task = self
            .handle
            .spawn(async move { inner.lookup_all(query).await });
        Box::pin(async move { task.await.unwrap_or_default() })
    }

    fn offset_ms(&self, track_key: &str) -> i64 {
        self.inner.cache.offset(track_key).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "lyrics offset read failed");
            0
        })
    }

    fn set_offset_ms(&self, track_key: &str, offset_ms: i64) {
        let offset = offset_ms.clamp(-MAX_OFFSET_MS, MAX_OFFSET_MS);
        if let Err(e) = self.inner.cache.set_offset(track_key, offset) {
            tracing::warn!(error = %e, "lyrics offset write failed");
        }
    }

    fn choose(&self, track_key: &str, provider: ProviderId) {
        if let Err(e) = self.inner.cache.set_choice(track_key, provider) {
            tracing::warn!(error = %e, "lyrics choice write failed");
        }
    }

    fn chosen(&self, track_key: &str) -> Option<ProviderId> {
        self.inner.chosen_id(track_key)
    }

    fn clear_choice(&self, track_key: &str) {
        if let Err(e) = self.inner.cache.clear_choice(track_key) {
            tracing::warn!(error = %e, "lyrics choice clear failed");
        }
    }

    fn credit(&self, track_key: &str, provider: ProviderId) -> Option<LyricsCredit> {
        self.inner
            .cache
            .credit(track_key, provider)
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "lyrics credit read failed");
                None
            })
    }

    fn providers(&self) -> Vec<LyricsProviderStatus> {
        self.inner
            .chain()
            .iter()
            .map(|e| {
                let info = e.provider.info();
                LyricsProviderStatus {
                    id: e.provider.id(),
                    enabled: e.enabled,
                    default_enabled: info.default_enabled,
                    risk_note: info.risk_note,
                    hosts: e.provider.hosts(),
                    best: e.provider.best_granularity(),
                }
            })
            .collect()
    }

    fn set_provider_enabled(&self, provider: ProviderId, enabled: bool) -> bool {
        if !self.set_enabled(provider, enabled) {
            return false;
        }
        if let Err(e) = self.inner.cache.set_provider_enabled(provider, enabled) {
            tracing::warn!(error = %e, "lyrics provider setting write failed");
        }
        true
    }
}

mod inner;

#[cfg(test)]
mod tests;
