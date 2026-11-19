//! The SoundCloud library as the app holds it (soundcloud-account criteria 11 and 12): read when
//! the person opens it signed in, again on Refresh, cleared the moment the account changes.
//! Nothing is read in the background: a read starts only while SoundCloud is on, someone is
//! signed in and the Library is set to show SoundCloud.

use std::sync::Arc;

use nemo_core::source_account::SourceAccountState;

use super::App;
use super::message::Job;
use super::model::{ScLibData, ScLibrary, ScMsg, ScPhase, ScTab, Source};
use crate::copy::soundcloud as text;

impl App {
    /// Someone is signed in to SoundCloud (and SoundCloud is on and running).
    #[must_use]
    pub fn sc_is_signed_in(&self) -> bool {
        matches!(self.sc_state(), Some(SourceAccountState::SignedIn(_)))
    }

    /// The SoundCloud library is the one on screen: the Library shows it and it waits for data.
    #[must_use]
    pub(crate) fn sc_library_pending(&self) -> bool {
        self.sc_is_signed_in()
            && self.source == Source::SoundCloud
            && matches!(self.sc_lib.phase, ScPhase::Idle | ScPhase::Loading)
    }

    /// The Search page is set to SoundCloud (and SoundCloud is on): its searches go to SoundCloud
    /// and need no YouTube Music notice.
    #[must_use]
    pub fn search_is_soundcloud(&self) -> bool {
        self.source == Source::SoundCloud && self.sc_open()
    }

    /// Clear the library when the account behind it changed or ended, then start the read the
    /// screen waits for. Cheap; runs after every update and frame.
    pub(super) fn sc_watch(&mut self) -> Vec<Job> {
        if self.soundcloud_info.is_none() {
            return Vec::new();
        }
        let signed = self.sc_is_signed_in();
        if self.sc_lib.owner != signed {
            // Another account, or none: nothing of the earlier one stays, even a failure text.
            self.sc_lib = ScLibrary {
                generation: self.sc_lib.generation + 1,
                owner: signed,
                ..ScLibrary::default()
            };
        }
        if signed && self.source.has_soundcloud() && self.sc_lib.phase == ScPhase::Idle {
            return self.sc_load();
        }
        Vec::new()
    }

    /// Start a read of the library.
    fn sc_load(&mut self) -> Vec<Job> {
        self.sc_lib.phase = ScPhase::Loading;
        self.sc_lib.error = None;
        vec![Job::ScLibrary {
            generation: self.sc_lib.generation,
        }]
    }

    /// Read the library again (Retry, Refresh). A read that runs is not started twice.
    pub(super) fn sc_reload(&mut self) -> Vec<Job> {
        if !self.sc_is_signed_in() || self.sc_lib.phase == ScPhase::Loading {
            return Vec::new();
        }
        self.sc_lib.generation += 1;
        self.sc_load()
    }

    /// The messages of the Library's source switch and SoundCloud tabs.
    pub(super) fn sc_library_msg(&mut self, msg: ScMsg) -> Vec<Job> {
        match msg {
            ScMsg::Source(source) => {
                if self.source == source {
                    return Vec::new();
                }
                self.source = source;
                let mut jobs = self.sc_watch();
                if matches!(self.ui.route(), crate::route::Route::Search(_)) {
                    // The answers on screen belong to the other service.
                    self.ui.search.forget_answers();
                    jobs.extend(self.dispatch(crate::state::Event::SearchRetry));
                }
                return jobs;
            }
            ScMsg::Tab(tab) => self.sc_tab = tab,
            ScMsg::Reload => return self.sc_reload(),
            ScMsg::LibraryLoaded { generation, result } => {
                if generation != self.sc_lib.generation {
                    return Vec::new();
                }
                match result {
                    Ok(data) => {
                        self.sc_lib.data = data;
                        self.sc_lib.phase = ScPhase::Done;
                        self.sc_lib.error = None;
                    }
                    Err(reason) => {
                        // What was read before stays; the page says it could not refresh.
                        self.sc_lib.phase = ScPhase::Failed;
                        self.sc_lib.error = Some(plain(&reason));
                    }
                }
            }
            _ => {}
        }
        Vec::new()
    }

    /// The tab's data, for the screen.
    pub(crate) fn sc_data(&self) -> Arc<ScLibData> {
        Arc::clone(&self.sc_lib.data)
    }

    /// The open tab.
    #[must_use]
    pub fn sc_open_tab(&self) -> ScTab {
        self.sc_tab
    }
}

