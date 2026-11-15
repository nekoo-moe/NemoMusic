//! Loading the Explore page and its mood and genre pages for [`App`] (real-explore spec).
//! [`crate::explore_feed`] decides what to keep; this file starts the jobs and applies answers.

use super::App;
use super::message::{ExploreMsg, Job};
use crate::refresh::Owner;
use crate::route::Route;

impl App {
    /// Ask for the Explore page when it was not asked for this account yet, and for the mood
    /// page of the route when it is not loaded. Nothing leaves the computer before the notice is
    /// accepted.
    pub(super) fn pump_explore(&mut self) -> Vec<Job> {
        let mut jobs = Vec::new();
        if !self.ui.youtube_enabled() {
            return jobs;
        }
        let owner = Owner::of(&self.backend.account.state());
        if self.explore.wants(&owner) {
            self.explore.start(owner);
            jobs.push(Job::Explore);
        }
        if let Route::Mood { id, .. } = self.ui.route()
            && self.moods.wants(id)
        {
            let id = id.clone();
            self.moods.start(id.clone());
            jobs.push(Job::Mood(id));
        }
        jobs
    }

    /// Apply an Explore message. Retries take effect on the update that follows.
    pub(super) fn explore_msg(&mut self, msg: ExploreMsg) -> Vec<Job> {
        match msg {
            ExploreMsg::Loaded(result) => {
                self.explore.finish(result);
                self.explore.fill_daily(&mut self.daily);
                self.daily_gen += 1;
            }
            ExploreMsg::MoodLoaded { id, result } => self.moods.finish(&id, result),
            ExploreMsg::Retry => self.explore.reload(),
            ExploreMsg::RetryMood(id) => self.moods.forget(&id),
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests;
