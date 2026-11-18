//! Frame budget of the whole app (app-motion spec): a settled window asks for no frames, an
//! animation asks only until it settles, and reduced motion settles at once.

use std::time::{Duration, Instant};

use super::{App, Message};
use crate::gui::headless::{Scenario, build_app};
use crate::mock::mock_backend;
use crate::state::Event;

/// A paused app with motion on (headless builds start with motion reduced).
fn idle_app() -> App {
    let mut app = build_app(&mock_backend(), &Scenario::default());
    app.ui.reduced_motion = false;
    app.sync_anim();
    app
}

/// Run frames 16 ms apart until the app stops asking; returns how many it took.
fn frames_until_idle(app: &mut App) -> usize {
    let start = Instant::now();
    for i in 0..600_u32 {
        if !app.needs_frames() {
            return i as usize;
        }
        app.on_frame(start + Duration::from_millis(u64::from(i) * 16));
    }
    600
}

#[test]
fn a_settled_window_asks_for_no_frames() {
    let app = idle_app();
    assert!(!app.ui.is_playing());
    assert!(!app.needs_frames(), "paused and settled: zero frames");
}

#[test]
fn a_pane_toggle_animates_then_stops_asking() {
    let mut app = idle_app();
    let _ = app.update_core(Message::TogglePane);
    assert!(app.needs_frames(), "the pane spring runs");
    let n = frames_until_idle(&mut app);
    // spatial.default settles in about 250 ms; 400 ms is the hard cap of design.md section 8.
    assert!(n > 1 && n <= 25, "{n} frames");
    assert!(!app.needs_frames());
}

#[test]
fn an_unfocused_window_asks_for_no_frames_and_asks_again_when_focused() {
    let mut app = idle_app();
    let _ = app.update_core(Message::TogglePane);
    assert!(app.needs_frames(), "the pane spring runs");
    let _ = app.update_core(Message::Focus(false));
    assert!(!app.needs_frames(), "no focus: no frames");
    let _ = app.update_core(Message::Focus(true));
    assert!(
        app.needs_frames(),
        "focus is back and the spring still has to settle"
    );
    assert!(app.last_frame.is_none(), "the pause is not one long frame");
}

#[test]
fn a_page_change_animates_then_stops_asking() {
    let mut app = idle_app();
    let _ = app.update_core(Message::Ui(Event::Go(crate::route::Route::Settings)));
    assert!(app.needs_frames());
    let n = frames_until_idle(&mut app);
    assert!(n > 1 && n <= 30, "{n} frames");
}

#[test]
fn reduced_motion_needs_no_frames_for_the_same_change() {
    let mut app = build_app(&mock_backend(), &Scenario::default());
    assert!(app.ui.reduced_motion);
    let _ = app.update_core(Message::TogglePane);
    let _ = app.update_core(Message::Ui(Event::Go(crate::route::Route::Settings)));
    assert!(!app.needs_frames(), "reduced motion jumps: no frames");
}

/// A page of a local album that is still loading (a skeleton), entered with motion on.
fn loading_page_app() -> App {
    let mut app = idle_app();
    let id = nemo_core::catalog::BrowseId(format!("{}x", crate::home::LOCAL_ALBUM_PREFIX));
    let route = crate::route::Route::Album(id);
    let key = super::DetailKey::of(&route).expect("a detail route");
    app.details.insert(key, super::DetailState::Loading);
    let _ = app.update_core(Message::Ui(Event::Go(route)));
    app
}

#[test]
fn a_loading_skeleton_is_still_and_asks_for_no_frames_once_faded_in() {
    let mut app = loading_page_app();
    assert!(app.detail_loading(), "the skeleton is on screen");
    assert!(app.needs_frames(), "the page fade runs first");
    let n = frames_until_idle(&mut app);
    assert!(n > 1 && n <= 30, "{n} frames");
    assert!(app.detail_loading(), "still loading, still drawn");
    assert!(!app.needs_frames(), "a still skeleton draws no frames");
    let phase = app.shimmer;
    app.on_frame(Instant::now());
    assert!(
        (app.shimmer - phase).abs() < 1e-6,
        "the highlight never moves"
    );
}

