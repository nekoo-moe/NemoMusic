//! Tests of the setup flow state machine.

use super::*;
use crate::route::Route;
use crate::settings::UiSettings;
use crate::state::Event;
use nemo_core::player::PlayerSnapshot;

fn state(settings: UiSettings) -> UiState {
    UiState::new(Route::Home, PlayerSnapshot::default(), false).with_settings(settings)
}

fn open() -> UiState {
    let mut s = state(UiSettings::default());
    s.apply(Event::Setup(SetupEvent::Open));
    s
}

fn go_to(s: &mut UiState, step: SetupStep) {
    while s.setup.step() != step {
        if s.setup.step() == SetupStep::Notice {
            s.apply(Event::Setup(SetupEvent::NoticeLater));
        } else {
            s.apply(Event::Setup(SetupEvent::Next));
        }
    }
}

#[test]
fn a_new_user_needs_setup_and_an_existing_user_does_not() {
    assert!(UiSettings::default().needs_setup());
    let accepted = UiSettings {
        tos_acknowledged: true,
        ..UiSettings::default()
    };
    assert!(!accepted.needs_setup(), "the notice was accepted before");
    let done = UiSettings {
        setup_completed: true,
        ..UiSettings::default()
    };
    assert!(!done.needs_setup());
    // Not showing until it is opened: the reducer starts with no flow.
    assert!(!state(UiSettings::default()).setup.active());
}

#[test]
fn next_and_back_walk_the_steps_and_stop_at_the_ends() {
    let mut s = open();
    assert_eq!(s.setup.step(), SetupStep::Welcome);
    s.apply(Event::Setup(SetupEvent::Back));
    assert_eq!(s.setup.step(), SetupStep::Welcome);
    s.apply(Event::Setup(SetupEvent::Next));
    assert_eq!(s.setup.step(), SetupStep::Look);
    assert!(s.setup.forward());
    s.apply(Event::Setup(SetupEvent::Back));
    assert_eq!(s.setup.step(), SetupStep::Welcome);
    assert!(!s.setup.forward());
    for step in SetupStep::ALL {
        assert_eq!(SetupStep::ALL[step.index()], step);
    }
    assert_eq!(SetupStep::Done.next(), None);
    assert_eq!(SetupStep::Welcome.prev(), None);
}

#[test]
fn skip_is_always_available_and_stores_completion_once() {
    for step in SetupStep::ALL {
        let mut s = open();
        go_to(&mut s, step);
        let fx = s.apply(Event::Setup(SetupEvent::Skip));
        assert!(!s.setup.active(), "skip from {step:?}");
        assert!(s.settings.setup_completed);
        assert!(matches!(&fx[..], [Effect::SettingsChanged(x)] if x.setup_completed));
    }
    // Running it again and skipping again stores nothing new.
    let mut s = open();
    s.apply(Event::Setup(SetupEvent::Skip));
    s.apply(Event::Setup(SetupEvent::Open));
    assert!(s.setup.active());
    assert!(s.apply(Event::Setup(SetupEvent::Skip)).is_empty());
}

#[test]
fn skipping_leaves_the_notice_for_the_first_run_sheet() {
    let mut s = open();
    assert!(!s.tos_sheet_visible(), "the flow covers the sheet");
    s.apply(Event::Setup(SetupEvent::Skip));
    assert!(s.tos_sheet_visible() && !s.youtube_enabled());
}

#[test]
fn the_notice_is_unticked_and_next_waits_for_an_answer() {
    let mut s = open();
    go_to(&mut s, SetupStep::Notice);
    assert!(!s.tos_checked, "never pre-ticked");
    s.apply(Event::Setup(SetupEvent::Next));
    s.apply(Event::Setup(SetupEvent::Enter));
    assert_eq!(s.setup.step(), SetupStep::Notice);
    // Continue without the tick does nothing.
    assert!(s.apply(Event::Setup(SetupEvent::NoticeContinue)).is_empty());
    assert_eq!(s.setup.step(), SetupStep::Notice);
    assert!(!s.settings.tos_acknowledged);
}

#[test]
fn ticking_and_continuing_accepts_the_notice_and_moves_on() {
    let mut s = open();
    go_to(&mut s, SetupStep::Notice);
    s.apply(Event::TosCheck(true));
    let fx = s.apply(Event::Setup(SetupEvent::Enter));
    assert!(matches!(&fx[..], [Effect::SettingsChanged(x)] if x.tos_acknowledged));
    assert_eq!(s.setup.step(), SetupStep::Sources);
    assert!(s.youtube_enabled());
}

