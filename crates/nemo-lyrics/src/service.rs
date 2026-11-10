//! [`LyricsHub`]: the [`LyricsService`] the app wires in (ADR 0002).
//!
//! Lookup order: cache first, then the provider chain, then store the result.
//! - A provider with a fresh cache row (hit, miss or error) is not asked again; cached hits still
//!   compete in the final pick. The local provider is never cached (the files can change).
//! - After the chain runs, hits are stored for 30 days, misses and wrong matches for 7 days,
//!   errors for 15 minutes (or the provider's `Retry-After`). Timeouts are not stored.
//! - Offsets live in SQLite and are clamped to +-10 s, as the ADR allows.
//!
//! The service futures are runtime-agnostic: the work runs on the tokio [`Handle`] given at
//! construction and the returned future only awaits it.

use std::fmt::Write as _;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use nemo_core::BoxFuture;
use nemo_core::lyrics::{LyricsError, LyricsProvider, LyricsQuery, ProviderId};
use nemo_core::services::{LyricsLookup, LyricsService};
use sha2::{Digest, Sha256};
use tokio::runtime::Handle;

use crate::cache::{CacheError, EntryStatus, LyricsCache};
use crate::chain::{ChainEntry, ChainOptions, Outcome, run_chain};
use crate::local::{LOCAL_ID, LocalProvider};
use crate::lrclib::LrclibProvider;
use crate::normalize::normalize;
use crate::pick::Candidate;

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

struct Inner {
    cache: LyricsCache,
    chain: Mutex<Vec<ChainEntry>>,
    options: ChainOptions,
}

impl Inner {
    fn chain(&self) -> MutexGuard<'_, Vec<ChainEntry>> {
        self.chain
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    async fn lookup(&self, query: LyricsQuery) -> LyricsLookup {
        let key = track_key(&query);
        let entries = self.chain().clone();
        let tried: Vec<ProviderId> = entries
            .iter()
            .filter(|e| e.enabled)
            .map(|e| e.provider.id())
            .collect();

        // Cache first. Providers answered by a fresh row are switched off for the chain run.
        let mut prior: Vec<Candidate> = Vec::new();
        let mut to_ask = entries.clone();
        for (order, entry) in to_ask.iter_mut().enumerate() {
            let id = entry.provider.id();
            if !entry.enabled || id == LOCAL_ID {
                continue;
            }
            match self.cache.get(&key, id) {
                Ok(Some(row)) => {
                    entry.enabled = false;
                    if let EntryStatus::Hit { doc, score } = row.status {
                        prior.push(Candidate {
                            doc,
                            score,
                            chain_order: order,
                        });
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(provider = id.as_str(), error = %e, "lyrics cache read failed");
                }
            }
        }

        // Then the chain.
        let result = run_chain(&to_ask, &query, self.options, &prior).await;

        // Then store.
        for report in &result.reports {
            let id = report.provider;
            if id == LOCAL_ID {
                continue;
            }
            let stored = match &report.outcome {
                Outcome::Hit { doc, score } => self.cache.put_hit(&key, id, doc, *score),
                Outcome::Miss | Outcome::Rejected => self.cache.put_miss(&key, id),
                Outcome::Error(e) => {
                    let retry_after = match e {
                        LyricsError::RateLimited { retry_after } => *retry_after,
                        _ => None,
                    };
                    self.cache.put_error(&key, id, retry_after)
                }
                Outcome::TimedOut => Ok(()),
            };
            if let Err(e) = stored {
                tracing::warn!(provider = id.as_str(), error = %e, "lyrics cache write failed");
            }
        }

        let mut lookup = result.into_lookup();
        lookup.tried = tried;
        lookup
    }
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
    /// (the app adds the YouTube Music provider here), then LRCLIB.
    ///
    /// The cache lives in `data_dir`. Work runs on `handle`.
    ///
    /// # Errors
    /// [`HubError`] if the cache cannot be opened or LRCLIB's HTTP client cannot be built.
    pub fn new(
        data_dir: &Path,
        handle: Handle,
        providers: Vec<Arc<dyn LyricsProvider>>,
    ) -> Result<Self, HubError> {
        let cache = LyricsCache::open(data_dir)?;
        let mut entries = vec![ChainEntry::new(Arc::new(LocalProvider::new()))];
        entries.extend(providers.into_iter().map(ChainEntry::new));
        entries.push(ChainEntry::new(Arc::new(LrclibProvider::new()?)));
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

    /// Switch a provider on or off. Returns false if no provider has that id.
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

    /// Forget cached lyrics for one track ("refetch lyrics"). The offset stays.
    pub fn refetch(&self, track_key: &str) {
        if let Err(e) = self.inner.cache.remove_track(track_key) {
            tracing::warn!(error = %e, "lyrics cache clear failed");
        }
    }

    /// Clear the whole lyrics cache (settings). Offsets stay.
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
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use nemo_core::lyrics::{Granularity, LyricLine, LyricsDoc};
    use nemo_core::track::VideoId;

    use super::*;

    type Answer = Result<Option<LyricsDoc>, LyricsError>;

    struct Fake {
        id: ProviderId,
        answer: Answer,
        calls: Arc<AtomicUsize>,
    }

    impl LyricsProvider for Fake {
        fn id(&self) -> ProviderId {
            self.id
        }
        fn hosts(&self) -> &'static [&'static str] {
            &["fake.example"]
        }
        fn fetch<'a>(&'a self, _q: &'a LyricsQuery) -> BoxFuture<'a, Answer> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let answer = self.answer.clone();
            Box::pin(async move { answer })
        }
    }

