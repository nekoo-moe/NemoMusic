//! When the app shows a skeleton (skeleton-loading spec): after 150 ms, only with no cached
//! data, never for an error or an empty answer, and with no frames when nothing needs them.

use std::time::{Duration, Instant};

use super::{App, DetailKey, DetailState, Message};
use crate::gui::headless::{Scenario, build_app};
use crate::mock::mock_backend;
use crate::route::Route;
use crate::skeleton::{Region, Show};
use crate::state::Event;
use nemo_core::catalog::BrowseId;

/// The album page of the mock backend, loaded (so its data is cached), motion on.
fn album_app() -> (App, DetailKey) {
    let route = Route::Album(BrowseId("mock-album-2".to_owned()));
    let mut app = build_app(
        &mock_backend(),
        &Scenario {
            route: route.clone(),
            ..Scenario::default()
        },
    );
    app.ui.reduced_motion = false;
    app.sync_anim();
    let key = DetailKey::of(&route).expect("a detail route");
    assert!(matches!(app.details.get(&key), Some(DetailState::Ready(_))));
    (app, key)
}

/// Run frames 16 ms apart for about `ms` milliseconds.
fn frames(app: &mut App, ms: u64) {
    let start = Instant::now();
    for i in 0..=(ms / 16) {
        app.on_frame(start + Duration::from_millis(i * 16));
    }
}

fn show(app: &App) -> Show {
    app.skeletons.show(Region::Detail)
}

/// The same page, loading again with nothing cached.
fn reload(app: &mut App, key: &DetailKey) {
    app.details.insert(key.clone(), DetailState::Loading);
    app.sync_anim();
}

#[test]
fn cached_data_never_shows_a_skeleton() {
    let (mut app, _) = album_app();
    assert_eq!(show(&app), Show::Content);
    frames(&mut app, 1000);
    assert_eq!(show(&app), Show::Content);
    assert!(!app.needs_frames(), "settled page, no frames");
}

#[test]
fn a_loading_page_shows_its_skeleton_after_150_ms_not_before() {
    let (mut app, key) = album_app();
    reload(&mut app, &key);
    assert_eq!(show(&app), Show::Content, "the wait has begun");
    assert!(app.needs_frames(), "the wait counts time");
    frames(&mut app, 100);
    assert_eq!(show(&app), Show::Content, "100 ms: still blank");
    frames(&mut app, 100);
    assert_eq!(show(&app), Show::Skeleton);
}

#[test]
fn a_fast_answer_never_flashes_the_skeleton() {
    let (mut app, key) = album_app();
    let ready = app.details.get(&key).cloned().expect("loaded");
    reload(&mut app, &key);
    frames(&mut app, 80);
    app.details.insert(key, ready);
    app.sync_anim();
    assert_eq!(show(&app), Show::Content);
    frames(&mut app, 600);
    assert_eq!(show(&app), Show::Content, "it never showed");
}

#[test]
fn data_replaces_the_skeleton_after_the_hold_with_a_fade() {
    let (mut app, key) = album_app();
    let ready = app.details.get(&key).cloned().expect("loaded");
    reload(&mut app, &key);
    frames(&mut app, 200);
    assert_eq!(show(&app), Show::Skeleton);
    app.details.insert(key, ready);
    app.sync_anim();
    assert_eq!(show(&app), Show::Skeleton, "held 300 ms once shown");
    frames(&mut app, 320);
    assert!(matches!(show(&app), Show::Fade(_)), "{:?}", show(&app));
    frames(&mut app, 250);
    assert_eq!(show(&app), Show::Content);
    assert!(!app.needs_frames(), "settled again");
}

#[test]
fn an_error_ends_the_skeleton_and_shows_no_skeleton_for_the_error() {
    let (mut app, key) = album_app();
    reload(&mut app, &key);
    frames(&mut app, 200);
    app.details
        .insert(key, DetailState::Failed("offline".to_owned()));
    app.sync_anim();
    frames(&mut app, 600);
    assert_eq!(show(&app), Show::Content, "the plain sentence with Retry");
    assert!(!app.needs_frames());
}

#[test]
fn a_shown_skeleton_shimmers_only_in_a_focused_window() {
    let (mut app, key) = album_app();
    reload(&mut app, &key);
    frames(&mut app, 200);
    assert_eq!(show(&app), Show::Skeleton);
    assert!(app.needs_frames(), "the band moves");
    let _ = app.update_core(Message::Focus(false));
    assert!(!app.needs_frames(), "no focus: no frames");
    let _ = app.update_core(Message::Focus(true));
    assert!(app.needs_frames());
}

