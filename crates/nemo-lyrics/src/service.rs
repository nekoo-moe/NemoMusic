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
use std::sync::{Arc, Mutex, MutexGuard};

use nemo_core::BoxFuture;
use nemo_core::lyrics::{
    LyricsCandidate, LyricsCredit, LyricsDoc, LyricsError, LyricsProvider, LyricsQuery, ProviderId,
};
use nemo_core::services::{LyricsLookup, LyricsProviderStatus, LyricsService};
use sha2::{Digest, Sha256};
use tokio::runtime::Handle;

use crate::blapi::BetterLyricsProvider;
use crate::cache::{CacheError, EntryStatus, LyricsCache};
use crate::chain::{ChainEntry, ChainOptions, Outcome, run_chain, run_chain_all};
use crate::local::{LOCAL_ID, LocalProvider};
use crate::lrclib::LrclibProvider;
use crate::netease::NeteaseProvider;
use crate::normalize::normalize;
use crate::pick::Candidate;
use crate::simpmusic::SimpMusicProvider;
use crate::unison::UnisonProvider;

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

/// Sort key of the picker: best granularity, then score, then chain order.
fn sort_candidates(list: &mut [(usize, LyricsCandidate)]) {
    list.sort_by(|a, b| {
        b.1.granularity
            .cmp(&a.1.granularity)
            .then_with(|| b.1.score.cmp(&a.1.score))
            .then_with(|| a.0.cmp(&b.0))
    });
}

impl Inner {
    fn chain(&self) -> MutexGuard<'_, Vec<ChainEntry>> {
        self.chain
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The provider the user picked for this track, if it is still in the chain.
    fn chosen_id(&self, key: &str) -> Option<ProviderId> {
        let name = self.cache.choice(key).ok().flatten()?;
        self.chain()
            .iter()
            .map(|e| e.provider.id())
            .find(|id| id.as_str() == name)
    }