    fn doc(name: &'static str) -> LyricsDoc {
        LyricsDoc {
            lines: vec![LyricLine {
                start_ms: Some(1000),
                end_ms: None,
                text: "hello".into(),
                words: vec![],
            }],
            granularity: Granularity::Line,
            provider: ProviderId::new(name),
            matched: None,
            instrumental: false,
        }
    }

    fn query() -> LyricsQuery {
        LyricsQuery {
            title: "Test Song".into(),
            artists: vec!["Test Artist".into()],
            album: Some("Test Album".into()),
            duration_ms: Some(200_000),
            ytm_video_id: Some(VideoId::parse("abcdefghijk").expect("id")),
            local_path: None,
        }
    }

    struct Setup {
        hub: LyricsHub,
        calls: Arc<AtomicUsize>,
        dir: std::path::PathBuf,
    }

    impl Drop for Setup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn setup(name: &str, answer: Answer) -> Setup {
        let dir = std::env::temp_dir().join(format!(
            "nemo-lyrics-hub-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        let calls = Arc::new(AtomicUsize::new(0));
        let fake = Fake {
            id: ProviderId::new("fake"),
            answer,
            calls: Arc::clone(&calls),
        };
        let cache = LyricsCache::open(&dir).expect("cache");
        let hub = LyricsHub::with_chain(
            cache,
            Handle::current(),
            vec![ChainEntry::new(Arc::new(fake))],
            ChainOptions::default(),
        );
        Setup { hub, calls, dir }
    }

    #[tokio::test]
    async fn hit_is_cached_and_served_without_asking_again() {
        let s = setup("hit", Ok(Some(doc("fake"))));
        let q = query();
        let first = s.hub.lookup(&q).await;
        assert!(first.doc.is_some());
        assert_eq!(first.tried, [ProviderId::new("fake")]);
        let second = s.hub.lookup(&q).await;
        assert_eq!(second.doc, first.doc);
        assert_eq!(s.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn miss_and_error_are_negative_cached() {
        let s = setup("miss", Ok(None));
        let q = query();
        assert!(s.hub.lookup(&q).await.doc.is_none());
        assert!(s.hub.lookup(&q).await.doc.is_none());
        assert_eq!(s.calls.load(Ordering::SeqCst), 1);

        let e = setup(
            "err",
            Err(LyricsError::RateLimited {
                retry_after: Some(Duration::from_secs(60)),
            }),
        );
        let lookup = e.hub.lookup(&q).await;
        assert!(lookup.doc.is_none());
        assert_eq!(lookup.tried.len(), 1);
        let _ = e.hub.lookup(&q).await;
        assert_eq!(e.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn refetch_and_clear_ask_the_provider_again() {
        let s = setup("refetch", Ok(Some(doc("fake"))));
        let q = query();
        let _ = s.hub.lookup(&q).await;
        s.hub.refetch(&track_key(&q));
        let _ = s.hub.lookup(&q).await;
        assert_eq!(s.calls.load(Ordering::SeqCst), 2);
        s.hub.clear_cache();
        let _ = s.hub.lookup(&q).await;
        assert_eq!(s.calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn disabled_provider_is_not_asked_or_listed() {
        let s = setup("disabled", Ok(Some(doc("fake"))));
        assert!(s.hub.set_enabled(ProviderId::new("fake"), false));
        assert!(!s.hub.set_enabled(ProviderId::new("nope"), false));
        let lookup = s.hub.lookup(&query()).await;
        assert!(lookup.doc.is_none() && lookup.tried.is_empty());
        assert_eq!(s.calls.load(Ordering::SeqCst), 0);
        assert_eq!(s.hub.chain_config(), [(ProviderId::new("fake"), false)]);
    }

    #[tokio::test]
    async fn order_is_editable() {
        let s = setup("order", Ok(None));
        s.hub
            .set_order(&[ProviderId::new("unknown"), ProviderId::new("fake")]);
        assert_eq!(s.hub.chain_config().len(), 1);
    }

    #[tokio::test]
    async fn offsets_are_stored_and_clamped() {
        let s = setup("offset", Ok(None));
        assert_eq!(s.hub.offset_ms("abc"), 0);
        s.hub.set_offset_ms("abc", 350);
        assert_eq!(s.hub.offset_ms("abc"), 350);
        s.hub.set_offset_ms("abc", -999_999);
        assert_eq!(s.hub.offset_ms("abc"), -MAX_OFFSET_MS);
        s.hub.set_offset_ms("abc", 0);
        assert_eq!(s.hub.offset_ms("abc"), 0);
    }

    #[test]
    fn key_is_video_id_or_stable_hash() {
        let mut q = query();
        assert_eq!(track_key(&q), "abcdefghijk");
        q.ytm_video_id = None;
        let a = track_key(&q);
        assert!(a.starts_with("h:") && a.len() == 2 + 32);
        // Case, punctuation, feat. suffix and 400 ms of duration noise do not change the key.
        let mut same = q.clone();
        same.title = "TEST song (feat. X)".into();
        same.artists = vec!["test  artist!".into()];
        same.duration_ms = Some(200_400);
        assert_eq!(track_key(&same), a);
        // A different track does.
        let mut other = q.clone();
        other.title = "Another Song".into();
        assert_ne!(track_key(&other), a);
        let mut longer = q;
        longer.duration_ms = Some(215_000);
        assert_ne!(track_key(&longer), a);
    }
}
