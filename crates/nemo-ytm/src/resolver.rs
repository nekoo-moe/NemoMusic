//! The resolver chain the app uses: a per-video cache in front of the native innertube
//! request, with `yt-dlp` as the fallback.
//!
//! - [`CachedResolver`] keeps each resolved stream until its `expire` time minus
//!   [`EXPIRY_MARGIN`], shares one request between callers that ask for the same video with the
//!   same [`StreamRequest`] at the same time, and resolves [`warm`](StreamResolver::warm) hints
//!   in the background. A stream resolved for one request is never served for another.
//! - [`FallbackResolver`] tries the primary resolver first and the fallback on any error. When
//!   the primary has not answered after [`RACE_AFTER`], the fallback starts alongside it and the
//!   first good answer wins; the other is cancelled. A
//!   request the anonymous native path cannot honour (an audio language or Premium) goes to the
//!   fallback directly. When
//!   a stream from the primary cannot be opened ([`stream_failed`](StreamResolver::stream_failed)),
//!   that video goes to the fallback from then on; after [`PRIMARY_FAILURE_LIMIT`] such
//!   failures the primary is switched off for the session. A primary that fails requests, or
//!   refuses many videos in a row, rests for a while (`rest` module); one refused video does not.
//!
//! Nothing here logs a URL or a video id.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use nemo_core::BoxFuture;
use nemo_core::audio::Codec;
use nemo_core::stream::{ResolvedStream, StreamError, StreamRequest, StreamResolver};
use nemo_core::track::VideoId;
use tokio::runtime::Handle;
use tokio::sync::OnceCell;

use crate::cipher::CipherResolver;
use crate::innertube::{ANDROID_VR, ClientProfile, InnertubeResolver, VISIONOS};
use crate::timing;
use crate::ytdlp::YtDlpResolver;

/// A cached stream is used only while it is valid for at least this long.
pub const EXPIRY_MARGIN: Duration = Duration::from_mins(10);
/// Most videos kept in the cache.
pub const MAX_CACHED: usize = 64;
/// Pause after each background resolve of a hint ([`StreamResolver::warm`]): hints come one at
/// a time and a little apart, so a queue or a start-up never makes a burst of requests.
pub const WARM_PAUSE: Duration = Duration::from_millis(400);
/// Failed opens of primary streams after which the primary is switched off.
pub const PRIMARY_FAILURE_LIMIT: u32 = 3;
/// How long the primary may take before the fallback starts alongside it. A native answer
/// takes 70 to 500 ms and a refusal about 100 ms (measured 2026-11-19); the request timeout is
/// [`PLAYER_TIMEOUT`](crate::innertube::PLAYER_TIMEOUT) per client.
pub const RACE_AFTER: Duration = Duration::from_millis(1500);
/// Client profiles of the native path, in order. `visionos` answered in 251 ms and its URL
/// opened; `android_vr` (the owner's plan) answered in 477 ms and opened too, but yt-dlp
/// reports its formats need a PO token, so it is second (measured 2026-11-11, one video).
pub const NATIVE_PROFILES: &[ClientProfile] = &[VISIONOS, ANDROID_VR];

mod check;
mod persist;
mod rest;

pub use check::{CHECK_TIMEOUT, HttpUrlCheck, UrlCheck};
pub use persist::{SIGN_OUT_STEP as STREAM_STORE_STEP, StreamStore};
use rest::{Outcome, Rest};
pub use rest::{PRIMARY_MISS_LIMIT, PRIMARY_PAUSE, PRIMARY_REFUSAL_LIMIT, PRIMARY_REFUSAL_PAUSE};

type Answer = Result<ResolvedStream, StreamError>;

/// The automatic request: original language, no Premium, no codec hint.
const DEFAULT_REQUEST: StreamRequest = StreamRequest {
    audio_language: None,
    prefer_premium: false,
    prefer_codec: None,
};
type Slot = Arc<OnceCell<Answer>>;
type Slots = Arc<Mutex<HashMap<CacheKey, Slot>>>;

/// One cache entry: a video and the request it was resolved with.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    video: String,
    audio_language: Option<String>,
    prefer_premium: bool,
    prefer_codec: Option<Codec>,
}

