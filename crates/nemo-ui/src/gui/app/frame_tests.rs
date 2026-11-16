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
