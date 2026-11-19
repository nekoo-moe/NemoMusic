//! Which regions want a skeleton (skeleton-loading spec). The rules live here, one line per
//! region: loading, nothing cached, and on screen. [`crate::skeleton`] holds the timing.

use super::App;
use crate::explore::ContentFilter;
use crate::route::Route;
use crate::skeleton::Region;

impl App {
    /// Tell each skeleton gate whether its region wants a skeleton now. Cheap; runs after every
    /// update and frame. A region is wanted only on its own page.
    pub(super) fn sync_skeletons(&mut self) {
        let reduced = self.ui.reduced_motion;
        let route = self.ui.route();
        let wants = [
            (Region::Detail, self.detail_loading()),
            (
                Region::Search,
                matches!(route, Route::Search(_)) && self.search_pending(),
            ),
            (
                Region::Home,
                matches!(route, Route::Home) && self.home_pending(),
            ),
            (
                Region::Explore,
                matches!(route, Route::Explore) && self.explore_pending(),
            ),
        ];
        for (region, wanted) in wants {
            self.skeletons.set(region, wanted, reduced);
        }
    }

    /// The answer of the current search text is on its way and nothing of it is on screen: no
    /// earlier answer (All) or no answer for the picked chip. An error or an empty answer is
    /// not pending.
    pub(crate) fn search_pending(&self) -> bool {
        let s = &self.ui.search;
        self.ui.youtube_enabled()
            && !s.query().is_empty()
            && match s.filter {
                None => s.results.is_none() && s.error.is_none(),
                Some(_) => s.filtered_now().is_none(),
            }
    }

    /// `true` when Home has something of its own to show: shelves, daily mixes, recent songs or
    /// local albums.
    pub(crate) fn home_cached(&self) -> bool {
        !(self.home.shelves.is_empty()
            && self.daily_sections.is_empty()
            && self.ui.recent.is_empty()
            && self.local_albums.is_empty())
    }

    /// Home waits for its first data: loading (the first load, or a reload after a sign-in) and
    /// nothing cached. The kind pages of Videos and Podcasts have their own state.
    pub(crate) fn home_pending(&self) -> bool {
        self.ui.youtube_enabled()
            && matches!(
                self.content_filter,
                ContentFilter::All | ContentFilter::Music
            )
            && (self.home.phase != super::model::HomePhase::Done || self.refresh.busy())
            && !self.home_cached()
    }

    /// Explore has no answer yet and has not failed.
    pub(crate) fn explore_pending(&self) -> bool {
        self.ui.youtube_enabled() && self.explore.data().is_none() && !self.explore.shows_fallback()
    }
}