/// A catalog failure in a sentence that holds no address and no detail.
fn plain(reason: &str) -> String {
    if reason.contains("Sign in") {
        text::EXPIRED.to_owned()
    } else if reason.contains("switched off") {
        "SoundCloud is switched off.".to_owned()
    } else {
        "Could not load your SoundCloud library. Try again in a minute.".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use nemo_core::source_account::SourceAccountState;

    use super::*;
    use crate::gui::app::Message;
    use crate::gui::headless::soundcloud::ScScene;
    use crate::gui::headless::{Scenario, build_app};
    use crate::mock::{MockScAccount, mock_backend_soundcloud};

    fn app_with(state: SourceAccountState) -> (App, nemo_core::services::Backend) {
        let backend = mock_backend_soundcloud(state);
        let app = build_app(
            &backend,
            &Scenario {
                soundcloud: Some(ScScene::Account),
                ..Scenario::default()
            },
        );
        (app, backend)
    }

    fn signed_in() -> SourceAccountState {
        SourceAccountState::SignedIn(MockScAccount::info())
    }

    fn reads(jobs: &[Job]) -> usize {
        jobs.iter()
            .filter(|j| matches!(j, Job::ScLibrary { .. }))
            .count()
    }

    #[test]
    fn nothing_is_read_while_the_library_shows_youtube_music_or_nobody_is_signed_in() {
        let (mut app, _) = app_with(signed_in());
        assert!(app.sc_watch().is_empty(), "YouTube Music is the source");
        assert_eq!(app.sc_lib.phase, ScPhase::Idle);
        let (mut out, _) = app_with(SourceAccountState::SignedOut);
        let jobs = out.update_core(Message::Sc(ScMsg::Source(Source::SoundCloud)));
        assert_eq!(reads(&jobs), 0, "signed out");
    }

    #[test]
    fn choosing_soundcloud_starts_one_read_and_the_answer_fills_the_tabs() {
        let (mut app, _) = app_with(signed_in());
        let jobs = app.update_core(Message::Sc(ScMsg::Source(Source::SoundCloud)));
        assert_eq!(reads(&jobs), 1);
        assert_eq!(app.sc_lib.phase, ScPhase::Loading);
        assert!(app.sc_library_pending());
        // Asking again while it runs does not start a second read.
        assert_eq!(reads(&app.update_core(Message::Sc(ScMsg::Reload))), 0);
        app.settle_blocking(jobs);
        assert_eq!(app.sc_lib.phase, ScPhase::Done);
        assert!(!app.sc_lib.data.likes.is_empty() && !app.sc_lib.data.playlists.is_empty());
        assert!(!app.sc_lib.data.history.is_empty());
        assert!(!app.sc_library_pending());
        // The rows of the tab play from the library: the list source resolves to the tracks.
        assert_eq!(
            app.tracks_of(&crate::gui::app::ListSource::ScLikes).len(),
            app.sc_lib.data.likes.len()
        );
    }

    #[test]
    fn an_answer_to_an_older_read_is_dropped() {
        let (mut app, _) = app_with(signed_in());
        let jobs = app.update_core(Message::Sc(ScMsg::Source(Source::SoundCloud)));
        let old = app.sc_lib.generation;
        app.settle_blocking(jobs);
        let _ = app.update_core(Message::Sc(ScMsg::Reload));
        assert_eq!(app.sc_lib.generation, old + 1);
        let _ = app.update_core(Message::Sc(ScMsg::LibraryLoaded {
            generation: old,
            result: Err("stale".to_owned()),
        }));
        assert_eq!(
            app.sc_lib.phase,
            ScPhase::Loading,
            "the stale failure changed nothing"
        );
    }

    #[test]
    fn a_failed_read_keeps_what_was_there_and_says_so_in_plain_words() {
        let (mut app, _) = app_with(signed_in());
        let jobs = app.update_core(Message::Sc(ScMsg::Source(Source::SoundCloud)));
        app.settle_blocking(jobs);
        let held = app.sc_lib.data.likes.len();
        let _ = app.update_core(Message::Sc(ScMsg::Reload));
        let generation = app.sc_lib.generation;
        let _ = app.update_core(Message::Sc(ScMsg::LibraryLoaded {
            generation,
            result: Err("https://api-v2.soundcloud.com/x?client_id=secret".to_owned()),
        }));
        assert_eq!(app.sc_lib.phase, ScPhase::Failed);
        assert_eq!(app.sc_lib.data.likes.len(), held);
        let text = app.sc_lib.error.clone().unwrap_or_default();
        assert!(!text.contains("secret") && !text.contains("http"), "{text}");
        // Retry starts a read again.
        assert_eq!(reads(&app.update_core(Message::Sc(ScMsg::Reload))), 1);
    }

    #[test]
    fn signing_out_clears_the_library_before_anything_is_shown() {
        let (mut app, backend) = app_with(signed_in());
        let jobs = app.update_core(Message::Sc(ScMsg::Source(Source::SoundCloud)));
        app.settle_blocking(jobs);
        assert!(!app.sc_lib.data.likes.is_empty());
        let before = app.sc_lib.generation;
        // The account signs out (the mock changes state at once).
        if let Some(account) = &backend.soundcloud_account {
            drop(account.sign_out());
        }
        let _ = app.update_core(Message::TimerTick);
        assert!(
            app.sc_lib.data.likes.is_empty(),
            "nothing of the account is left"
        );
        assert_eq!(app.sc_lib.phase, ScPhase::Idle);
        assert!(app.sc_lib.generation > before);
        assert!(!app.sc_lib.owner);
    }

    #[test]
    fn refresh_reloads_soundcloud_only_when_the_library_shows_it() {
        let (mut app, _) = app_with(signed_in());
        let jobs = app.update_core(Message::Sc(ScMsg::Source(Source::SoundCloud)));
        app.settle_blocking(jobs);
        assert_eq!(reads(&app.update_core(Message::Refresh)), 1);
        let (mut yt, _) = app_with(signed_in());
        assert_eq!(reads(&yt.update_core(Message::Refresh)), 0);
    }

    #[test]
    fn the_source_switch_and_tabs_are_remembered() {
        let (mut app, _) = app_with(signed_in());
        let _ = app.update_core(Message::Sc(ScMsg::Source(Source::SoundCloud)));
        let _ = app.update_core(Message::Sc(ScMsg::Tab(ScTab::History)));
        assert_eq!(app.source, Source::SoundCloud);
        assert_eq!(app.sc_open_tab(), ScTab::History);
        assert!(Source::All.has_soundcloud() && !Source::YouTube.has_soundcloud());
    }

    fn search_app(tos: bool) -> App {
        let backend = mock_backend_soundcloud(signed_in());
        build_app(
            &backend,
            &Scenario {
                soundcloud: Some(ScScene::Account),
                settings: crate::settings::UiSettings {
                    tos_acknowledged: tos,
                    soundcloud: crate::settings::OptInState {
                        enabled: true,
                        confirmed_version: 2,
                    },
                    ..crate::settings::UiSettings::default()
                },
                route: crate::route::Route::Search(String::new()),
                ..Scenario::default()
            },
        )
    }

    fn searches(jobs: &[Job]) -> (usize, usize) {
        let sc = jobs
            .iter()
            .filter(|j| matches!(j, Job::ScSearch(_) | Job::ScSearchKind { .. }))
            .count();
        let yt = jobs
            .iter()
            .filter(|j| matches!(j, Job::Search(_) | Job::SearchKind { .. }))
            .count();
        (sc, yt)
    }

    #[test]
    fn a_search_goes_to_the_service_the_switch_names() {
        use crate::state::Event;
        let mut app = search_app(true);
        let _ = app.update_core(Message::Ui(Event::SearchInput("velvet".to_owned())));
        let yt = app.update_core(Message::Ui(Event::SearchSubmit));
        assert_eq!(searches(&yt), (0, 1), "YouTube Music is the default");
        let mut app = search_app(true);
        let _ = app.update_core(Message::Sc(ScMsg::Source(Source::SoundCloud)));
        let _ = app.update_core(Message::Ui(Event::SearchInput("velvet".to_owned())));
        let sc = app.update_core(Message::Ui(Event::SearchSubmit));
        assert_eq!(searches(&sc), (1, 0));
        let filtered = app.update_core(Message::Ui(Event::SearchFilter(Some(
            nemo_core::catalog::SearchKind::Albums,
        ))));
        assert_eq!(searches(&filtered), (1, 0), "a chip too");
    }

    #[test]
    fn soundcloud_search_needs_no_youtube_notice_and_youtube_still_does() {
        use crate::state::Event;
        let mut app = search_app(false);
        let _ = app.update_core(Message::Sc(ScMsg::Source(Source::SoundCloud)));
        let _ = app.update_core(Message::Ui(Event::SearchInput("velvet".to_owned())));
        let jobs = app.update_core(Message::Ui(Event::SearchSubmit));
        assert_eq!(searches(&jobs), (1, 0), "SoundCloud waits for no notice");
        let mut yt = search_app(false);
        let _ = yt.update_core(Message::Ui(Event::SearchInput("velvet".to_owned())));
        let jobs = yt.update_core(Message::Ui(Event::SearchSubmit));
        assert_eq!(searches(&jobs), (0, 0), "nothing leaves before the notice");
    }

    #[test]
    fn switching_the_source_on_the_search_page_drops_the_other_answers_and_searches_again() {
        use crate::state::Event;
        let mut app = search_app(true);
        let _ = app.update_core(Message::Ui(Event::SearchInput("velvet".to_owned())));
        let jobs = app.update_core(Message::Ui(Event::SearchSubmit));
        app.settle_blocking(jobs);
        assert!(app.ui.search.results.is_some());
        let jobs = app.update_core(Message::Sc(ScMsg::Source(Source::SoundCloud)));
        assert!(
            app.ui.search.results.is_none(),
            "YouTube answers do not stay"
        );
        assert_eq!(searches(&jobs), (1, 0));
        // Choosing the same source again does nothing.
        assert!(
            app.update_core(Message::Sc(ScMsg::Source(Source::SoundCloud)))
                .is_empty()
        );
    }
}
