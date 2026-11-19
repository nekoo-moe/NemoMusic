//! Which regions want a skeleton (skeleton-loading spec). The rules live here, one line per
//! region: loading, nothing cached, and on screen. [`crate::skeleton`] holds the timing.

use super::App;
use crate::explore::{ContentFilter, LibTab};
use crate::layout;
use crate::route::Route;
use crate::skeleton::Region;

/// Narrowest sidebar that shows the library list (below it only icons show).
const SIDEBAR_WIDE: u32 = 200;

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
            (
                Region::Library,
                matches!(route, Route::Library)
                    && if self.source == super::model::Source::SoundCloud {
                        self.sc_library_pending()
                    } else {
                        self.library_pending()
                    },
            ),
            (Region::Sidebar, self.sidebar_pending()),
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
        (self.ui.youtube_enabled() || self.search_is_soundcloud())
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

    /// The account library is on its way: signed in, none on screen (not even the Home shelves
    /// that stand in for it), and no failure. A signed-out account has nothing to load; a
    /// failure and an empty library have their own sentences.
    pub(crate) fn library_loading(&self) -> bool {
        self.signed_in() && !self.library_failed && self.account_lists().is_none()
    }

    /// The open Library tab waits for the account library with nothing of its own to show
    /// (local files are cached data: the Local tab never waits, Albums shows local albums).
    pub(crate) fn library_pending(&self) -> bool {
        self.library_loading()
            && match self.lib_tab {
                LibTab::Local => false,
                LibTab::Albums => self.local_albums.is_empty(),
                _ => true,
            }
    }

    /// The sidebar list waits for the account library and has no rows (local albums would be
    /// rows) while the list is on screen: wide sidebar, not full screen, not the mini player.
    pub(crate) fn sidebar_pending(&self) -> bool {
        self.library_loading()
            && self.local_albums.is_empty()
            && !self.mini
            && !self.fullscreen
            && !matches!(self.ui.route(), Route::NowPlaying)
            && layout::left_width(self.window_px().0) >= SIDEBAR_WIDE
    }

    /// Explore has no answer yet and has not failed.
    pub(crate) fn explore_pending(&self) -> bool {
        self.ui.youtube_enabled() && self.explore.data().is_none() && !self.explore.shows_fallback()
    }
}
