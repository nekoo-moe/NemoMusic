//! Reloading the user's music data for [`App`] (library-refresh spec). [`crate::refresh`]
//! decides what runs; this file starts the steps and applies their answers.

use super::App;
use super::message::{AccountBundle, Job, RefreshData};
use super::model::{HomePhase, Source};
use crate::refresh::{Answer, FAILED_NOTICE, Owner, Step};
use crate::state::Event;
use std::sync::Arc;

impl App {
    /// Start a reload when the account changed since the data on screen was loaded: a sign-in,
    /// a switch to another account, or a sign-out. Cheap; runs after every update and frame.
    pub(super) fn watch_account(&mut self) -> Vec<Job> {
        let now = Owner::of(&self.backend.account.state());
        if now != Owner::Pending && self.refresh.owner().is_some_and(|seen| *seen != now) {
            // The library and the likes changed in this session belong to the account before:
            // the sidebar never shows another account's playlists, even when the reload fails.
            self.likes.clear();
            self.account_library = None;
            self.library_failed = false;
        }
        self.refresh
            .observe(now)
            .map(Job::Refresh)
            .into_iter()
            .collect()
    }

    /// The Refresh button, `F5` and `Ctrl+R`. A press while a reload runs does nothing.
    pub(super) fn refresh_msg(&mut self) -> Vec<Job> {
        // Only the active area reloads (source-workspaces).
        if self.source == Source::SoundCloud {
            return self.sc_reload();
        }
        if !self.ui.youtube_enabled() {
            return Vec::new();
        }
        let jobs: Vec<Job> = self
            .refresh
            .request()
            .map(Job::Refresh)
            .into_iter()
            .collect();
        if !jobs.is_empty() {
            // Explore reloads beside it, on its own (real-explore spec).
            self.explore.reload();
        }
        jobs
    }

    /// A reload step answered: use it when it still counts, then start the next step. A
    /// failure keeps what is on screen; the run ends with one short notice.
    pub(super) fn on_refreshed(&mut self, generation: u64, data: RefreshData) -> Vec<Job> {
        // An account change that came with this answer makes it stale before it is used.
        let mut jobs = self.watch_account();
        let (step, ok) = match &data {
            RefreshData::Home(result) => (Step::Home, result.is_ok()),
            RefreshData::Account(bundle) => {
                (Step::Account, !bundle_failed(bundle, self.signed_in()))
            }
        };
        match self.refresh.answered(generation, step, ok) {
            Answer::Drop { next } => jobs.extend(next.map(Job::Refresh)),
            Answer::Use { next, ended } => {
                if ok {
                    self.apply_refresh(data);
                } else if step == Step::Account && self.signed_in() {
                    // The library that was there stays; with none, the sidebar says it failed.
                    self.library_failed = self.account_library.is_none();
                }
                jobs.extend(next.map(Job::Refresh));
                if ended == Some(true) {
                    jobs.extend(self.dispatch(Event::Notice(FAILED_NOTICE.to_owned())));
                }
            }
        }
        jobs
    }

    fn apply_refresh(&mut self, data: RefreshData) {
        match data {
            RefreshData::Home(Ok(Some(shelves))) if !shelves.is_empty() => {
                self.home.shelves.clone_from(shelves.as_ref());
                self.home.phase = HomePhase::Done;
                self.daily_gen += 1;
            }
            // No home page, or an empty one: keep the shelves there are.
            RefreshData::Home(_) => {}
            RefreshData::Account(bundle) => {
                self.daily = bundle.daily.clone();
                self.explore.fill_daily(&mut self.daily);
                self.daily_gen += 1;
                if self.signed_in() {
                    self.set_account_library(&bundle);
                } else {
                    self.account_library = None;
                }
            }
        }
    }

    /// Keep the library of an account bundle. Set even when empty (an account may have nothing
    /// saved); a bundle where no account call answered leaves what is there.
    pub(super) fn set_account_library(&mut self, bundle: &AccountBundle) {
        if bundle.account_answered {
            self.account_library = Some(Arc::new(bundle.library.clone()));
            self.library_failed = false;
        } else {
            self.library_failed = self.account_library.is_none();
        }
    }
}

/// `true` when a bundle brought nothing to use: no account call answered while signed in, or
/// no data at all while signed out. The data before stays on screen then.
fn bundle_failed(b: &AccountBundle, signed_in: bool) -> bool {
    if signed_in {
        return !b.account_answered;
    }
    let lib = &b.library;
    let day = &b.daily;
    lib.playlists.is_empty()
        && lib.albums.is_empty()
        && lib.artists.is_empty()
        && lib.songs.is_empty()
        && day.liked.is_empty()
        && day.history.is_empty()
        && day.followed.is_empty()
        && day.new_releases.is_empty()
        && day.moods.is_empty()
        && day.videos.is_empty()
        && day.podcasts.is_empty()
        && day.feed_shelves.is_empty()
}

#[cfg(test)]
mod tests;
