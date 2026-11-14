//! The resolver chain the app uses: a per-video cache in front of the native innertube
//! request, with `yt-dlp` as the fallback.
//!
//! - [`CachedResolver`] keeps each resolved stream until its `expire` time minus
//!   [`EXPIRY_MARGIN`], shares one request between callers that ask for the same video at the
//!   same time, and resolves [`warm`](StreamResolver::warm) hints in the background.
//! - [`FallbackResolver`] tries the primary resolver first and the fallback on any error. When
//!   a stream from the primary cannot be opened ([`stream_failed`](StreamResolver::stream_failed)),
//!   that video goes to the fallback from then on; after [`PRIMARY_FAILURE_LIMIT`] such
//!   failures the primary is switched off for the session.
//!
//! Nothing here logs a URL or a video id.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use nemo_core::BoxFuture;
use nemo_core::audio::Codec;
use nemo_core::stream::{ResolvedStream, StreamError, StreamResolver};
use nemo_core::track::VideoId;
use tokio::runtime::Handle;
use tokio::sync::OnceCell;

use crate::innertube::{ANDROID_VR, ClientProfile, InnertubeResolver, VISIONOS};
use crate::ytdlp::YtDlpResolver;

/// A cached stream is used only while it is valid for at least this long.
pub const EXPIRY_MARGIN: Duration = Duration::from_mins(10);
/// Most videos kept in the cache.
pub const MAX_CACHED: usize = 64;
/// Failed opens of primary streams after which the primary is switched off.
pub const PRIMARY_FAILURE_LIMIT: u32 = 3;
/// Failed primary resolves in a row after which the primary rests.
pub const PRIMARY_MISS_LIMIT: u32 = 2;
/// How long the primary rests.
pub const PRIMARY_PAUSE: Duration = Duration::from_mins(10);
/// Client profiles of the native path, in order. `visionos` answered in 251 ms and its URL
/// opened; `android_vr` (the owner's plan) answered in 477 ms and opened too, but yt-dlp
/// reports its formats need a PO token, so it is second (measured 2026-11-11, one video).
pub const NATIVE_PROFILES: &[ClientProfile] = &[VISIONOS, ANDROID_VR];

type Answer = Result<ResolvedStream, StreamError>;
type Slot = Arc<OnceCell<Answer>>;
type Slots = Arc<Mutex<HashMap<String, Slot>>>;

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

/// Caches resolved streams per video id until they are about to expire.
#[derive(Clone)]
pub struct CachedResolver {
    inner: Arc<dyn StreamResolver>,
    handle: Handle,
    slots: Slots,
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
        }
    }

    /// The slot for `id`: the live one, or a fresh one.
    fn slot(slots: &Slots, id: &str) -> Slot {
        let now = SystemTime::now();
        let mut map = slots.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(slot) = map.get(id).filter(|s| slot_is_live(s, now)) {
            return Arc::clone(slot);
        }
        if map.len() >= MAX_CACHED {
            map.retain(|_, s| slot_is_live(s, now));
            if map.len() >= MAX_CACHED {
                map.retain(|_, s| s.get().is_none());
            }
        }
        let slot: Slot = Arc::default();
        map.insert(id.to_owned(), Arc::clone(&slot));
        slot
    }

    async fn get(inner: Arc<dyn StreamResolver>, slots: Slots, video: VideoId) -> Answer {
        let slot = Self::slot(&slots, video.as_str());
        let answer = slot
            .get_or_init(|| async { inner.resolve(&video).await })
            .await
            .clone();
        let keep = answer.as_ref().is_ok_and(|s| usable(s, SystemTime::now()));
        if !keep {
            let mut map = slots.lock().unwrap_or_else(PoisonError::into_inner);
            if map
                .get(video.as_str())
                .is_some_and(|s| Arc::ptr_eq(s, &slot))
            {
                map.remove(video.as_str());
            }
        }
        answer
    }

    /// Number of videos with a cached or pending answer (for tests and reports).
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
        let task = self.handle.spawn(Self::get(
            Arc::clone(&self.inner),
            Arc::clone(&self.slots),
            video.clone(),
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
            .get(video.as_str())
            .is_some_and(|s| slot_is_live(s, SystemTime::now()));
        if !known {
            // The answer lands in the cache; nobody waits for it here.
            drop(self.handle.spawn(Self::get(
                Arc::clone(&self.inner),
                Arc::clone(&self.slots),
                video.clone(),
            )));
        }
    }

    fn stream_failed(&self, video: &VideoId) {
        self.slots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(video.as_str());
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
    /// Primary resolves that failed in a row.
    misses: AtomicU32,
    /// The primary is not asked before this moment.
    paused_until: Mutex<Option<Instant>>,
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
            misses: AtomicU32::new(0),
            paused_until: Mutex::new(None),
        }
    }

    fn use_primary(&self, video: &VideoId) -> bool {
        let paused = self
            .paused_until
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some_and(|until| Instant::now() < until);
        !paused
            && self.failures.load(Ordering::SeqCst) < PRIMARY_FAILURE_LIMIT
            && !self
                .skip_primary
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contains(video.as_str())
    }

    /// Count a failed primary resolve; after [`PRIMARY_MISS_LIMIT`] in a row the primary rests
    /// for [`PRIMARY_PAUSE`] (YouTube's bot check refuses anonymous player requests for a while).
    fn primary_missed(&self, missed: bool) {
        if !missed {
            self.misses.store(0, Ordering::SeqCst);
            return;
        }
        if self.misses.fetch_add(1, Ordering::SeqCst) + 1 >= PRIMARY_MISS_LIMIT {
            self.misses.store(0, Ordering::SeqCst);
            *self
                .paused_until
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some(Instant::now() + PRIMARY_PAUSE);
            tracing::info!("native resolver paused after repeated refusals");
        }
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

impl StreamResolver for FallbackResolver {
    fn resolve<'a>(&'a self, video: &'a VideoId) -> BoxFuture<'a, Answer> {
        Box::pin(async move {
            if self.use_primary(video) {
                match self.primary.resolve(video).await {
                    Ok(stream) => {
                        self.primary_missed(false);
                        self.mark(video, true);
                        return Ok(stream);
                    }
                    Err(e) => {
                        self.primary_missed(true);
                        tracing::debug!(error = %e, "native resolve failed, using yt-dlp");
                    }
                }
            }
            let answer = self.fallback.resolve(video).await;
            self.mark(video, false);
            answer
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
    let inner: Arc<dyn StreamResolver> =
        match InnertubeResolver::new(handle.clone(), NATIVE_PROFILES) {
            Ok(native) => Arc::new(FallbackResolver::new(
                Arc::new(native.with_allowed_codecs(allowed)),
                Arc::new(ytdlp),
            )),
            Err(e) => {
                tracing::warn!(error = %e, "native resolver disabled");
                Arc::new(ytdlp)
            }
        };
    Arc::new(CachedResolver::new(inner, handle.clone()))
}

#[cfg(test)]
mod tests;
