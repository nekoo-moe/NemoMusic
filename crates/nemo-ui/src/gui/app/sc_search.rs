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

    use super::super::message::Message;
    use super::super::model::{ListSource, ScMsg, Source};
    use super::*;
    use crate::gui::headless::soundcloud::ScScene;
    use crate::gui::headless::{Scenario, build_app};
    use crate::mock::{MockScAccount, mock_backend_soundcloud};
    use crate::state::Event;

    fn app(tos: bool) -> App {
        build_app(
            &mock_backend_soundcloud(SourceAccountState::SignedIn(MockScAccount::info())),
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

    fn run_search(app: &mut App, text: &str) -> Vec<Job> {
        let _ = app.update_core(Message::Ui(Event::SearchInput(text.to_owned())));
        app.update_core(Message::Ui(Event::SearchSubmit))
    }

    fn kinds(jobs: &[Job]) -> (usize, usize, usize) {
        let count = |f: fn(&Job) -> bool| jobs.iter().filter(|j| f(j)).count();
        (
            count(|j| matches!(j, Job::ScSearch(_) | Job::ScSearchKind { .. })),
            count(|j| matches!(j, Job::Search(_) | Job::SearchKind { .. })),
            count(|j| matches!(j, Job::ScGroup { .. })),
        )
    }

    #[test]
    fn all_runs_youtube_music_and_a_soundcloud_group_side_by_side() {
        let mut app = app(true);
        let _ = app.update_core(Message::Sc(ScMsg::Source(Source::All)));
        assert!(app.search_is_all() && !app.search_is_soundcloud());
        let jobs = run_search(&mut app, "velvet");
        assert_eq!(kinds(&jobs), (0, 1, 1), "{jobs:?}");
        assert!(matches!(
            app.sc_group_now().map(|g| &g.phase),
            Some(ScGroupPhase::Loading)
        ));
        // YouTube Music's job comes first in the list; the group is the second search.
        assert!(
            matches!(jobs.last(), Some(Job::Search(_))),
            "the YouTube Music search is not held back by the group"
        );
    }

    #[test]
    fn each_source_alone_starts_no_group() {
        for source in [Source::YouTube, Source::SoundCloud] {
            let mut app = app(true);
            let _ = app.update_core(Message::Sc(ScMsg::Source(source)));
            let jobs = run_search(&mut app, "velvet");
            assert_eq!(kinds(&jobs).2, 0, "{source:?}");
            assert!(app.sc_group_now().is_none());
        }
    }

    #[test]
    fn all_without_the_youtube_notice_searches_soundcloud_alone() {
        let mut app = app(false);
        let _ = app.update_core(Message::Sc(ScMsg::Source(Source::All)));
        assert!(app.search_is_soundcloud() && !app.search_is_all());
        let jobs = run_search(&mut app, "velvet");
        assert_eq!(kinds(&jobs), (1, 0, 0), "nothing leaves for YouTube Music");
    }

    #[test]
    fn the_group_answer_fills_the_group_and_its_songs_are_a_list_of_their_own() {
        let mut app = app(true);
        let _ = app.update_core(Message::Sc(ScMsg::Source(Source::All)));
        let jobs = run_search(&mut app, "velvet");
        app.settle_blocking(jobs);
        let group = app.sc_group_now().expect("group");
        let ScGroupPhase::Done(found) = &group.phase else {
            panic!("the mock answers: {:?}", group.phase);
        };
        assert!(!found.songs.is_empty());
        assert_eq!(app.tracks_of(&ListSource::ScSearch), found.songs);
        // Without the group (another source) the list is empty, whatever YouTube Music found.
        let _ = app.update_core(Message::Sc(ScMsg::Source(Source::YouTube)));
        assert!(app.tracks_of(&ListSource::ScSearch).is_empty());
    }

    #[test]
    fn a_chip_asks_both_and_videos_have_no_group() {
        let mut app = app(true);
        let _ = app.update_core(Message::Sc(ScMsg::Source(Source::All)));
        let _ = run_search(&mut app, "velvet");
        let albums = app.update_core(Message::Ui(Event::SearchFilter(Some(SearchKind::Albums))));
        assert_eq!(kinds(&albums), (0, 1, 1));
        assert_eq!(
            app.sc_group_now().map(|g| g.kind),
            Some(Some(SearchKind::Albums))
        );
        let videos = app.update_core(Message::Ui(Event::SearchFilter(Some(SearchKind::Videos))));
        assert_eq!(kinds(&videos), (0, 1, 0));
        assert!(app.sc_group_now().is_none());
        // Back on the All chip, the first group is still there.
        let _ = app.update_core(Message::Ui(Event::SearchFilter(None)));
        assert!(app.sc_group_now().is_some_and(|g| g.kind.is_none()));
    }

    #[test]
    fn an_answer_nobody_waits_for_is_dropped_and_a_failure_is_a_fixed_state() {
        let mut app = app(true);
        let _ = app.update_core(Message::Sc(ScMsg::Source(Source::All)));
        let _ = run_search(&mut app, "velvet");
        let stray = Ok(Arc::new(SearchResults::default()));
        app.sc_group_done("other text", None, stray.clone());
        app.sc_group_done("velvet", Some(SearchKind::Albums), stray);
        assert!(matches!(
            app.sc_group_now().map(|g| &g.phase),
            Some(ScGroupPhase::Loading)
        ));
        app.sc_group_done("velvet", None, Err("a long detail text".to_owned()));
        assert!(matches!(
            app.sc_group_now().map(|g| &g.phase),
            Some(ScGroupPhase::Failed)
        ));
        // A later answer for the same entry does not undo the failure.
        app.sc_group_done("velvet", None, Ok(Arc::new(SearchResults::default())));
        assert!(matches!(
            app.sc_group_now().map(|g| &g.phase),
            Some(ScGroupPhase::Failed)
        ));
    }

    #[test]
    fn a_new_text_and_a_source_switch_forget_the_old_groups() {
        let mut app = app(true);
        let _ = app.update_core(Message::Sc(ScMsg::Source(Source::All)));
        let _ = run_search(&mut app, "velvet");
        let _ = run_search(&mut app, "ember");
        assert!(app.sc_groups.iter().all(|g| g.query == "ember"));
        let jobs = app.update_core(Message::Sc(ScMsg::Source(Source::YouTube)));
        assert!(app.sc_groups.is_empty());
        assert_eq!(kinds(&jobs).2, 0);
    }
}
