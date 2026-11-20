//! Tests of the listening area: the switch, what each area remembers, the fall back to YouTube
//! Music, the saved choice at start, and which sidebar entry is lit.

use nemo_core::catalog::BrowseId;
use nemo_core::source_account::SourceAccountState;

use super::*;
use crate::explore::LibSort;
use crate::gui::app::{HomePhase, Message, ScMsg};
use crate::gui::headless::soundcloud::ScScene;
use crate::gui::headless::{Scenario, build_app};
use crate::mock::{MockScAccount, mock_backend_soundcloud};
use crate::route::Route;
use crate::settings::UiSettings;
use crate::state::Event;

fn signed_in() -> SourceAccountState {
    SourceAccountState::SignedIn(MockScAccount::info())
}

/// An app with SoundCloud on and its account in `state`, in the area `saved` stored last time.
fn app_with(state: SourceAccountState, saved: Source) -> App {
    build_app(
        &mock_backend_soundcloud(state),
        &Scenario {
            soundcloud: Some(ScScene::Account),
            settings: UiSettings {
                listening_area: saved,
                tos_acknowledged: true,
                ..UiSettings::default()
            },
            ..Scenario::default()
        },
    )
}

fn app() -> App {
    app_with(signed_in(), Source::YouTube)
}

fn pick(app: &mut App, source: Source) -> Vec<Job> {
    app.update_core(Message::Sc(ScMsg::Source(source)))
}

fn page(id: &str) -> BrowseId {
    BrowseId(id.to_owned())
}

#[test]
fn one_press_moves_to_the_landing_page_with_fresh_history_and_stores_the_choice() {
    let mut app = app();
    assert_eq!(app.source, Source::YouTube);
    assert_eq!(app.ui.route(), &Route::Home);
    let _ = app.update_core(Message::Ui(Event::Go(Route::Search(String::new()))));
    assert!(app.ui.nav.can_back());
    let _ = pick(&mut app, Source::SoundCloud);
    assert_eq!(app.source, Source::SoundCloud);
    assert_eq!(
        app.ui.route(),
        &Route::Library,
        "SoundCloud lands on its first entry"
    );
    assert!(!app.ui.nav.can_back(), "Back never reaches the other area");
    assert_eq!(app.ui.settings.listening_area, Source::SoundCloud);
    let _ = pick(&mut app, Source::YouTube);
    assert_eq!(app.ui.route(), &Route::Home, "YouTube Music lands on Home");
    assert_eq!(app.ui.settings.listening_area, Source::YouTube);
}

#[test]
fn pressing_the_active_area_or_an_area_that_is_not_offered_does_nothing() {
    let mut app = app();
    assert!(pick(&mut app, Source::YouTube).is_empty());
    assert_eq!(app.ui.route(), &Route::Home);
    let mut out = app_with(SourceAccountState::SignedOut, Source::YouTube);
    assert!(!out.source_offered(Source::SoundCloud));
    let _ = pick(&mut out, Source::SoundCloud);
    assert_eq!(out.source, Source::YouTube);
    assert_eq!(
        out.ui.settings.listening_area,
        Source::YouTube,
        "nothing stored"
    );
}

#[test]
fn the_search_text_is_remembered_for_each_area() {
    let mut app = app();
    let _ = app.update_core(Message::Ui(Event::SearchInput("velvet".to_owned())));
    let _ = pick(&mut app, Source::SoundCloud);
    assert_eq!(app.ui.search.input, "", "SoundCloud has its own box");
    let _ = app.update_core(Message::Ui(Event::SearchInput("ember".to_owned())));
    let _ = pick(&mut app, Source::YouTube);
    assert_eq!(app.ui.search.input, "velvet");
    let _ = pick(&mut app, Source::SoundCloud);
    assert_eq!(app.ui.search.input, "ember");
}

#[test]
fn the_sort_order_is_remembered_for_each_area() {
    let mut app = app();
    app.lib_sort = LibSort::Title;
    let _ = pick(&mut app, Source::SoundCloud);
    assert_eq!(
        app.lib_sort,
        LibSort::Recent,
        "a new area starts in its default order"
    );
    app.lib_sort = LibSort::Artist;
    let _ = pick(&mut app, Source::YouTube);
    assert_eq!(app.lib_sort, LibSort::Title);
    let _ = pick(&mut app, Source::SoundCloud);
    assert_eq!(app.lib_sort, LibSort::Artist);
}

