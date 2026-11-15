//! Reloading the user's music data for [`App`] (library-refresh spec). [`crate::refresh`]
//! decides what runs; this file starts the steps and applies their answers.

use super::App;
use super::message::{AccountBundle, Job, RefreshData};
use super::model::HomePhase;
use crate::refresh::{Answer, FAILED_NOTICE, Owner, Step};
use crate::state::Event;
use std::sync::Arc;

impl App {
    /// Start a reload when the account changed since the data on screen was loaded: a sign-in,
    /// a switch to another account, or a sign-out. Cheap; runs after every update and frame.
    pub(super) fn watch_account(&mut self) -> Vec<Job> {
        let now = Owner::of(&self.backend.account.state());
        if now != Owner::Pending && self.refresh.owner().is_some_and(|seen| *seen != now) {
            // Likes changed in this session belong to the account before.
            self.likes.clear();
        }
        self.refresh
            .observe(now)
            .map(Job::Refresh)
            .into_iter()
            .collect()
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
                self.daily_gen += 1;
                if self.signed_in() {
                    // Set even when empty: another account may simply have nothing saved.
                    self.account_library = Some(Arc::new(bundle.library.clone()));
                }
            }
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