#[test]
fn a_tab_change_slides_the_pill_then_stops_asking() {
    let mut app = idle_app();
    let _ = app.update_core(Message::NpTab(super::NpTab::Info));
    let _ = frames_until_idle(&mut app); // the pane opens first
    let _ = app.update_core(Message::NpTab(super::NpTab::Lyrics));
    let _ = app.update_core(Message::NpTab(super::NpTab::Info));
    assert!((app.micro.tab.target - 2.0).abs() < 1e-6);
    let n = frames_until_idle(&mut app);
    assert!(n <= 25, "{n} frames");
    assert!((app.micro.tab.value - 2.0).abs() < 1e-3);
}

#[test]
fn a_page_change_slides_the_sidebar_pill_to_the_entry() {
    let mut app = idle_app();
    let _ = app.update_core(Message::Ui(Event::Go(crate::route::Route::Library)));
    assert!((app.micro.nav.target - 3.0).abs() < 1e-6);
    assert!(app.needs_frames());
    let _ = frames_until_idle(&mut app);
    assert!((app.micro.nav.value - 3.0).abs() < 1e-3);
}

#[test]
fn a_search_filter_slides_its_pill() {
    let mut app = idle_app();
    let _ = app.update_core(Message::Ui(Event::SearchFilter(Some(
        nemo_core::catalog::SearchKind::Albums,
    ))));
    let (x, w) = &app.chip_pills[2];
    assert!(
        x.target > 0.0 && w.target > 0.0,
        "{} {}",
        x.target,
        w.target
    );
    let _ = frames_until_idle(&mut app);
    assert!((app.chip_pills[2].0.value - app.chip_pills[2].0.target).abs() < 1e-3);
}

#[test]
fn an_arriving_picture_fades_in_over_150_ms_then_asks_for_no_frames() {
    let mut app = idle_app();
    let key = "a-cover".to_owned();
    let pixels = std::sync::Arc::new(crate::cover::Baked {
        width: 1,
        height: 1,
        rgba: vec![0; 4],
    });
    let _ = app.update_core(Message::Art {
        key: key.clone(),
        pixels: Ok(pixels),
    });
    assert!(app.micro.art.opacity(&key) < 0.01);
    assert!(app.needs_frames());
    let n = frames_until_idle(&mut app);
    // 150 ms at 16 ms a frame.
    assert!((8..=11).contains(&n), "{n} frames");
    assert!((app.micro.art.opacity(&key) - 1.0).abs() < 1e-6);
}

#[test]
fn a_picture_appears_at_once_with_reduced_motion() {
    let mut app = idle_app();
    app.ui.reduced_motion = true;
    let pixels = std::sync::Arc::new(crate::cover::Baked {
        width: 1,
        height: 1,
        rgba: vec![0; 4],
    });
    let _ = app.update_core(Message::Art {
        key: "k".to_owned(),
        pixels: Ok(pixels),
    });
    assert!((app.micro.art.opacity("k") - 1.0).abs() < 1e-6);
    assert!(!app.needs_frames());
}

#[test]
fn nothing_but_the_menu_moves_while_a_track_menu_is_open() {
    let mut app = build_app(
        &mock_backend(),
        &Scenario {
            playing: true,
            ..Scenario::default()
        },
    );
    app.ui.reduced_motion = false;
    assert!(crate::gui::views::backdrop::window_inputs(&app).playing);
    let _ = app.update_core(Message::Ui(Event::OpenMenu(crate::mock::track(2))));
    app.sync_anim();
    assert!(app.surfaces.menu.visible());
    assert!(
        !crate::gui::views::backdrop::window_inputs(&app).playing,
        "the backdrop holds still"
    );
    let phase = app.eq_phase;
    app.on_frame(Instant::now());
    assert!((app.eq_phase - phase).abs() < 1e-6, "the equaliser holds");
}
