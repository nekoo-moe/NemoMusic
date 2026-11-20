//! Search in the SoundCloud listening area (source-workspaces spec): the box, the chips and the
//! results belong to SoundCloud alone, and a search typed there is not run in YouTube Music.

use super::App;
use super::model::Source;

impl App {
    /// The Search page belongs to SoundCloud: its searches go to SoundCloud and need no YouTube
    /// Music notice.
    #[must_use]
    pub fn search_is_soundcloud(&self) -> bool {
        self.sc_open() && self.source == Source::SoundCloud
    }
}

#[cfg(test)]
mod tests {
    use nemo_core::catalog::SearchKind;
    use nemo_core::source_account::SourceAccountState;

    use super::super::message::{Job, Message};
    use super::super::model::Source;
    use super::*;
    use crate::gui::headless::soundcloud::ScScene;
    use crate::gui::headless::{Scenario, build_app};
    use crate::mock::{MockScAccount, mock_backend_soundcloud};
    use crate::state::Event;

    /// An app in `area` with SoundCloud signed in; `tos` says whether the YouTube Music notice
    /// was accepted.
    fn app(area: Source, tos: bool) -> App {
        build_app(
            &mock_backend_soundcloud(SourceAccountState::SignedIn(MockScAccount::info())),
            &Scenario {
                soundcloud: Some(ScScene::Account),
                area: Some(area),
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

    fn run_search(app: &mut App, text: &str) -> Vec<Job> {
        let _ = app.update_core(Message::Ui(Event::SearchInput(text.to_owned())));
        app.update_core(Message::Ui(Event::SearchSubmit))
    }

    /// How many SoundCloud and YouTube Music searches `jobs` start.
    fn kinds(jobs: &[Job]) -> (usize, usize) {
        let count = |f: fn(&Job) -> bool| jobs.iter().filter(|j| f(j)).count();
        (
            count(|j| matches!(j, Job::ScSearch(_) | Job::ScSearchKind { .. })),
            count(|j| matches!(j, Job::Search(_) | Job::SearchKind { .. })),
        )
    }

    #[test]
    fn a_search_runs_in_the_area_it_was_typed_in_and_never_in_the_other() {
        let mut yt = app(Source::YouTube, true);
        assert!(!yt.search_is_soundcloud());
        assert_eq!(kinds(&run_search(&mut yt, "velvet")), (0, 1));
        let mut sc = app(Source::SoundCloud, true);
        assert!(sc.search_is_soundcloud());
        assert_eq!(kinds(&run_search(&mut sc, "velvet")), (1, 0));
    }

    #[test]
    fn a_chip_search_follows_the_area_too() {
        let mut yt = app(Source::YouTube, true);
        let _ = run_search(&mut yt, "velvet");
        let jobs = yt.update_core(Message::Ui(Event::SearchFilter(Some(SearchKind::Albums))));
        assert_eq!(kinds(&jobs), (0, 1));
        let mut sc = app(Source::SoundCloud, true);
        let _ = run_search(&mut sc, "velvet");
        let jobs = sc.update_core(Message::Ui(Event::SearchFilter(Some(SearchKind::Albums))));
        assert_eq!(kinds(&jobs), (1, 0));
    }

    #[test]
    fn soundcloud_search_needs_no_youtube_music_notice() {
        let mut sc = app(Source::SoundCloud, false);
        assert_eq!(kinds(&run_search(&mut sc, "velvet")), (1, 0));
    }

    #[test]
    fn the_answer_of_a_search_lands_in_the_area_that_asked() {
        let mut app = app(Source::SoundCloud, true);
        let jobs = run_search(&mut app, "velvet");
        // The person switches to YouTube Music while the SoundCloud search runs.
        let _ = app.update_core(Message::Sc(crate::gui::app::ScMsg::Source(Source::YouTube)));
        assert!(
            app.ui.search.results.is_none(),
            "YouTube Music shows nothing yet"
        );
        app.settle_blocking(jobs);
        assert!(
            app.ui.search.results.is_none(),
            "the SoundCloud answer is not shown in YouTube Music"
        );
        assert!(
            app.rest.search.results.is_some(),
            "it waits in the area that asked"
        );
        let _ = app.update_core(Message::Sc(crate::gui::app::ScMsg::Source(
            Source::SoundCloud,
        )));
        assert!(
            app.ui.search.results.is_some(),
            "it is there on the way back"
        );
    }
}
