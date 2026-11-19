//! Resolve ahead of the click (cold-start spec): the UI says which song the pointer rests on
//! (or went down on), and the resolver starts that one song in the background.
//!
//! One hint at a time: a new song replaces the running hint, which cancels it; `None` (the
//! pointer left) cancels it. Never a list: the UI names one song per call.

use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use nemo_core::track::VideoId;
use nemo_ytm::resolver::{CachedResolver, ResolveHint};

/// What the UI hook calls.
pub type HintFn = Arc<dyn Fn(Option<VideoId>) + Send + Sync>;

/// Where the hint goes once the services exist (the window opens first).
pub type HintSlot = Arc<OnceLock<Arc<HoverHint>>>;

/// The one running hint.
pub struct HoverHint {
    resolver: Arc<CachedResolver>,
    running: Mutex<Option<(VideoId, ResolveHint)>>,
}

impl HoverHint {
    /// Hints for `resolver`.
    #[must_use]
    pub fn new(resolver: Arc<CachedResolver>) -> Arc<Self> {
        Arc::new(Self {
            resolver,
            running: Mutex::new(None),
        })
    }

    /// `Some(video)`: resolve that song now (the same song again changes nothing); `None`:
    /// cancel the running hint.
    pub fn tell(&self, video: Option<VideoId>) {
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        match video {
            Some(video) if running.as_ref().is_some_and(|(v, _)| *v == video) => {}
            Some(video) => {
                // The old hint is dropped (cancelled) before the new one starts.
                *running = None;
                let hint = self.resolver.hint(&video);
                *running = Some((video, hint));
            }
            None => *running = None,
        }
    }
}

/// The UI hook: tells the hint in `slot`, once it exists; does nothing before.
#[must_use]
pub fn hook(slot: HintSlot) -> HintFn {
    Arc::new(move |video| {
        if let Some(hint) = slot.get() {
            hint.tell(video);
        }
    })
}

#[cfg(test)]
mod tests;