    /// Answer cached fresh rows: which providers need no question, and their hits.
    fn read_cache(&self, key: &str, entries: &mut [ChainEntry]) -> Vec<Candidate> {
        let mut prior = Vec::new();
        for (order, entry) in entries.iter_mut().enumerate() {
            let id = entry.provider.id();
            if !entry.enabled || id == LOCAL_ID {
                continue;
            }
            match self.cache.get(key, id) {
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
        prior
    }

    /// Store what the providers answered.
    fn store(&self, key: &str, reports: &[crate::chain::ProviderReport]) {
        for report in reports {
            let id = report.provider;
            if id == LOCAL_ID {
                continue;
            }
            let stored = match &report.outcome {
                Outcome::Hit { doc, score, credit } => {
                    let put = self.cache.put_hit(key, id, doc, *score);
                    match credit {
                        Some(c) if put.is_ok() => self.cache.put_credit(key, id, c),
                        _ => put,
                    }
                }
                Outcome::Miss | Outcome::Rejected => self.cache.put_miss(key, id),
                Outcome::Error(e) => {
                    let retry_after = match e {
                        LyricsError::RateLimited { retry_after } => *retry_after,
                        _ => None,
                    };
                    self.cache.put_error(key, id, retry_after)
                }
                Outcome::TimedOut => Ok(()),
            };
            if let Err(e) = stored {
                tracing::warn!(provider = id.as_str(), error = %e, "lyrics cache write failed");
            }
        }
    }

    fn candidate(
        &self,
        key: &str,
        entries: &[ChainEntry],
        doc: LyricsDoc,
        score: u8,
        credit: Option<LyricsCredit>,
    ) -> LyricsCandidate {
        let risk = entries
            .iter()
            .find(|e| e.provider.id() == doc.provider)
            .and_then(|e| e.provider.info().risk_note);
        let credit = credit.or_else(|| self.cache.credit(key, doc.provider).ok().flatten());
        let source_note = credit
            .as_ref()
            .and_then(|c| c.note.clone())
            .or_else(|| risk.map(str::to_owned));
        LyricsCandidate {
            provider: doc.provider,
            granularity: doc.granularity,
            score,
            source_note,
            credit,
            doc,
        }
    }

    async fn lookup(&self, query: LyricsQuery) -> LyricsLookup {
        let key = track_key(&query);
        let entries = self.chain().clone();
        let tried: Vec<ProviderId> = entries
            .iter()
            .filter(|e| e.enabled)
            .map(|e| e.provider.id())
            .collect();

        // The user's pick wins when its lyrics are already cached.
        let chosen = self.chosen_id(&key);
        if let Some(id) = chosen.filter(|id| *id != LOCAL_ID)
            && let Ok(Some(row)) = self.cache.get(&key, id)
            && let EntryStatus::Hit { doc, .. } = row.status
        {
            return LyricsLookup {
                doc: Some(doc),
                tried,
            };
        }

        // Cache first. Providers answered by a fresh row are switched off for the chain run.
        let mut to_ask = entries.clone();
        let prior = self.read_cache(&key, &mut to_ask);

        let result = run_chain(&to_ask, &query, self.options, &prior).await;
        self.store(&key, &result.reports);

        // A fresh answer from the picked provider beats the automatic pick.
        let picked = chosen.and_then(|id| {
            result.reports.iter().find_map(|r| match &r.outcome {
                Outcome::Hit { doc, .. } if r.provider == id => Some(doc.clone()),
                _ => None,
            })
        });
        let mut lookup = result.into_lookup();
        if picked.is_some() {
            lookup.doc = picked;
        }
        lookup.tried = tried;
        lookup
    }

    async fn lookup_all(&self, query: LyricsQuery) -> Vec<LyricsCandidate> {
        let key = track_key(&query);
        let entries = self.chain().clone();
        let mut to_ask = entries.clone();
        let prior = self.read_cache(&key, &mut to_ask);

        let result = run_chain_all(&to_ask, &query, self.options, &prior).await;
        self.store(&key, &result.reports);

        let mut list: Vec<(usize, LyricsCandidate)> = prior
            .into_iter()
            .map(|c| {
                (
                    c.chain_order,
                    self.candidate(&key, &entries, c.doc, c.score, None),
                )
            })
            .collect();
        for report in result.reports {
            if let Outcome::Hit { doc, score, credit } = report.outcome {
                let order = entries
                    .iter()
                    .position(|e| e.provider.id() == report.provider)
                    .unwrap_or(usize::MAX);
                list.push((order, self.candidate(&key, &entries, doc, score, credit)));
            }
        }
        sort_candidates(&mut list);
        list.into_iter().map(|(_, c)| c).collect()
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
    /// (the app adds the YouTube Music provider here), then Unison, LRCLIB, and the opt-in
    /// providers (Better Lyrics API, `SimpMusic`, `NetEase`), which stay off until the user turns
    /// them on. A provider's on/off state is read from the cache database, so the user's
    /// choice survives restarts; with no choice stored, [`LyricsProvider::info`] decides.
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use nemo_core::lyrics::{Granularity, LyricLine, LyricsDoc, ProviderInfo};
    use nemo_core::track::VideoId;

    use super::*;

    type Answer = Result<Option<LyricsDoc>, LyricsError>;

    struct Fake {
        id: ProviderId,
        answer: Answer,
        calls: Arc<AtomicUsize>,
        credit: Option<LyricsCredit>,
        info: ProviderInfo,
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
        fn fetch_credited<'a>(
            &'a self,
            _q: &'a LyricsQuery,
        ) -> BoxFuture<'a, Result<Option<nemo_core::lyrics::CreditedDoc>, LyricsError>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let answer = self.answer.clone();
            let credit = self.credit.clone();
            Box::pin(async move { Ok(answer?.map(|d| (d, credit))) })
        }
        fn info(&self) -> ProviderInfo {
            self.info
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
            credit: None,
            info: ProviderInfo::OPEN,
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

    fn word_doc(name: &'static str) -> LyricsDoc {
        let mut d = doc(name);
        d.granularity = Granularity::Word;
        d
    }

    struct Multi {
        hub: LyricsHub,
        calls: Vec<Arc<AtomicUsize>>,
        dir: std::path::PathBuf,
    }

    impl Drop for Multi {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// A hub with fake providers `(name, answer, credit, default_enabled)` in chain order.
    fn multi(name: &str, fakes: Vec<(&'static str, Answer, Option<LyricsCredit>, bool)>) -> Multi {
        let dir = std::env::temp_dir().join(format!(
            "nemo-lyrics-multi-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        let mut calls = Vec::new();
        let entries = fakes
            .into_iter()
            .map(|(id, answer, credit, on)| {
                let c = Arc::new(AtomicUsize::new(0));
                calls.push(Arc::clone(&c));
                let fake = Fake {
                    id: ProviderId::new(id),
                    answer,
                    calls: c,
                    credit,
                    info: if on {
                        ProviderInfo::OPEN
                    } else {
                        ProviderInfo::opt_in("risky")
                    },
                };
                ChainEntry {
                    provider: Arc::new(fake),
                    enabled: on,
                }
            })
            .collect();
        let hub = LyricsHub::with_chain(
            LyricsCache::open(&dir).expect("cache"),
            Handle::current(),
            entries,
            ChainOptions::default(),
        );
        Multi { hub, calls, dir }
    }

    fn credit() -> LyricsCredit {
        LyricsCredit {
            label: "Lyrics from Fake".into(),
            url: Some("https://fake.example/song/1".into()),
            by: Some("tester".into()),
            by_url: None,
            note: Some("community confidence: low".into()),
        }
    }

    #[tokio::test]
    async fn lookup_all_lists_every_answer_best_first_with_notes() {
        let m = multi(
            "all",
            vec![
                ("line-src", Ok(Some(doc("line-src"))), None, true),
                (
                    "word-src",
                    Ok(Some(word_doc("word-src"))),
                    Some(credit()),
                    true,
                ),
                ("empty-src", Ok(None), None, true),
                ("opt-in", Ok(Some(doc("opt-in"))), None, false),
            ],
        );
        let all = m.hub.lookup_all(&query()).await;
        let names: Vec<&str> = all.iter().map(|c| c.provider.as_str()).collect();
        // Word first; the disabled provider is not asked; a miss does not appear.
        assert_eq!(names, ["word-src", "line-src"]);
        assert_eq!(all[0].granularity, Granularity::Word);
        assert_eq!(all[0].score, 100);
        assert_eq!(
            all[0].source_note.as_deref(),
            Some("community confidence: low")
        );
        assert_eq!(
            all[0].credit.as_ref().map(|c| c.label.as_str()),
            Some("Lyrics from Fake")
        );
        assert_eq!(m.calls[3].load(Ordering::SeqCst), 0);
        // A second call is served from the cache, credit included.
        let again = m.hub.lookup_all(&query()).await;
        assert_eq!(again.len(), 2);
        assert_eq!(again[0].credit, all[0].credit);
        assert_eq!(m.calls[0].load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn chosen_provider_is_returned_first_and_remembered() {
        let m = multi(
            "choose",
            vec![
                ("line-src", Ok(Some(doc("line-src"))), None, true),
                ("word-src", Ok(Some(word_doc("word-src"))), None, true),
            ],
        );
        let q = query();
        let key = track_key(&q);
        assert_eq!(m.hub.chosen(&key), None);
        // Without a pick the best granularity wins.
        let first = m.hub.lookup(&q).await;
        assert_eq!(first.doc.map(|d| d.provider.as_str()), Some("word-src"));

        m.hub.choose(&key, ProviderId::new("line-src"));
        assert_eq!(m.hub.chosen(&key), Some(ProviderId::new("line-src")));
        let picked = m.hub.lookup(&q).await;
        assert_eq!(picked.doc.map(|d| d.provider.as_str()), Some("line-src"));

        // The pick survives a refetch (it is a user setting) and still wins on fresh answers.
        m.hub.refetch(&key);
        let again = m.hub.lookup(&q).await;
        assert_eq!(again.doc.map(|d| d.provider.as_str()), Some("line-src"));

        m.hub.clear_choice(&key);
        assert_eq!(m.hub.chosen(&key), None);
        let best = m.hub.lookup(&q).await;
        assert_eq!(best.doc.map(|d| d.provider.as_str()), Some("word-src"));
    }

    #[tokio::test]
    async fn credit_is_stored_with_the_hit() {
        let m = multi(
            "credit",
            vec![(
                "word-src",
                Ok(Some(word_doc("word-src"))),
                Some(credit()),
                true,
            )],
        );
        let q = query();
        let key = track_key(&q);
        let id = ProviderId::new("word-src");
        assert_eq!(m.hub.credit(&key, id), None);
        let _ = m.hub.lookup(&q).await;
        assert_eq!(m.hub.credit(&key, id), Some(credit()));
        m.hub.refetch(&key);
        assert_eq!(m.hub.credit(&key, id), None);
    }

    #[tokio::test]
    async fn provider_list_and_choices_persist() {
        let dir = std::env::temp_dir().join(format!("nemo-lyrics-persist-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let hub = LyricsHub::new(&dir, Handle::current(), Vec::new()).expect("hub");
        let state = |hub: &LyricsHub| {
            hub.providers()
                .into_iter()
                .map(|p| (p.id.as_str(), p.enabled))
                .collect::<Vec<_>>()
        };
        // Open sources on, grey sources off, each grey one with a risk note.
        assert_eq!(
            state(&hub),
            [
                ("Local file", true),
                ("Unison", true),
                ("LRCLIB", true),
                ("Better Lyrics", false),
                ("SimpMusic", false),
                ("NetEase", false),
            ]
        );
        for p in hub.providers() {
            assert_eq!(
                p.risk_note.is_some(),
                !p.default_enabled,
                "{}",
                p.id.as_str()
            );
            assert!(p.id == LOCAL_ID || !p.hosts.is_empty());
        }
        assert!(hub.set_provider_enabled(ProviderId::new("NetEase"), true));
        assert!(hub.set_provider_enabled(ProviderId::new("Unison"), false));
        assert!(!hub.set_provider_enabled(ProviderId::new("nope"), true));
        drop(hub);

        let again = LyricsHub::new(&dir, Handle::current(), Vec::new()).expect("hub");
        let now = state(&again);
        assert!(now.contains(&("NetEase", true)) && now.contains(&("Unison", false)));
        drop(again);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
