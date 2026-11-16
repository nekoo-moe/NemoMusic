//! Every floating surface enters, leaves and then asks for no frames.

use std::time::{Duration, Instant};

use super::super::{App, Message};
use crate::gui::headless::{Scenario, build_app};
use crate::mock::{self, mock_backend};
use crate::state::Event;

fn moving_app() -> App {
    let mut app = build_app(&mock_backend(), &Scenario::default());
    app.ui.reduced_motion = false;
    app.sync_anim();
    app
}

/// Frames 16 ms apart for `ms`; returns the frames the app still asked for.
fn frames(app: &mut App, ms: u64) -> usize {
    let start = Instant::now();
    let mut asked = 0;
    for i in 0..ms / 16 {
        if app.needs_frames() {
            asked += 1;
            app.on_frame(start + Duration::from_millis(i * 16));
        }
    }
    asked
}

fn send(app: &mut App, event: Event) {
    let _ = app.update_core(Message::Ui(event));
}

#[test]
fn the_signal_popover_fades_in_then_out_within_motion_tokens() {
    let mut app = moving_app();
    send(&mut app, Event::ToggleSignalPath);
    assert!(app.surfaces.signal.visible());
    assert!(app.surfaces.signal.value() < 0.01, "starts hidden");
    // motion.base: about 13 frames.
    let n = frames(&mut app, 1000);
    assert!((10..=14).contains(&n), "{n}");
    assert!((app.surfaces.signal.value() - 1.0).abs() < 1e-6);
    send(&mut app, Event::ToggleSignalPath);
    assert!(app.surfaces.signal.visible(), "drawn while it leaves");
    let n = frames(&mut app, 1000);
    assert!((5..=8).contains(&n), "motion.fast: {n}");
    assert!(!app.surfaces.signal.visible() && !app.needs_frames());
}

#[test]
fn a_closed_menu_keeps_its_track_while_it_fades_out() {
    let mut app = moving_app();
    send(&mut app, Event::OpenMenu(mock::track(2)));
    frames(&mut app, 400);
    send(&mut app, Event::CloseMenu);
    assert!(app.ui.menu.is_none());
    let shown = app.surfaces.menu_shown().map(|t| t.id.clone());
    assert_eq!(shown, Some(mock::track(2).id));
    frames(&mut app, 400);
    assert!(app.surfaces.menu_shown().is_none());
    assert!(!app.needs_frames());
}

#[test]
fn a_toast_enters_and_leaves_when_dismissed() {
    let mut app = moving_app();
    send(&mut app, Event::Notice("Link copied".to_owned()));
    assert!(app.surfaces.toast.value() < 0.01);
    frames(&mut app, 300);
    assert!((app.surfaces.toast.value() - 1.0).abs() < 1e-6);
    send(&mut app, Event::DismissToast);
    let text = app.surfaces.toast_shown().map(|t| t.text.clone());
    assert_eq!(text.as_deref(), Some("Link copied"));
    frames(&mut app, 300);
    assert!(app.surfaces.toast_shown().is_none());
    assert!(!app.needs_frames());
}

#[test]
fn a_new_toast_text_enters_again() {
    let mut app = moving_app();
    send(&mut app, Event::Notice("One".to_owned()));
    frames(&mut app, 300);
    send(&mut app, Event::Notice("Two".to_owned()));
    assert!(app.surfaces.toast.value() < 0.01, "the new notice enters");
}

#[test]
fn reduced_motion_shows_and_hides_surfaces_at_once() {
    let mut app = build_app(&mock_backend(), &Scenario::default());
    assert!(app.ui.reduced_motion);
    send(&mut app, Event::ToggleSignalPath);
    send(&mut app, Event::OpenMenu(mock::track(1)));
    assert!((app.surfaces.signal.value() - 1.0).abs() < 1e-6);
    assert!((app.surfaces.menu.value() - 1.0).abs() < 1e-6);
    assert!(!app.needs_frames());
    send(&mut app, Event::CloseMenu);
    assert!(app.surfaces.menu_shown().is_none());
    assert!(!app.needs_frames());
}
