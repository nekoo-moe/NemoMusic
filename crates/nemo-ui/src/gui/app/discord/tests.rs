use std::sync::Arc;

use nemo_core::integrations::{BrowsingSection, IntegrationsService, WindowContext};
use nemo_core::opt_in::OptInInfo;
use nemo_core::services::Backend;

use super::DiscordMsg;
use crate::gui::app::Message;
use crate::gui::headless::{Scenario, build_app};
use crate::mock::{MockIntegrations, mock_backend};
use crate::route::Route;
use crate::state::Event;

const INFO: OptInInfo = OptInInfo {
    id: "discord",
    title: "Discord presence",
    sends: "The title and artist of the song you play, to the Discord app on this computer.",
    hosts: &[],
    risk: "They are shown on your Discord profile.",
    version: 1,
    default_on: false,
};

fn backend(service: &Arc<MockIntegrations>) -> Backend {
    Backend {
        integrations: Arc::clone(service) as _,
        ..mock_backend()
    }
}

fn app() -> (crate::gui::app::App, Arc<MockIntegrations>) {
    let service = Arc::new(MockIntegrations::new(INFO));
    let app = build_app(&backend(&service), &Scenario::default());
    (app, service)
}

fn press(app: &mut crate::gui::app::App, msg: DiscordMsg) {
    drop(app.update(Message::Discord(msg)));
}

#[test]
fn an_app_without_discord_offers_no_row() {
    let app = build_app(&mock_backend(), &Scenario::default());
    assert!(app.discord_info.is_none());
    assert!(!app.discord_on());
}

#[test]
fn the_switch_asks_first_and_turns_on_only_after_turn_on() {
    let (mut app, service) = app();
    assert!(app.discord_info.is_some());
    press(&mut app, DiscordMsg::Ask);
    assert!(app.discord_confirming);
    assert!(!service.discord(), "asking does not turn it on");
    press(&mut app, DiscordMsg::Cancel);
    assert!(!app.discord_confirming && !service.discord());
    press(&mut app, DiscordMsg::Ask);
    press(&mut app, DiscordMsg::TurnOn);
    assert!(!app.discord_confirming);
    assert!(service.discord() && app.discord_on());
    press(&mut app, DiscordMsg::TurnOff);
    assert!(!service.discord());
}

#[test]
fn hide_song_names_is_its_own_switch() {
    let (mut app, service) = app();
    assert!(!app.discord_hidden());
    press(&mut app, DiscordMsg::Hide(true));
    assert!(service.discord_private() && app.discord_hidden());
    assert!(!service.discord(), "hiding does not turn presence on");
    press(&mut app, DiscordMsg::Hide(false));
    assert!(!app.discord_hidden());
}

#[test]
fn the_window_and_the_sleep_timer_are_reported() {
    let (mut app, service) = app();
    assert_eq!(service.last_window(), Some(WindowContext::default()));
    drop(app.update(Message::Focus(false)));
    assert_eq!(service.last_window().map(|w| w.focused), Some(false));
    drop(app.update(Message::Focus(true)));

    drop(app.update(Message::Ui(Event::Go(Route::Library))));
    assert_eq!(
        service.last_window().and_then(|w| w.section),
        Some(BrowsingSection::Library)
    );
    drop(app.update(Message::Ui(Event::Go(Route::Search("miles".into())))));
    assert_eq!(
        service.last_window().and_then(|w| w.section),
        Some(BrowsingSection::Search)
    );
    drop(app.update(Message::Ui(Event::Go(Route::Settings))));
    assert_eq!(service.last_window().and_then(|w| w.section), None);
}

#[test]
fn the_privacy_page_builds_with_and_without_the_row() {
    let (mut app, _service) = app();
    app.settings_section = crate::gui::app::SettingsSection::Privacy;
    app.dispatch(Event::Go(Route::Settings));
    drop(app.view());
    press(&mut app, DiscordMsg::Ask);
    drop(app.view());
    let mut plain = build_app(&mock_backend(), &Scenario::default());
    plain.settings_section = crate::gui::app::SettingsSection::Privacy;
    plain.dispatch(Event::Go(Route::Settings));
    drop(plain.view());
}
