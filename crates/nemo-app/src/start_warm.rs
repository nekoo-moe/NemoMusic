//! Resolve ahead at start-up (cold-start spec): the song that played last and the first songs
//! of Home, so the first click finds its stream ready. At most [`LIMIT`] songs per run, one at
//! a time (the resolver takes hints one at a time and a little apart), and never a list.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use nemo_core::catalog::{FeedItem, HomeFeed};
use nemo_core::hooks::{PlayHookEvent, PlayObserver};
use nemo_core::stream::StreamResolver;
use nemo_core::track::VideoId;
use nemo_ytm::resolver::StreamStore;

/// Most songs resolved ahead at start-up, the last played one included.
pub const LIMIT: usize = 3;

/// The start-up budget of resolve-ahead hints.
pub struct StartupWarm {
    resolver: Arc<dyn StreamResolver>,
    left: AtomicUsize,
}

/// Where the budget goes once the resolver exists; the catalog is built before it.
pub type WarmSlot = Arc<OnceLock<Arc<StartupWarm>>>;

impl StartupWarm {
    /// A budget of [`LIMIT`] hints for `resolver`.
    #[must_use]
    pub fn new(resolver: Arc<dyn StreamResolver>) -> Arc<Self> {
        Arc::new(Self {
            resolver,
            left: AtomicUsize::new(LIMIT),
        })
    }

    /// Hint `videos` in order, as many as the budget still allows.
    pub fn hint(&self, videos: impl IntoIterator<Item = VideoId>) {
        for video in videos {
            let taken = self
                .left
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1));
            if taken.is_err() {
                return;
            }
            self.resolver.warm(&video);
        }
    }

    /// Hint the first songs of the Home feed: shelf by shelf, in the order shown.
    pub fn hint_home(&self, feed: &HomeFeed) {
        self.hint(home_videos(feed, LIMIT));
    }
}

/// The video ids of the first `limit` songs and videos of `feed`, in the order shown.
#[must_use]
pub fn home_videos(feed: &HomeFeed, limit: usize) -> Vec<VideoId> {
    feed.shelves
        .iter()
        .flat_map(|shelf| shelf.items.iter())
        .filter_map(|item| match item {
            FeedItem::Song(track) | FeedItem::Video(track) => track.video_id.clone(),
            _ => None,
        })
        .take(limit)
        .collect()
}

/// Records the song that plays, for the next start-up.
pub struct LastPlayed(pub Arc<StreamStore>);

impl PlayObserver for LastPlayed {
    fn on_event(&self, event: &PlayHookEvent) {
        if let PlayHookEvent::TrackStarted { track, .. } = event
            && track.local_path.is_none()
            && let Some(video) = &track.video_id
        {
            self.0.set_last_played(video);
        }
    }
}

#[cfg(test)]
mod tests;
