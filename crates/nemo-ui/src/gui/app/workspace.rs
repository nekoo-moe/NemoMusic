//! The listening area as the app runs it (source-workspaces spec): which one is active, the one
//! press that switches it, what each area remembers, and the way back to YouTube Music when
//! SoundCloud stops being possible. Playback is never touched: the queue, the player bar, Now
//! Playing and the lyrics go on across a switch.

use nemo_core::source_account::SourceAccountState;

use super::App;
use super::message::Job;
use crate::explore::LibSort;
use crate::route::{Nav, NavEntry};
use crate::search::SearchState;
use crate::settings::SettingChange;
use crate::state::Event;
use crate::workspace::Source;

/// What the area that is not active keeps for when it comes back.
#[derive(Debug, Clone, Default)]
pub(crate) struct Remembered {
    /// Its search box, answers and chip.
    pub search: SearchState,
    /// The sort order of its Library.
    pub sort: LibSort,
}

impl App {
    /// `true` when `source` can be the active area now: YouTube Music always; SoundCloud while it
    /// is on, running and someone is signed in.
    #[must_use]
    pub fn source_offered(&self, source: Source) -> bool {
        match source {
            Source::YouTube => true,
            Source::SoundCloud => self.sc_is_signed_in(),
        }
    }

    /// SoundCloud may stay the active area: signed in, or a saved session is being looked up
    /// (the first seconds of a start without a cached name).
    fn sc_area_holds(&self) -> bool {
        matches!(
            self.sc_state(),
            Some(SourceAccountState::SignedIn(_) | SourceAccountState::SigningIn)
        )
    }

    /// The sidebar entry to light: the one the person came from, if this area offers it. A page
    /// of the other service (opened from Now Playing) lights nothing, because it is in neither
    /// entry's list.
    #[must_use]
    pub fn nav_selected(&self) -> Option<NavEntry> {
        let route = self.ui.route();
        if self.source.is_foreign(route) {
            return None;
        }
        self.ui
            .nav
            .active_entry()
            .filter(|e| self.source.nav_index(*e).is_some())
    }

    /// The start-up area: the saved one when it can run, else YouTube Music.
    pub(super) fn start_source(&mut self) {
        let saved = self.ui.settings.listening_area;
        self.source = if saved == Source::SoundCloud && self.sc_area_holds() {
            Source::SoundCloud
        } else {
            Source::YouTube
        };
        if self.source == Source::SoundCloud {
            self.ui.nav = Nav::new(self.source.landing());
        }
        if self.source != saved {
            // The saved area cannot run: store the way back, so the next start agrees.
            let _ = self.dispatch(Event::Setting(SettingChange::ListeningArea(self.source)));
        }
    }

    /// Press on the switch. Nothing happens for the active area or one that is not offered.
    pub(super) fn switch_source(&mut self, source: Source) -> Vec<Job> {
        if source == self.source || !self.source_offered(source) {
            return Vec::new();
        }
        self.apply_source(source)
    }

    /// Leave SoundCloud when it can no longer be the area (switched off, signed out, session
    /// ended): back to YouTube Music at once, with no question. Cheap; runs after every update.
    pub(super) fn watch_source(&mut self) -> Vec<Job> {
        if self.source == Source::SoundCloud && !self.sc_area_holds() {
            return self.apply_source(Source::YouTube);
        }
        Vec::new()
    }

    /// Make `source` the active area: swap what each area remembers, start its history on its
    /// landing page, store the choice, and read what its screens wait for.
    fn apply_source(&mut self, source: Source) -> Vec<Job> {
        std::mem::swap(&mut self.ui.search, &mut self.rest.search);
        std::mem::swap(&mut self.lib_sort, &mut self.rest.sort);
        self.source = source;
        // History belongs to the area: Back never lands on a page of the other one.
        self.ui.nav = Nav::new(source.landing());
        self.ui.text_focused = false;
        self.ui.menu = None;
        self.sort_menu = None;
        self.rows_limit = super::ROWS_STEP;
        self.anim.restart_page();
        let mut jobs = self.dispatch(Event::Setting(SettingChange::ListeningArea(source)));
        jobs.extend(self.sc_watch());
        jobs
    }
}

#[cfg(test)]
mod tests;
