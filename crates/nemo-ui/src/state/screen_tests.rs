//! Unit tests of the reducer for screens: toasts, notice, search, queue and menu.

use super::*;
use crate::keys::KeyAction;
use crate::mock;
use crate::settings::SettingChange;
use nemo_core::catalog::SearchResults;
use nemo_core::player::PlayerEvent;
use nemo_core::segment::{Segment, SegmentCategory};
use std::sync::Arc;

fn state() -> UiState {
    let mut s = UiState::new(Route::Home, PlayerSnapshot::default(), false);
    let queue: Vec<Track> = (0..4).map(mock::track).collect();
    s.apply(Event::Snapshot(PlayerSnapshot {
        state: PlayState::Playing,
        current: Some(queue[1].clone()),
        queue,
        queue_index: Some(1),
        ..PlayerSnapshot::default()
    }));
    s
}

#[test]
fn a_failure_is_a_calm_toast_that_expires() {
    let mut s = state();
    s.apply(Event::Player(PlayerEvent::Failed {
        message: "Could not play this song.".into(),
    }));
    assert_eq!(s.toast.as_ref().map(|t| t.undo), Some(false));
    for _ in 0..500 {
        s.apply(Event::Frame { dt_ms: 16.0 });
    }
    assert!(s.toast.is_none());
}

#[test]
fn a_skipped_segment_offers_undo() {
    let mut s = state();
    s.apply(Event::Player(PlayerEvent::SegmentSkipped(Segment {
        start: Duration::from_secs(42),
        end: Duration::from_secs(60),
        category: SegmentCategory::Sponsor,
        id: "x".into(),
    })));
    let t = s.toast.clone().expect("toast");
    assert!(t.undo && t.text.contains("(0:42)"));
    assert_eq!(
        s.apply(Event::UndoSkip),
        vec![Effect::Send(PlayerCommand::UndoSkip)]
    );
    assert!(s.toast.is_none());
}

#[test]
fn the_notice_needs_its_checkbox_and_reports_the_setting() {
    let mut s = state();
    assert!(s.tos_sheet_visible() && !s.youtube_enabled());
    assert_eq!(s.apply(Event::TosContinue), vec![]);
    s.apply(Event::TosCheck(true));
    let fx = s.apply(Event::TosContinue);
    assert!(matches!(&fx[..], [Effect::SettingsChanged(x)] if x.tos_acknowledged));
    assert!(!s.tos_sheet_visible() && s.youtube_enabled());
}

#[test]
fn not_now_hides_the_sheet_but_keeps_youtube_off() {
    let mut s = state();
    s.apply(Event::TosLater);
    assert!(!s.tos_sheet_visible() && !s.youtube_enabled());
    s.apply(Event::TosReview);
    assert!(s.tos_sheet_visible());
}

#[test]
fn sponsorblock_switch_sends_the_player_command_and_the_callback() {
    let mut s = state();
    let fx = s.apply(Event::Setting(SettingChange::SponsorBlock(true)));
    assert_eq!(fx[0], Effect::Send(PlayerCommand::SetSkipSegments(true)));
    assert!(matches!(&fx[1], Effect::SettingsChanged(x) if x.sponsorblock));
    let fx = s.apply(Event::Setting(SettingChange::ReduceMotion(true)));
    assert!(s.reduced_motion && fx.len() == 1);
}

#[test]
fn queue_clear_removes_from_the_end_and_keeps_the_current_track() {
    let mut s = state();
    let fx = s.apply(Event::Queue(QueueOp::Clear));
    assert_eq!(
        fx,
        vec![
            Effect::Send(PlayerCommand::RemoveAt(3)),
            Effect::Send(PlayerCommand::RemoveAt(2)),
            Effect::Send(PlayerCommand::RemoveAt(0)),
        ]
    );
    let fx = s.apply(Event::Queue(QueueOp::PlayNext(3)));
    assert_eq!(fx.len(), 2);
    assert_eq!(s.apply(Event::Queue(QueueOp::PlayNext(1))), vec![]);
}

#[test]
fn search_events_debounce_then_run_and_enter_plays() {
    let mut s = state();
    let fx = s.apply(Event::SearchInput("lofi".into()));
    let Effect::Debounce(seq) = fx[0] else {
        panic!("expected a debounce, got {fx:?}");
    };
    let fx = s.apply(Event::SearchDebounced(seq));
    assert_eq!(fx, vec![Effect::RunSearch("lofi".into())]);
    let results = Arc::new(SearchResults {
        songs: vec![mock::track(5)],
        ..SearchResults::default()
    });
    s.apply(Event::SearchDone {
        query: "lofi".into(),
        result: Ok(results),
    });
    let fx = s.apply(Event::SearchSubmit);
    assert!(matches!(
        &fx[..],
        [Effect::Send(PlayerCommand::SetQueue { start: 0, .. })]
    ));
}

#[test]
fn search_shortcuts_focus_the_box_and_esc_closes_things_in_order() {
    let mut s = state();
    let fx = s.apply(Event::Key(KeyAction::OpenSearch));
    assert!(fx.contains(&Effect::FocusSearch) && s.text_focused);
    s.apply(Event::OpenMenu(mock::track(2)));
    s.apply(Event::ToggleSignalPath);
    s.apply(Event::ToggleQueue);
    s.apply(Event::Key(KeyAction::Close));
    assert!(s.menu.is_none() && s.signal_open && s.queue_open);
    s.apply(Event::Key(KeyAction::Close));
    assert!(!s.signal_open && s.queue_open);
    s.apply(Event::Key(KeyAction::Close));
    assert!(!s.queue_open);
}

#[test]
fn recent_tracks_and_menu_actions() {
    let mut s = state();
    assert_eq!(s.recent.len(), 1);
    s.apply(Event::OpenMenu(mock::track(2)));
    let fx = s.apply(Event::GoToArtist(mock::track(2)));
    assert!(s.menu.is_none());
    assert!(matches!(
        &fx[..],
        [Effect::Resolve {
            kind: ResolveKind::Artist,
            ..
        }]
    ));
    s.apply(Event::Go(Route::Queue));
    assert!(
        s.queue_open && *s.route() == Route::Home,
        "queue is a panel"
    );
}