#[test]
fn the_shimmer_clock_moves_while_a_skeleton_shows_and_rests_after() {
    let (mut app, key) = album_app();
    reload(&mut app, &key);
    frames(&mut app, 200);
    let a = app.skeletons.phase();
    frames(&mut app, 160);
    assert!((app.skeletons.phase() - a).abs() > 0.05, "the band moves");
    app.details.insert(key, DetailState::Failed(String::new()));
    app.sync_anim();
    frames(&mut app, 800);
    assert!(app.skeletons.phase().abs() < 1e-6, "the clock rests");
}

#[test]
fn reduced_motion_draws_a_still_skeleton_and_asks_for_no_frames() {
    let (mut app, key) = album_app();
    app.ui.reduced_motion = true;
    reload(&mut app, &key);
    frames(&mut app, 200);
    assert_eq!(show(&app), Show::Skeleton);
    assert!(!app.needs_frames(), "a still skeleton needs no frames");
    assert!(app.skeletons.phase().abs() < 1e-6, "no band");
    app.details
        .insert(key, DetailState::Failed("offline".to_owned()));
    app.sync_anim();
    frames(&mut app, 400);
    assert_eq!(show(&app), Show::Content, "swapped without a fade");
}

#[test]
fn leaving_the_page_drops_its_skeleton_at_once() {
    let (mut app, key) = album_app();
    reload(&mut app, &key);
    frames(&mut app, 200);
    assert_eq!(show(&app), Show::Skeleton);
    let _ = app.update_core(Message::Ui(Event::Go(Route::Settings)));
    assert_eq!(show(&app), Show::Content);
    frames(&mut app, 600);
    assert!(!app.skeletons.any_visible());
}

/// An app on `route` with the notice accepted (YouTube is on), motion on, settled.
fn app_on(route: Route, search: Option<&str>) -> App {
    let mut app = build_app(
        &mock_backend(),
        &Scenario {
            route,
            search: search.map(str::to_owned),
            settings: crate::UiSettings {
                tos_acknowledged: true,
                ..crate::UiSettings::default()
            },
            ..Scenario::default()
        },
    );
    app.ui.reduced_motion = false;
    app.sync_anim();
    app
}

#[test]
fn search_waits_with_a_skeleton_only_with_no_answer_on_screen() {
    use crate::gui::headless::Pending;
    use nemo_core::catalog::{SearchKind, SearchResults};
    let mut app = app_on(Route::Search(String::new()), Some("velvet"));
    assert!(!app.search_pending(), "an answer is on screen");
    Pending::Search.apply(&mut app);
    assert!(app.search_pending(), "nothing answered yet");
    app.ui.search.error = Some("offline".to_owned());
    assert!(!app.search_pending(), "an error keeps its sentence");
    app.ui.search.error = None;
    app.ui.search.results = Some(std::sync::Arc::new(SearchResults::default()));
    assert!(
        !app.search_pending(),
        "an empty answer keeps its empty state"
    );
    // A picked chip with no answer yet waits again, even with the All answer on screen.
    app.ui.search.filter = Some(SearchKind::Albums);
    app.ui.search.filtered = None;
    assert!(app.search_pending(), "the chip's answer is on its way");
}

#[test]
fn search_shows_its_skeleton_after_150_ms_and_never_when_the_box_is_empty() {
    use crate::gui::headless::Pending;
    let mut app = app_on(Route::Search(String::new()), Some("velvet"));
    Pending::Search.apply(&mut app);
    app.sync_anim();
    frames(&mut app, 100);
    assert_eq!(app.skeletons.show(Region::Search), Show::Content);
    frames(&mut app, 100);
    assert_eq!(app.skeletons.show(Region::Search), Show::Skeleton);
    app.ui.search.input.clear();
    app.sync_anim();
    frames(&mut app, 600);
    assert!(!app.skeletons.any_visible(), "no text, nothing to wait for");
}