impl CacheKey {
    fn new(video: &VideoId, request: &StreamRequest) -> Self {
        Self {
            video: video.as_str().to_owned(),
            audio_language: request.audio_language.clone(),
            prefer_premium: request.prefer_premium,
            prefer_codec: request.prefer_codec,
        }
    }
}

fn usable(stream: &ResolvedStream, now: SystemTime) -> bool {
    stream.expires_at.is_some_and(|at| {
        at.duration_since(now)
            .is_ok_and(|left| left > EXPIRY_MARGIN)
    })
}

fn slot_is_live(slot: &Slot, now: SystemTime) -> bool {
    match slot.get() {
        None => true,
        Some(Ok(stream)) => usable(stream, now),
        Some(Err(_)) => false,
    }
}

/// Caches resolved streams per video id and [`StreamRequest`] until they are about to expire.
#[derive(Clone)]
pub struct CachedResolver {
    inner: Arc<dyn StreamResolver>,
    handle: Handle,
    slots: Slots,
    persisted: Option<Persisted>,
    /// One permit: hints are resolved one at a time.
    warming: Arc<tokio::sync::Semaphore>,
}

/// The file of kept streams and the check that runs before one of them is reused.
#[derive(Clone)]
struct Persisted {
    store: Arc<StreamStore>,
    check: Arc<dyn UrlCheck>,
}

impl std::fmt::Debug for CachedResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedResolver").finish_non_exhaustive()
    }
}

impl CachedResolver {
    /// Cache the answers of `inner`. Background work runs on `handle`.
    #[must_use]
    pub fn new(inner: Arc<dyn StreamResolver>, handle: Handle) -> Self {
        Self {
            inner,
            handle,
            slots: Arc::default(),
            persisted: None,
            warming: Arc::new(tokio::sync::Semaphore::new(1)),
        }
    }

    /// Also keep the streams of the automatic request in `store` (a file that survives a
    /// restart), and reuse one from an earlier run only after `check` says its address still
    /// answers.
    #[must_use]
    pub fn with_persisted(mut self, store: Arc<StreamStore>, check: Arc<dyn UrlCheck>) -> Self {
        self.persisted = Some(Persisted { store, check });
        self
    }

    /// The slot for `key`: the live one, or a fresh one.
    fn slot(slots: &Slots, key: &CacheKey) -> Slot {
        let now = SystemTime::now();
        let mut map = slots.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(slot) = map.get(key).filter(|s| slot_is_live(s, now)) {
            return Arc::clone(slot);
        }
        if map.len() >= MAX_CACHED {
            map.retain(|_, s| slot_is_live(s, now));
            if map.len() >= MAX_CACHED {
                map.retain(|_, s| s.get().is_none());
            }
        }
        let slot: Slot = Arc::default();
        map.insert(key.clone(), Arc::clone(&slot));
        slot
    }

    /// A stream kept by an earlier run, when its address still answers.
    async fn from_disk(
        persisted: Option<&Persisted>,
        video: &VideoId,
        request: &StreamRequest,
    ) -> Option<ResolvedStream> {
        let kept = persisted?;
        if *request != DEFAULT_REQUEST {
            return None;
        }
        let stream = kept.store.get(video)?;
        let began = Instant::now();
        let alive = kept.check.alive(&stream).await;
        timing::stage("stored stream check", began, alive);
        if alive {
            return Some(stream);
        }
        let store = Arc::clone(&kept.store);
        let video = video.clone();
        drop(tokio::task::spawn_blocking(move || store.forget(&video)));
        None
    }

    /// Keep a fresh stream of the automatic request in the file.
    fn remember(
        persisted: Option<&Persisted>,
        video: &VideoId,
        request: &StreamRequest,
        answer: &Answer,
    ) {
        let (Some(kept), Ok(stream)) = (persisted, answer) else {
            return;
        };
        if *request == DEFAULT_REQUEST && kept.store.put(video, stream) {
            let store = Arc::clone(&kept.store);
            drop(tokio::task::spawn_blocking(move || store.flush()));
        }
    }

