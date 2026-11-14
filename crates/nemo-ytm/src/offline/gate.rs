//! Offline mode: the shared switch and the gates that refuse requests while it is on
//! (criteria 4 and 5).
//!
//! [`OfflineSwitch`] is the app's [`NetworkMode`]. The gates wrap the stream resolver, the
//! stream opener, the segment source and the image source; while offline mode is on they answer
//! at once with an error and never call the service inside, so no request reaches a YouTube
//! host. The switch turns itself on after two network failures of user actions within
//! [`FAILURE_WINDOW`], with one notice, and only the user turns it off. Nothing here probes the
//! network.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use nemo_core::BoxFuture;
use nemo_core::downloads::NetworkMode;
use nemo_core::segment::{Segment, SegmentError, SegmentSource};
use nemo_core::services::{ByteSource, ImageError, ImageSource, StreamOpener};
use nemo_core::stream::{ResolvedStream, StreamError, StreamRequest, StreamResolver};
use nemo_core::track::{Artwork, VideoId};

use super::Clock;

/// Two failures this close together switch offline mode on.
pub const FAILURE_WINDOW: Duration = Duration::from_secs(30);

/// The error text of every refused request.
const OFFLINE: &str = "offline mode is on";

/// The app-wide offline switch.
pub struct OfflineSwitch {
    offline: AtomicBool,
    notice: AtomicBool,
    /// When the last counted failure happened, in clock seconds.
    last_failure: Mutex<Option<i64>>,
    clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for OfflineSwitch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OfflineSwitch")
            .field("offline", &self.is_offline())
            .finish_non_exhaustive()
    }
}

impl OfflineSwitch {
    /// A switch that starts online. `clock` times the failure window.
    #[must_use]
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            offline: AtomicBool::new(false),
            notice: AtomicBool::new(false),
            last_failure: Mutex::new(None),
            clock,
        }
    }

    /// A user action failed for lack of network. The second failure within [`FAILURE_WINDOW`]
    /// turns offline mode on and raises the notice once; later failures change nothing.
    pub fn record_failure(&self) {
        if self.is_offline() {
            return;
        }
        let now = self.clock.now_secs();
        let window = i64::try_from(FAILURE_WINDOW.as_secs()).unwrap_or(i64::MAX);
        let mut last = self
            .last_failure
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let switch = last.is_some_and(|at| now.saturating_sub(at) <= window);
        if switch {
            *last = None;
            self.offline.store(true, Ordering::SeqCst);
            self.notice.store(true, Ordering::SeqCst);
            tracing::info!("offline mode switched on after repeated network failures");
        } else {
            *last = Some(now);
        }
    }

    /// `true` once after offline mode switched itself on, so the UI says so once.
    pub fn take_notice(&self) -> bool {
        self.notice.swap(false, Ordering::SeqCst)
    }
}

impl NetworkMode for OfflineSwitch {
    fn is_offline(&self) -> bool {
        self.offline.load(Ordering::SeqCst)
    }

    fn set_offline(&self, on: bool) {
        self.offline.store(on, Ordering::SeqCst);
        if !on {
            *self
                .last_failure
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = None;
        }
    }
}

/// The stream resolver behind the offline gate. Network failures of a resolve count toward the
/// automatic switch (a resolve is a user action: playing a song).
pub struct GatedResolver {
    inner: Arc<dyn StreamResolver>,
    switch: Arc<OfflineSwitch>,
}

impl GatedResolver {
    /// Gate `inner` with `switch`.
    #[must_use]
    pub fn new(inner: Arc<dyn StreamResolver>, switch: Arc<OfflineSwitch>) -> Self {
        Self { inner, switch }
    }

    async fn counted(
        &self,
        answer: BoxFuture<'_, Result<ResolvedStream, StreamError>>,
    ) -> Result<ResolvedStream, StreamError> {
        let answer = answer.await;
        if matches!(answer, Err(StreamError::Network(_))) {
            self.switch.record_failure();
        }
        answer
    }
}

impl StreamResolver for GatedResolver {
    fn resolve<'a>(
        &'a self,
        video: &'a VideoId,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        if self.switch.is_offline() {
            return Box::pin(async { Err(StreamError::Network(OFFLINE.to_owned())) });
        }
        Box::pin(self.counted(self.inner.resolve(video)))
    }

    fn resolve_with<'a>(
        &'a self,
        video: &'a VideoId,
        request: &'a StreamRequest,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        if self.switch.is_offline() {
            return Box::pin(async { Err(StreamError::Network(OFFLINE.to_owned())) });
        }
        Box::pin(self.counted(self.inner.resolve_with(video, request)))
    }

    fn warm(&self, video: &VideoId) {
        if !self.switch.is_offline() {
            self.inner.warm(video);
        }
    }

    fn stream_failed(&self, video: &VideoId) {
        self.inner.stream_failed(video);
    }
}

/// The stream opener behind the offline gate. Network failures count toward the switch.
pub struct GatedOpener {
    inner: Arc<dyn StreamOpener>,
    switch: Arc<OfflineSwitch>,
}

impl GatedOpener {
    /// Gate `inner` with `switch`.
    #[must_use]
    pub fn new(inner: Arc<dyn StreamOpener>, switch: Arc<OfflineSwitch>) -> Self {
        Self { inner, switch }
    }
}

impl StreamOpener for GatedOpener {
    fn open(&self, stream: &ResolvedStream) -> Result<Box<dyn ByteSource>, StreamError> {
        if self.switch.is_offline() {
            return Err(StreamError::Network(OFFLINE.to_owned()));
        }
        let opened = self.inner.open(stream);
        if matches!(opened, Err(StreamError::Network(_))) {
            self.switch.record_failure();
        }
        opened
    }
}

/// The segment source behind the offline gate. Lookups run in the background, so their
/// failures do not count toward the switch.
pub struct GatedSegments {
    inner: Arc<dyn SegmentSource>,
    switch: Arc<OfflineSwitch>,
}

impl GatedSegments {
    /// Gate `inner` with `switch`.
    #[must_use]
    pub fn new(inner: Arc<dyn SegmentSource>, switch: Arc<OfflineSwitch>) -> Self {
        Self { inner, switch }
    }
}

impl SegmentSource for GatedSegments {
    fn segments<'a>(
        &'a self,
        video: &'a VideoId,
        duration: Duration,
    ) -> BoxFuture<'a, Result<Vec<Segment>, SegmentError>> {
        if self.switch.is_offline() {
            return Box::pin(async { Err(SegmentError::Network(OFFLINE.to_owned())) });
        }
        self.inner.segments(video, duration)
    }
}

/// The image source behind the offline gate. Cover loads follow scrolling, so their failures
/// do not count toward the switch.
pub struct GatedImages {
    inner: Arc<dyn ImageSource>,
    switch: Arc<OfflineSwitch>,
}

impl GatedImages {
    /// Gate `inner` with `switch`.
    #[must_use]
    pub fn new(inner: Arc<dyn ImageSource>, switch: Arc<OfflineSwitch>) -> Self {
        Self { inner, switch }
    }
}

impl ImageSource for GatedImages {
    fn image<'a>(
        &'a self,
        artwork: &'a Artwork,
        target_px: u32,
    ) -> BoxFuture<'a, Result<Vec<u8>, ImageError>> {
        if self.switch.is_offline() {
            return Box::pin(async { Err(ImageError::Network(OFFLINE.to_owned())) });
        }
        self.inner.image(artwork, target_px)
    }
}