#[test]
fn home_waits_with_a_skeleton_only_when_nothing_is_cached() {
    use crate::gui::headless::Pending;
    let mut app = app_on(Route::Home, None);
    assert!(!app.home_pending(), "Home has its shelves");
    Pending::Home.apply(&mut app);
    assert!(app.home_pending());
    app.sync_anim();
    frames(&mut app, 200);
    assert_eq!(app.skeletons.show(Region::Home), Show::Skeleton);
    // A recent song is cached data: the page shows it and the small refresh line stays.
    app.ui.recent.push(crate::mock::track(0));
    assert!(!app.home_pending());
}

#[test]
fn explore_waits_with_a_skeleton_until_an_answer_or_a_failure() {
    use crate::gui::headless::Pending;
    let mut app = app_on(Route::Explore, None);
    Pending::Explore.apply(&mut app);
    assert!(app.explore_pending());
    app.sync_anim();
    frames(&mut app, 200);
    assert_eq!(app.skeletons.show(Region::Explore), Show::Skeleton);
    app.explore.finish(Err("offline".to_owned()));
    assert!(
        !app.explore_pending(),
        "the fallback shelves and the notice show"
    );
    app.sync_anim();
    frames(&mut app, 600);
    assert_eq!(app.skeletons.show(Region::Explore), Show::Content);
}

#[test]
fn a_skeleton_asks_for_no_frames_while_another_page_is_open() {
    use crate::gui::headless::Pending;
    let mut app = app_on(Route::Home, None);
    Pending::Home.apply(&mut app);
    app.sync_anim();
    frames(&mut app, 200);
    assert!(app.needs_frames());
    let _ = app.update_core(Message::Ui(Event::Go(Route::Settings)));
    frames(&mut app, 600);
    assert!(!app.skeletons.any_visible());
    assert!(!app.needs_frames(), "the hidden page costs no frames");
}

/// A signed-in app on `route` whose account library has not arrived (nothing cached), motion on.
fn loading_library_app(route: Route) -> App {
    use crate::gui::headless::Pending;
    let mut app = build_app(
        &crate::mock::mock_backend_signed_in(),
        &Scenario {
            route,
            size: (1366, 697),
            settings: crate::UiSettings {
                tos_acknowledged: true,
                ..crate::UiSettings::default()
            },
            ..Scenario::default()
        },
    );
    app.ui.reduced_motion = false;
    Pending::Library.apply(&mut app);
    app.sync_anim();
    app
}

#[test]
fn the_library_and_the_sidebar_wait_with_a_skeleton_only_while_it_loads() {
    let mut app = loading_library_app(Route::Library);
    assert!(app.library_loading() && app.library_pending() && app.sidebar_pending());
    frames(&mut app, 200);
    assert_eq!(app.skeletons.show(Region::Library), Show::Skeleton);
    assert_eq!(app.skeletons.show(Region::Sidebar), Show::Skeleton);
    // Local files are cached data: the Local tab never waits.
    app.lib_tab = crate::explore::LibTab::Local;
    assert!(!app.library_pending());
    app.lib_tab = crate::explore::LibTab::Overview;
    // A failure has its sentence, an empty library its empty state: neither is loading.
    app.library_failed = true;
    assert!(!app.library_loading());
    app.library_failed = false;
    app.account_library = Some(std::sync::Arc::new(super::AccountLibrary::default()));
    assert!(!app.library_loading() && !app.sidebar_pending());
    app.sync_anim();
    frames(&mut app, 600);
    assert!(!app.skeletons.any_visible());
    assert!(!app.needs_frames());
}

#[test]
fn a_signed_out_window_and_cached_local_albums_never_wait_for_the_library() {
    let app = app_on(Route::Library, None);
    assert!(!app.library_loading(), "signed out: nothing to load");
    let mut app = loading_library_app(Route::Library);
    app.local_albums.push(nemo_core::catalog::AlbumSummary {
        id: BrowseId("local-album-x".to_owned()),
        title: "Album".to_owned(),
        artists: Vec::new(),
        year: None,
        thumbnails: Vec::new(),
    });
    assert!(!app.sidebar_pending(), "a local album is a row");
    app.lib_tab = crate::explore::LibTab::Albums;
    assert!(!app.library_pending(), "the Albums tab shows it");
}

#[test]
fn the_sidebar_skeleton_needs_the_wide_sidebar() {
    let mut app = loading_library_app(Route::Library);
    assert!(app.sidebar_pending());
    app.fullscreen = true;
    assert!(!app.sidebar_pending(), "full screen hides it");
    app.fullscreen = false;
    app.mini = true;
    assert!(!app.sidebar_pending(), "the mini player hides it");
}
