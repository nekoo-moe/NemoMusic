//! What the Explore page shows (real-explore spec): YouTube Music's own Explore answer (new
//! releases, charts, moods and genres, new music videos), the mood and genre pages opened from
//! it, and when the page falls back to the seed shelves. Pure.
//!
//! The last good answer stays on screen when a later load fails; the seed shelves are shown only
//! when Explore never loaded.

use std::collections::HashMap;
use std::sync::Arc;

use nemo_core::catalog::{BrowseId, Explore, FeedItem, FeedShelf};
use nemo_core::track::Track;

use crate::daily::DailyData;
use crate::refresh::Owner;

/// The one line the page shows when Explore did not load and nothing loaded before.
pub const FALLBACK_NOTICE: &str = "Explore did not load. Showing a few picks instead.";
/// The one line the page shows when a reload failed and the last answer stays on screen.
pub const STALE_NOTICE: &str = "Could not refresh Explore. Showing what loaded before.";
/// The error kept for an answer with nothing in it.
pub const EMPTY_ANSWER: &str = "YouTube Music sent an empty Explore page";
/// Most mood pages kept in memory.
pub const MOOD_PAGES_MAX: usize = 16;

/// Progress of the Explore answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExplorePhase {
    /// Not asked yet (waiting for the first-run notice).
    #[default]
    Idle,
    /// A request runs.
    Loading,
    /// The last request answered with content.
    Ready,
    /// The last request failed or answered nothing.
    Failed,
}

/// The Explore answer and its loading state.
#[derive(Debug, Clone, Default)]
pub struct ExploreFeed {
    data: Option<Arc<Explore>>,
    phase: ExplorePhase,
    error: Option<String>,
    /// Whose session the last request ran for; a new owner asks again.
    asked_for: Option<Owner>,
}

impl ExploreFeed {
    /// `true` when a request should start for `owner`: never asked for this account (or for
    /// nobody signed in), and none running. A pending sign-in waits.
    #[must_use]
    pub fn wants(&self, owner: &Owner) -> bool {
        *owner != Owner::Pending
            && self.phase != ExplorePhase::Loading
            && self.asked_for.as_ref() != Some(owner)
    }

    /// A request starts for `owner`.
    pub fn start(&mut self, owner: Owner) {
        self.asked_for = Some(owner);
        self.phase = ExplorePhase::Loading;
    }

    /// The request answered. An answer with nothing in it counts as a failure; a failure keeps
    /// the last good answer.
    pub fn finish(&mut self, result: Result<Arc<Explore>, String>) {
        match result {
            Ok(e) if has_content(&e) => {
                self.data = Some(e);
                self.phase = ExplorePhase::Ready;
                self.error = None;
            }
            Ok(_) => self.fail(EMPTY_ANSWER.to_owned()),
            Err(e) => self.fail(e),
        }
    }

    fn fail(&mut self, error: String) {
        self.phase = ExplorePhase::Failed;
        self.error = Some(error);
    }

    /// Ask again on the next update (Retry, Refresh). Does nothing while a request runs.
    pub fn reload(&mut self) {
        if self.phase != ExplorePhase::Loading {
            self.asked_for = None;
        }
    }

    /// The last good answer.
    #[must_use]
    pub fn data(&self) -> Option<&Explore> {
        self.data.as_deref()
    }

    /// Loading progress.
    #[must_use]
    pub fn phase(&self) -> ExplorePhase {
        self.phase
    }

    /// Why the last request failed, if it did.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// `true` when the page shows the seed shelves: the last request failed and no answer ever
    /// arrived.
    #[must_use]
    pub fn shows_fallback(&self) -> bool {
        self.data.is_none() && self.phase == ExplorePhase::Failed
    }