    async fn get(
        inner: Arc<dyn StreamResolver>,
        slots: Slots,
        persisted: Option<Persisted>,
        video: VideoId,
        request: StreamRequest,
    ) -> Answer {
        let key = CacheKey::new(&video, &request);
        let slot = Self::slot(&slots, &key);
        let answer = slot
            .get_or_init(|| async {
                if let Some(kept) = Self::from_disk(persisted.as_ref(), &video, &request).await {
                    return Ok(kept);
                }
                let answer = inner.resolve_with(&video, &request).await;
                Self::remember(persisted.as_ref(), &video, &request, &answer);
                answer
            })
            .await
            .clone();
        let keep = answer.as_ref().is_ok_and(|s| usable(s, SystemTime::now()));
        if !keep {
            let mut map = slots.lock().unwrap_or_else(PoisonError::into_inner);
            if map.get(&key).is_some_and(|s| Arc::ptr_eq(s, &slot)) {
                map.remove(&key);
            }
        }
        answer
    }

    /// Number of cached or pending answers, one per video and request (for tests and reports).
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// `true` when nothing is cached or pending.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl StreamResolver for CachedResolver {
    fn resolve<'a>(&'a self, video: &'a VideoId) -> BoxFuture<'a, Answer> {
        self.resolve_with(video, &DEFAULT_REQUEST)
    }

    fn resolve_with<'a>(
        &'a self,
        video: &'a VideoId,
        request: &'a StreamRequest,
    ) -> BoxFuture<'a, Answer> {
        let task = self.handle.spawn(Self::get(
            Arc::clone(&self.inner),
            Arc::clone(&self.slots),
            self.persisted.clone(),
            video.clone(),
            request.clone(),
        ));
        Box::pin(async move {
            task.await
                .unwrap_or_else(|_| Err(StreamError::Network("resolve was cancelled".to_owned())))
        })
    }

    fn warm(&self, video: &VideoId) {
        let known = self
            .slots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&CacheKey::new(video, &DEFAULT_REQUEST))
            .is_some_and(|s| slot_is_live(s, SystemTime::now()));
        if !known {
            // The answer lands in the cache; nobody waits for it here. Hints go one at a time,
            // a little apart; a song that is asked for meanwhile shares the running resolve.
            let (inner, slots) = (Arc::clone(&self.inner), Arc::clone(&self.slots));
            let (persisted, warming) = (self.persisted.clone(), Arc::clone(&self.warming));
            let video = video.clone();
            drop(self.handle.spawn(async move {
                let Ok(_turn) = warming.acquire().await else {
                    return;
                };
                let _ = Self::get(inner, slots, persisted, video, StreamRequest::default()).await;
                tokio::time::sleep(WARM_PAUSE).await;
            }));
        }
    }

    /// Drops the cached answers of `video` for every request.
    fn stream_failed(&self, video: &VideoId) {
        self.slots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|key, _| key.video != video.as_str());
        if let Some(kept) = &self.persisted {
            kept.store.forget(video);
        }
        self.inner.stream_failed(video);
    }
}

/// Tries `primary`, then `fallback`.
pub struct FallbackResolver {
    primary: Arc<dyn StreamResolver>,
    fallback: Arc<dyn StreamResolver>,
    /// Videos whose last stream came from the primary.
    via_primary: Mutex<HashSet<String>>,
    /// Videos whose primary stream could not be opened.
    skip_primary: Mutex<HashSet<String>>,
    failures: AtomicU32,
    /// Failed and refused primary resolves, and when the primary rests.
    rest: Rest,
    /// The fallback starts alongside the primary after this long.
    race_after: Duration,
}