#[test]
fn not_now_moves_on_without_accepting() {
    let mut s = open();
    go_to(&mut s, SetupStep::Notice);
    s.apply(Event::Setup(SetupEvent::NoticeLater));
    assert_eq!(s.setup.step(), SetupStep::Sources);
    assert!(!s.youtube_enabled() && s.tos_dismissed);
    // Finishing does not bring the sheet back this session, as with `Not now` on the sheet.
    s.apply(Event::Setup(SetupEvent::Finish));
    assert!(!s.tos_sheet_visible());
}

#[test]
fn enter_is_next_and_on_the_last_step_it_finishes() {
    let mut s = open();
    s.apply(Event::Setup(SetupEvent::Enter));
    assert_eq!(s.setup.step(), SetupStep::Look);
    go_to(&mut s, SetupStep::Done);
    let fx = s.apply(Event::Setup(SetupEvent::Enter));
    assert!(!s.setup.active());
    assert!(matches!(&fx[..], [Effect::SettingsChanged(x)] if x.setup_completed));
}

#[test]
fn running_setup_again_resets_nothing() {
    let mut settings = UiSettings {
        tos_acknowledged: true,
        setup_completed: true,
        sponsorblock: false,
        ..UiSettings::default()
    };
    settings.volume = 40;
    let mut s = state(settings.clone());
    s.apply(Event::Setup(SetupEvent::Open));
    assert!(s.setup.active() && s.setup.step() == SetupStep::Welcome);
    assert!(s.apply(Event::Setup(SetupEvent::Next)).is_empty());
    assert!(s.apply(Event::Setup(SetupEvent::Skip)).is_empty());
    assert_eq!(s.settings, settings);
    assert!(s.notice_decided());
}

#[test]
fn events_do_nothing_while_the_flow_is_closed() {
    let mut s = state(UiSettings::default());
    for ev in [
        SetupEvent::Next,
        SetupEvent::Back,
        SetupEvent::Skip,
        SetupEvent::Finish,
        SetupEvent::Enter,
        SetupEvent::NoticeContinue,
        SetupEvent::NoticeLater,
    ] {
        assert!(s.apply(Event::Setup(ev)).is_empty());
    }
    assert!(!s.settings.setup_completed && !s.tos_dismissed);
}

#[test]
fn the_sources_choices_are_remembered_until_the_flow_opens_again() {
    let mut s = open();
    s.apply(Event::Setup(SetupEvent::UseWithoutAccount));
    s.apply(Event::Setup(SetupEvent::SkipFolders));
    assert!(s.setup.without_account() && s.setup.folders_skipped());
    s.apply(Event::Setup(SetupEvent::Skip));
    s.apply(Event::Setup(SetupEvent::Open));
    assert!(!s.setup.without_account() && !s.setup.folders_skipped());
}

#[test]
fn the_summary_names_each_choice_and_never_an_id() {
    let mut settings = UiSettings {
        tos_acknowledged: true,
        block_ads: false,
        ..UiSettings::default()
    };
    let mut setup = Setup::default();
    let lines = summary(&settings, &setup, None, 0);
    let get = |k: &str| {
        lines
            .iter()
            .find(|(what, _)| *what == k)
            .map(|(_, v)| v.clone())
    };
    assert_eq!(get("YouTube Music").as_deref(), Some("Not signed in"));
    assert_eq!(get("Ad blocking").as_deref(), Some("Off"));
    assert_eq!(get("SponsorBlock").as_deref(), Some("On"));
    assert_eq!(get("Local music").as_deref(), Some("No folder added"));
    assert_eq!(get("SoundCloud").as_deref(), Some("Off"));
    setup.open();
    setup.without_account = true;
    let lines = summary(&settings, &setup, None, 2);
    assert!(lines.contains(&("YouTube Music", "Without an account".to_owned())));
    assert!(lines.contains(&("Local music", "2 folders".to_owned())));
    let lines = summary(&settings, &setup, Some("Sam"), 1);
    assert!(lines.contains(&("YouTube Music", "Signed in as Sam".to_owned())));
    settings.tos_acknowledged = false;
    let lines = summary(&settings, &Setup::default(), None, 0);
    assert!(lines.contains(&("YouTube Music", "Waiting for the notice".to_owned())));
}
