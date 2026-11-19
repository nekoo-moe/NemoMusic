//! Put a page back to "waiting for its first data" in a headless app, for the skeleton shots
//! (skeleton-loading spec). The app is built and settled first, so every page is full; this
//! empties the one under test and marks it as loading.

use super::super::app::{App, HomePhase};
use crate::explore_feed::ExploreFeed;
use crate::refresh::Owner;

/// A page that waits for its first data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pending {
    /// Search: the answer of the current text (and of the picked chip) is on its way.
    Search,
    /// Home: nothing cached, the first load runs.
    Home,
    /// Explore: no answer yet, the first load runs.
    Explore,
}

impl Pending {
    /// Apply it to `app`.
    pub(crate) fn apply(self, app: &mut App) {
        match self {
            Self::Search => {
                let s = &mut app.ui.search;
                s.results = None;
                s.filtered = None;
                s.answered = None;
                s.error = None;
                s.loading = true;
            }
            Self::Home => {
                app.home.shelves.clear();
                app.home.phase = HomePhase::Loading;
                app.daily_sections.clear();
                app.ui.recent.clear();
                app.local_albums.clear();
            }
            Self::Explore => {
                app.explore = ExploreFeed::default();
                app.explore.start(Owner::SignedOut);
            }
        }
    }
}