impl std::fmt::Debug for FallbackResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FallbackResolver")
            .field("failures", &self.failures.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl FallbackResolver {
    /// Ask `primary` first and `fallback` when it fails.
    #[must_use]
    pub fn new(primary: Arc<dyn StreamResolver>, fallback: Arc<dyn StreamResolver>) -> Self {
        Self {
            primary,
            fallback,
            via_primary: Mutex::default(),
            skip_primary: Mutex::default(),
            failures: AtomicU32::new(0),
            rest: Rest::default(),
            race_after: RACE_AFTER,
        }
    }

    /// Start the fallback alongside the primary after `after` instead of [`RACE_AFTER`].
    #[must_use]
    pub const fn with_race_after(mut self, after: Duration) -> Self {
        self.race_after = after;
        self
    }

    /// The primary is the anonymous native path: it serves the original track only, so a
    /// request with an audio language or Premium goes to the fallback (`yt-dlp`).
    fn use_primary(&self, video: &VideoId, request: &StreamRequest) -> bool {
        if request.audio_language.is_some() || request.prefer_premium {
            return false;
        }
        !self.rest.resting()
            && self.failures.load(Ordering::SeqCst) < PRIMARY_FAILURE_LIMIT
            && !self
                .skip_primary
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contains(video.as_str())
    }

    fn mark(&self, video: &VideoId, primary: bool) {
        let mut set = self
            .via_primary
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if primary {
            if set.len() >= MAX_CACHED * 4 {
                set.clear();
            }
            set.insert(video.as_str().to_owned());
        } else {
            set.remove(video.as_str());
        }
    }
}

impl FallbackResolver {
    /// Take the answer of the primary: a stream is kept, an error goes to the fallback.
    async fn after_primary(
        &self,
        video: &VideoId,
        request: &StreamRequest,
        answer: Answer,
        began: Instant,
    ) -> Answer {
        timing::stage("resolve native", began, answer.is_ok());
        match answer {
            Ok(stream) => {
                self.rest.record(Outcome::Answered);
                self.mark(video, true);
                Ok(stream)
            }
            Err(e) => {
                self.rest.record(Outcome::of(&Err(e.clone())));
                tracing::debug!(error = %e, "native resolve failed, using yt-dlp");
                self.run_fallback(video, request).await
            }
        }
    }

    async fn run_fallback(&self, video: &VideoId, request: &StreamRequest) -> Answer {
        let began = Instant::now();
        let answer = self.fallback.resolve_with(video, request).await;
        self.after_fallback(video, answer, began)
    }

    fn after_fallback(&self, video: &VideoId, answer: Answer, began: Instant) -> Answer {
        timing::stage("resolve yt-dlp", began, answer.is_ok());
        if let Err(e) = &answer {
            // Fixed texts only (no URL, no id): the reason behind "could not get the stream".
            tracing::warn!(error = %e, "no stream from the native path or yt-dlp");
        }
        self.mark(video, false);
        answer
    }

    /// The primary is slow: start the fallback beside it and take the first good answer. The
    /// future that is dropped ends its request or process. When both fail, the fallback's error
    /// is the answer, as without a race.
    async fn race(
        &self,
        video: &VideoId,
        request: &StreamRequest,
        mut primary: BoxFuture<'_, Answer>,
        began: Instant,
    ) -> Answer {
        tracing::debug!(target: "nemo::start", "native is slow, yt-dlp starts beside it");
        let fallback_began = Instant::now();
        let mut fallback = self.fallback.resolve_with(video, request);
        tokio::select! {
            answer = &mut primary => {
                self.primary_first(video, answer, began, fallback, fallback_began).await
            }
            answer = &mut fallback => {
                self.fallback_first(video, answer, primary, fallback_began).await
            }
        }
    }

    /// The primary answered first: a stream cancels the fallback; an error waits for it.
    async fn primary_first(
        &self,
        video: &VideoId,
        answer: Answer,
        began: Instant,
        fallback: BoxFuture<'_, Answer>,
        fallback_began: Instant,
    ) -> Answer {
        timing::stage("resolve native", began, answer.is_ok());
        self.rest.record(Outcome::of(&answer));
        if let Ok(stream) = answer {
            drop(fallback);
            self.mark(video, true);
            return Ok(stream);
        }
        let answer = fallback.await;
        self.after_fallback(video, answer, fallback_began)
    }

    /// The fallback answered first: a stream cancels the primary (too slow to count on); an
    /// error waits for the primary.
    async fn fallback_first(
        &self,
        video: &VideoId,
        answer: Answer,
        primary: BoxFuture<'_, Answer>,
        fallback_began: Instant,
    ) -> Answer {
        if answer.is_ok() {
            drop(primary);
            // Too slow to count on: it counts as a failed request.
            self.rest.record(Outcome::Failed);
            return self.after_fallback(video, answer, fallback_began);
        }
        let late = primary.await;
        self.rest.record(Outcome::of(&late));
        if let Ok(stream) = late {
            self.mark(video, true);
            return Ok(stream);
        }
        self.after_fallback(video, answer, fallback_began)
    }
}

impl StreamResolver for FallbackResolver {
    fn resolve<'a>(&'a self, video: &'a VideoId) -> BoxFuture<'a, Answer> {
        self.resolve_with(video, &DEFAULT_REQUEST)
    }

    fn resolve_with<'a>(
        &'a self,
        video: &'a VideoId,
        request: &'a StreamRequest,
    ) -> BoxFuture<'a, Answer> {
        Box::pin(async move {
            if !self.use_primary(video, request) {
                return self.run_fallback(video, request).await;
            }
            let began = Instant::now();
            let mut primary = self.primary.resolve_with(video, request);
            match tokio::time::timeout(self.race_after, &mut primary).await {
                Ok(answer) => self.after_primary(video, request, answer, began).await,
                Err(_) => self.race(video, request, primary, began).await,
            }
        })
    }

    fn stream_failed(&self, video: &VideoId) {
        let was_primary = self
            .via_primary
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(video.as_str());
        if was_primary {
            let total = self.failures.fetch_add(1, Ordering::SeqCst) + 1;
            tracing::info!(failures = total, "a native stream could not be opened");
            self.skip_primary
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(video.as_str().to_owned());
            self.primary.stream_failed(video);
        } else {
            self.fallback.stream_failed(video);
        }
    }
}

