use std::sync::Arc;

use nemo_core::dev::{DevLevel, DevProbe};
use nemo_core::services::Backend;

use super::{
    CLICK_RESET_MS, CLICKS_TO_UNLOCK, COPIED_TOAST, ClickCounter, ClipboardSink, DevMsg, DevTab,
    UNLOCK_TOAST,
};
use crate::gui::app::{App, Message};
use crate::gui::headless::{Scenario, build_app};
use crate::keys::{Key, Mods};
use crate::mock::{MockProbe, mock_backend, mock_backend_dev, sample_snapshot};

fn toast(app: &App) -> Option<&str> {
    app.ui.toast.as_ref().map(|t| t.text.as_str())
}

fn app_with(backend: &Backend) -> App {
    build_app(backend, &Scenario::default())
}

fn click(app: &mut App) {
    app.update_core(Message::Dev(DevMsg::Click));
}

fn unlock(app: &mut App) {
    for _ in 0..CLICKS_TO_UNLOCK {
        click(app);
    }
}

#[derive(Default)]
struct FakeClipboard(Vec<String>);

impl ClipboardSink for FakeClipboard {
    fn write(&mut self, text: String) {
        self.0.push(text);
    }
}

#[test]
fn ten_clicks_in_a_row_complete_a_run_and_the_next_run_starts_again() {
    let mut counter = ClickCounter::default();
    for n in 0..9 {
        assert!(!counter.click(n * 100), "click {n}");
    }
    assert!(counter.click(900), "the tenth click");
    assert_eq!(counter.count(), 0);
    assert!(!counter.click(1000), "the eleventh click starts a new run");
    assert_eq!(counter.count(), 1);
}

#[test]
fn three_seconds_without_a_click_reset_the_count() {
    let mut counter = ClickCounter::default();
    for n in 0..9 {
        counter.click(n * 100);
    }
    assert!(
        !counter.click(800 + CLICK_RESET_MS + 1),
        "too late: a new run"
    );
    assert_eq!(counter.count(), 1);
    // Exactly at the limit still counts.
    let mut counter = ClickCounter::default();
    counter.click(0);
    counter.click(CLICK_RESET_MS);
    assert_eq!(counter.count(), 2);
}

#[test]
fn slow_clicks_that_each_follow_within_three_seconds_still_count() {
    let mut counter = ClickCounter::default();
    let opened =
        (0..u64::from(CLICKS_TO_UNLOCK)).any(|n| counter.click(n * (CLICK_RESET_MS - 100)));
    assert!(opened);
}

#[test]
fn the_tenth_click_opens_the_panel_once_with_one_toast() {
    let mut app = app_with(&mock_backend_dev());
    for _ in 0..CLICKS_TO_UNLOCK - 1 {
        click(&mut app);
    }
    assert!(!app.dev.open && !app.dev.unlocked);
    // Other messages between the clicks do not matter.
    app.update_core(Message::Noop);
    app.update_core(Message::Focus(true));
    click(&mut app);
    assert!(app.dev.open && app.dev.unlocked);
    assert_eq!(toast(&app), Some(UNLOCK_TOAST));
    // The eleventh click does not toggle it.
    app.ui.toast = None;
    click(&mut app);
    assert!(app.dev.open);
    assert_eq!(toast(&app), None);
}

#[test]
fn ctrl_shift_d_does_nothing_until_unlocked_and_then_toggles() {
    let mut app = app_with(&mock_backend_dev());
    let keys = Message::Key(
        Key::Char('d'),
        Mods {
            ctrl: true,
            shift: true,
        },
    );
    app.update_core(keys.clone());
    assert!(!app.dev.open, "locked: no entry point");
    unlock(&mut app);
    app.update_core(Message::Dev(DevMsg::Close));
    app.update_core(keys.clone());
    assert!(app.dev.open);
    app.update_core(keys);
    assert!(!app.dev.open);
}

#[test]
fn escape_closes_the_panel_first() {
    let mut app = app_with(&mock_backend_dev());
    unlock(&mut app);
    app.update_core(Message::Key(Key::Escape, Mods::default()));
    assert!(!app.dev.open);
}

#[test]
fn opening_reads_a_snapshot_and_a_tick_refreshes_only_while_open() {
    let mut app = app_with(&mock_backend_dev());
    assert!(app.dev.snap.trace.is_empty(), "nothing read while closed");
    app.update_core(Message::TimerTick);
    assert!(app.dev.snap.trace.is_empty());
    unlock(&mut app);
    assert_eq!(app.dev.snap, sample_snapshot());
}

#[test]
fn a_build_without_a_probe_opens_an_empty_panel() {
    let mut app = app_with(&mock_backend());
    unlock(&mut app);
    assert!(app.dev.open);
    assert!(app.dev.snap.trace.is_empty() && app.dev.snap.providers.is_empty());
}

#[test]
fn the_trace_filters_pick_a_provider_and_failed_calls() {
    let mut app = app_with(&mock_backend_dev());
    unlock(&mut app);
    let all = app.dev.trace_rows().len();
    assert_eq!(all, sample_snapshot().trace.len());
    app.update_core(Message::Dev(DevMsg::Provider(Some(
        "soundcloud".to_owned(),
    ))));
    assert!(
        app.dev
            .trace_rows()
            .iter()
            .all(|r| r.provider == "soundcloud")
    );
    app.update_core(Message::Dev(DevMsg::Provider(None)));
    app.update_core(Message::Dev(DevMsg::ErrorsOnly(true)));
    let failed = app.dev.trace_rows();
    assert!(!failed.is_empty() && failed.len() < all);
    assert!(failed.iter().all(|r| r.failed));
}