    /// The one short line for the page, if any.
    #[must_use]
    pub fn notice(&self) -> Option<&'static str> {
        match (self.phase, self.data.is_some()) {
            (ExplorePhase::Failed, true) => Some(STALE_NOTICE),
            (ExplorePhase::Failed, false) => Some(FALLBACK_NOTICE),
            _ => None,
        }
    }

    /// The shelves listed under the moods, charts first; `n` of [`Self::shelf`] counts in this
    /// order.
    pub fn shelves(&self) -> impl Iterator<Item = &FeedShelf> {
        self.data
            .iter()
            .flat_map(|e| e.charts.iter().chain(e.shelves.iter()))
    }

    /// Shelf `n` of [`Self::shelves`].
    #[must_use]
    pub fn shelf(&self, n: usize) -> Option<&FeedShelf> {
        self.shelves().nth(n)
    }

    /// Put the new releases and the moods of the last answer into the daily inputs. Moods are
    /// replaced only when the answer has some, so the home-feed chips stay otherwise.
    pub fn fill_daily(&self, daily: &mut DailyData) {
        let Some(e) = self.data() else { return };
        daily.new_releases.clone_from(&e.new_releases);
        if !e.moods.is_empty() {
            daily.moods.clone_from(&e.moods);
        }
    }
}

/// `true` when an answer has anything the page can show.
#[must_use]
pub fn has_content(e: &Explore) -> bool {
    !(e.new_releases.is_empty()
        && e.moods.is_empty()
        && e.charts.iter().all(|s| s.items.is_empty())
        && e.shelves.iter().all(|s| s.items.is_empty()))
}

/// The songs and music videos of a shelf, in order: the queue its play buttons start.
#[must_use]
pub fn shelf_tracks(shelf: &FeedShelf) -> Vec<Track> {
    shelf
        .items
        .iter()
        .filter_map(|item| match item {
            FeedItem::Song(t) | FeedItem::Video(t) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

/// State of one mood or genre page.
#[derive(Debug, Clone)]
pub enum MoodPage {
    /// A request runs.
    Loading,
    /// Its shelves.
    Ready(Arc<Vec<FeedShelf>>),
    /// The request failed (short message).
    Failed(String),
}

/// The mood and genre pages opened in this session, at most [`MOOD_PAGES_MAX`].
#[derive(Debug, Clone, Default)]
pub struct MoodPages {
    pages: HashMap<BrowseId, MoodPage>,
    /// Ids in the order they were asked for, oldest first.
    order: Vec<BrowseId>,
}

impl MoodPages {
    /// `true` when page `id` was never asked for (or was forgotten).
    #[must_use]
    pub fn wants(&self, id: &BrowseId) -> bool {
        !self.pages.contains_key(id)
    }

    /// A request for `id` starts. The oldest page is dropped beyond [`MOOD_PAGES_MAX`].
    pub fn start(&mut self, id: BrowseId) {
        if self.order.len() >= MOOD_PAGES_MAX && !self.order.is_empty() {
            let old = self.order.remove(0);
            self.pages.remove(&old);
        }
        self.order.push(id.clone());
        self.pages.insert(id, MoodPage::Loading);
    }

    /// Page `id` answered. An answer for a page that was dropped meanwhile is ignored. An answer
    /// with no shelf counts as a failure.
    pub fn finish(&mut self, id: &BrowseId, result: Result<Arc<Vec<FeedShelf>>, String>) {
        let Some(page) = self.pages.get_mut(id) else {
            return;
        };
        *page = match result {
            Ok(shelves) if shelves.iter().any(|s| !s.items.is_empty()) => MoodPage::Ready(shelves),
            Ok(_) => MoodPage::Failed("YouTube Music sent an empty page".to_owned()),
            Err(e) => MoodPage::Failed(e),
        };
    }

    /// Forget page `id`, so the next update asks for it again (Retry).
    pub fn forget(&mut self, id: &BrowseId) {
        self.pages.remove(id);
        self.order.retain(|k| k != id);
    }

    /// Page `id`, if asked for.
    #[must_use]
    pub fn get(&self, id: &BrowseId) -> Option<&MoodPage> {
        self.pages.get(id)
    }

    /// Shelf `n` of page `id`, when it is loaded.
    #[must_use]
    pub fn shelf(&self, id: &BrowseId, n: usize) -> Option<&FeedShelf> {
        match self.pages.get(id) {
            Some(MoodPage::Ready(shelves)) => shelves.get(n),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