/// The chain the app uses: cache -> native innertube request -> `yt-dlp` (fast arguments,
/// then the full extraction). `allowed` are the codecs the audio engine decodes.
#[must_use]
pub fn default_chain(
    ytdlp: YtDlpResolver,
    allowed: &[Codec],
    handle: &Handle,
) -> Arc<dyn StreamResolver> {
    persisted_chain(ytdlp, allowed, handle, None)
}

/// [`default_chain`] whose cache also keeps its streams in `store`, when there is one: a song
/// played in the last hours then starts at once after a restart.
#[must_use]
pub fn persisted_chain(
    ytdlp: YtDlpResolver,
    allowed: &[Codec],
    handle: &Handle,
    store: Option<Arc<StreamStore>>,
) -> Arc<CachedResolver> {
    persisted_chain_with(ytdlp, allowed, handle, store, None)
}

/// [`persisted_chain`] with the in-process tier (cold-start spec): the order is the cache, the
/// plain native request, `cipher` (the `tv` client and the embedded solver), `yt-dlp`. Without a
/// `cipher` it is the chain of [`persisted_chain`].
#[must_use]
pub fn persisted_chain_with(
    ytdlp: YtDlpResolver,
    allowed: &[Codec],
    handle: &Handle,
    store: Option<Arc<StreamStore>>,
    cipher: Option<Arc<CipherResolver>>,
) -> Arc<CachedResolver> {
    let ytdlp: Arc<dyn StreamResolver> = Arc::new(ytdlp);
    let second: Arc<dyn StreamResolver> = match cipher {
        Some(cipher) => Arc::new(FallbackResolver::new(cipher, ytdlp)),
        None => ytdlp,
    };
    let inner: Arc<dyn StreamResolver> =
        match InnertubeResolver::new(handle.clone(), NATIVE_PROFILES) {
            Ok(native) => Arc::new(FallbackResolver::new(
                Arc::new(native.with_allowed_codecs(allowed)),
                second,
            )),
            Err(e) => {
                tracing::warn!(error = %e, "native resolver disabled");
                second
            }
        };
    let cached = CachedResolver::new(inner, handle.clone());
    let cached = match (store, HttpUrlCheck::new()) {
        (Some(store), Ok(check)) => cached.with_persisted(store, Arc::new(check)),
        (Some(_), Err(e)) => {
            tracing::warn!(error = %e, "streams are not kept between runs");
            cached
        }
        (None, _) => cached,
    };
    Arc::new(cached)
}

#[cfg(test)]
mod tests;