#[test]
fn signing_out_of_soundcloud_goes_back_to_youtube_music_at_once() {
    let backend = mock_backend_soundcloud(signed_in());
    let mut app = build_app(
        &backend,
        &Scenario {
            soundcloud: Some(ScScene::Account),
            area: Some(Source::SoundCloud),
            ..Scenario::default()
        },
    );
    assert_eq!(app.source, Source::SoundCloud);
    if let Some(account) = &backend.soundcloud_account {
        drop(account.sign_out());
    }
    let _ = app.update_core(Message::TimerTick);
    assert_eq!(app.source, Source::YouTube);
    assert_eq!(app.ui.route(), &Route::Home);
    assert_eq!(app.ui.settings.listening_area, Source::YouTube);
    assert!(!app.account.open && !app.create_open, "no popup");
}

#[test]
fn turning_soundcloud_off_goes_back_to_youtube_music_with_no_question_of_its_own() {
    let mut app = app();
    let _ = pick(&mut app, Source::SoundCloud);
    let _ = app.update_core(Message::Sc(ScMsg::TurnOff));
    // Signed in: the switch asks about signing out first; the area stays until it is answered.
    assert!(app.sc.confirm_off);
    assert_eq!(app.source, Source::SoundCloud);
    let _ = app.update_core(Message::Sc(ScMsg::ConfirmOff));
    let _ = app.update_core(Message::Sc(ScMsg::TurnedOff(Ok(
        nemo_core::account::SignOutReport::default(),
    ))));
    assert!(!app.sc_open());
    assert_eq!(app.source, Source::YouTube);
    assert_eq!(app.ui.route(), &Route::Home);
}

#[test]
fn the_saved_area_is_used_at_start_only_when_it_can_run() {
    let saved = app_with(signed_in(), Source::SoundCloud);
    assert_eq!(saved.source, Source::SoundCloud);
    assert_eq!(saved.ui.route(), &Route::Library);
    let signed_out = app_with(SourceAccountState::SignedOut, Source::SoundCloud);
    assert_eq!(signed_out.source, Source::YouTube, "no session: fall back");
    assert_eq!(signed_out.ui.route(), &Route::Home);
    assert_eq!(
        signed_out.ui.settings.listening_area,
        Source::YouTube,
        "and the fall back is stored"
    );
    let no_soundcloud = build_app(
        &crate::mock::mock_backend(),
        &Scenario {
            settings: UiSettings {
                listening_area: Source::SoundCloud,
                ..UiSettings::default()
            },
            ..Scenario::default()
        },
    );
    assert_eq!(no_soundcloud.source, Source::YouTube);
}

#[test]
fn only_the_active_area_loads_in_the_background() {
    let mut app = app_with(signed_in(), Source::SoundCloud);
    assert_eq!(
        app.home.phase,
        HomePhase::Idle,
        "Home waits while SoundCloud is the area"
    );
    assert!(!app.refresh.ready(), "so does YouTube Music's account data");
    let jobs = pick(&mut app, Source::YouTube);
    assert!(jobs.iter().any(|j| matches!(j, Job::HomeFeed)), "{jobs:?}");
    assert!(
        jobs.iter().any(|j| matches!(j, Job::AccountData)),
        "{jobs:?}"
    );
}

#[test]
fn each_area_lights_its_own_entry_and_a_page_of_the_other_service_lights_none() {
    let mut app = app();
    assert_eq!(app.nav_selected(), Some(NavEntry::Home));
    let _ = app.update_core(Message::Ui(Event::Go(Route::Library)));
    assert_eq!(app.nav_selected(), Some(NavEntry::Library));
    // A SoundCloud playlist opened from Now Playing while YouTube Music is the area.
    let _ = app.update_core(Message::Ui(Event::Go(Route::Playlist(page(
        "sc:playlist:1",
    )))));
    assert_eq!(app.nav_selected(), None);
    let _ = pick(&mut app, Source::SoundCloud);
    assert_eq!(app.nav_selected(), Some(NavEntry::Library));
    let _ = app.update_core(Message::Ui(Event::Go(Route::Search(String::new()))));
    assert_eq!(app.nav_selected(), Some(NavEntry::Search));
    let _ = app.update_core(Message::Ui(Event::Go(Route::Album(page("MPREb_x")))));
    assert_eq!(
        app.nav_selected(),
        None,
        "a YouTube Music album lights nothing here"
    );
    let _ = app.update_core(Message::Ui(Event::Go(Route::Playlist(page(
        "sc:playlist:1",
    )))));
    assert_eq!(
        app.nav_selected(),
        Some(NavEntry::Search),
        "its own page lights the entry it came from"
    );
}

#[test]
fn home_and_explore_are_not_in_the_soundcloud_area() {
    let mut app = app_with(signed_in(), Source::SoundCloud);
    assert_eq!(app.source.nav(), [NavEntry::Library, NavEntry::Search]);
    for route in [Route::Home, Route::Explore] {
        let _ = app.update_core(Message::Ui(Event::Go(route)));
        assert_eq!(app.ui.route(), &Route::Library, "sent to the landing page");
    }
}