#[test]
fn a_selected_row_is_found_again_by_its_key() {
    let mut app = app_with(&mock_backend_dev());
    unlock(&mut app);
    let row = app.dev.snap.trace[1].clone();
    let key = (row.at_ms, row.provider.clone(), row.op.clone());
    app.update_core(Message::Dev(DevMsg::Select(Some(key))));
    assert_eq!(app.dev.selected_row(), Some(&row));
    app.update_core(Message::Dev(DevMsg::Select(None)));
    assert_eq!(app.dev.selected_row(), None);
}

#[test]
fn the_log_filters_by_level_and_text() {
    let mut app = app_with(&mock_backend_dev());
    unlock(&mut app);
    app.update_core(Message::Dev(DevMsg::Level(DevLevel::Error)));
    assert!(
        app.dev
            .log_lines()
            .iter()
            .all(|l| l.level == DevLevel::Error)
    );
    app.update_core(Message::Dev(DevMsg::Level(DevLevel::Trace)));
    let all = app.dev.log_lines().len();
    assert_eq!(all, sample_snapshot().log.len());
    app.update_core(Message::Dev(DevMsg::LogFilter("SOUNDCLOUD".to_owned())));
    let some = app.dev.log_lines();
    assert!(!some.is_empty() && some.len() < all);
}

#[test]
fn a_paused_log_keeps_its_lines_while_the_trace_goes_on() {
    let mut app = app_with(&mock_backend_dev());
    unlock(&mut app);
    app.update_core(Message::Dev(DevMsg::Pause(true)));
    let frozen = app.dev.snap.log.clone();
    let mut fresh = sample_snapshot();
    fresh.log.clear();
    fresh.taken_ms += 1000;
    app.dev.refresh(fresh);
    assert_eq!(app.dev.snap.log, frozen);
    assert_eq!(app.dev.snap.taken_ms, sample_snapshot().taken_ms + 1000);
}

#[test]
fn clear_asks_the_probe_to_empty_the_ring() {
    let probe = Arc::new(MockProbe::default());
    let backend = Backend {
        dev: Some(Arc::clone(&probe) as Arc<dyn DevProbe>),
        ..mock_backend()
    };
    let mut app = app_with(&backend);
    unlock(&mut app);
    assert!(!app.dev.snap.trace.is_empty());
    app.update_core(Message::Dev(DevMsg::Clear));
    assert_eq!(probe.clears(), 1);
    assert!(app.dev.snap.trace.is_empty());
}

#[test]
fn copy_report_writes_one_report_to_the_clipboard_and_nothing_else() {
    let probe = Arc::new(MockProbe::default());
    let backend = Backend {
        dev: Some(Arc::clone(&probe) as Arc<dyn DevProbe>),
        ..mock_backend()
    };
    let mut app = app_with(&backend);
    unlock(&mut app);
    let mut clipboard = FakeClipboard::default();
    app.dev_copy(&mut clipboard);
    assert_eq!(clipboard.0.len(), 1);
    let report = &clipboard.0[0];
    for part in [
        "NemoMusic developer report",
        "== Providers ==",
        "== Resolver ==",
        "== Account ==",
        "== System ==",
        "== Trace",
        "== Log",
        "soundcloud: calls 9, errors 3",
        "YouTube Music: signed in yes, token yes, size long",
        "in-process solver: switched itself off for a day",
        "yt-dlp: managed copy, build 3f9a0c71be42, version Unknown",
    ] {
        assert!(report.contains(part), "missing {part:?} in\n{report}");
    }
    assert_eq!(probe.clears(), 0, "copying empties nothing");
    // Through the message: the app keeps the text for the clipboard task and says `Copied`.
    app.update_core(Message::Dev(DevMsg::Copy));
    assert!(
        app.dev_copied
            .as_deref()
            .is_some_and(|t| t.starts_with("NemoMusic"))
    );
    assert_eq!(toast(&app), Some(COPIED_TOAST));
}

#[test]
fn the_report_holds_no_value_of_a_token_or_a_cookie() {
    let mut snap = sample_snapshot();
    snap.accounts[0].cookie_names = vec!["SAPISID".to_owned()];
    let text = super::report::text(&snap, &super::UiFacts::default());
    assert!(text.contains("cookies SAPISID"));
    assert!(!text.contains("SAPISID="));
}

#[test]
fn tabs_have_labels_and_an_index_each() {
    for (i, tab) in DevTab::ALL.iter().enumerate() {
        assert_eq!(tab.index(), i);
        assert_eq!(DevTab::labels()[i], tab.label());
    }
}

/// The panel never writes a file, opens a socket or logs: its sources do not even name those
/// APIs. A copy goes to the clipboard sink and nothing else.
#[test]
fn the_panel_sources_name_no_file_network_or_log_api() {
    let sources = [
        include_str!("../dev.rs"),
        include_str!("report.rs"),
        include_str!("../../views/dev.rs"),
        include_str!("../../views/dev/account.rs"),
        include_str!("../../views/dev/log.rs"),
        include_str!("../../views/dev/parts.rs"),
        include_str!("../../views/dev/providers.rs"),
        include_str!("../../views/dev/system.rs"),
        include_str!("../../views/dev/trace.rs"),
    ];
    for text in sources {
        for banned in [
            "std::fs",
            "File::",
            "OpenOptions",
            "std::net",
            "reqwest",
            "tracing::",
            "println!",
            "std::process",
        ] {
            assert!(!text.contains(banned), "found {banned}");
        }
    }
}
