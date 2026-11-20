//! Renders each tab of the panel from the sample session and clicks its buttons. These need a
//! headless renderer, so they are ignored by default: run with
//! `ICED_TEST_BACKEND=tiny-skia cargo test -p nemo-ui --features gui -- --ignored dev::`.

use iced::Size;
use iced_test::Simulator;

use crate::gui::app::dev::{DevMsg, DevTab};
use crate::gui::app::{App, Message};
use crate::gui::assets;
use crate::gui::headless::{Scenario, build_app};
use crate::mock::mock_backend_dev;

fn app(tab: DevTab, size: (u32, u32)) -> App {
    build_app(
        &mock_backend_dev(),
        &Scenario {
            size,
            dev: Some(tab),
            ..Scenario::default()
        },
    )
}

fn simulator(app: &App, size: (u32, u32)) -> Simulator<'_, Message> {
    let settings = iced::Settings {
        fonts: assets::ALL_FONTS.iter().map(|f| (*f).into()).collect(),
        default_font: assets::font(false),
        default_text_size: iced::Pixels(14.0),
        ..iced::Settings::default()
    };
    #[allow(clippy::cast_precision_loss)]
    Simulator::with_size(
        settings,
        Size::new(size.0 as f32, size.1 as f32),
        app.view(),
    )
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn every_tab_draws_its_facts_at_the_smallest_window() {
    let size = (960, 600);
    for (tab, expected) in [
        (DevTab::Trace, "api-v2.soundcloud.com"),
        (DevTab::Log, "native resolver rests after repeated refusals"),
        (DevTab::Providers, "lyrics:lrclib"),
        (DevTab::Account, "Cookie names"),
        (
            DevTab::System,
            "Speakers (USB DAC), 48000 Hz, 24 bit, 2 ch, Shared",
        ),
    ] {
        let app = app(tab, size);
        assert!(app.dev.open);
        let mut sim = simulator(&app, size);
        assert!(sim.find("Developer tools").is_ok(), "{tab:?}: title");
        assert!(sim.find("Copy report").is_ok(), "{tab:?}: copy");
        assert!(sim.find(expected).is_ok(), "{tab:?}: {expected}");
    }
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn the_buttons_send_their_messages() {
    let size = (960, 600);
    let app = app(DevTab::Trace, size);
    let mut sim = simulator(&app, size);
    sim.click("Copy report").expect("copy");
    sim.click("Clear").expect("clear");
    sim.click("Errors only").expect("errors only");
    sim.click("Providers").expect("a tab");
    let sent: Vec<Message> = sim.into_messages().collect();
    assert!(
        matches!(
            sent.as_slice(),
            [
                Message::Dev(DevMsg::Copy),
                Message::Dev(DevMsg::Clear),
                Message::Dev(DevMsg::ErrorsOnly(true)),
                Message::Dev(DevMsg::Tab(2)),
            ]
        ),
        "{sent:?}"
    );
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn the_about_text_is_a_click_target_that_looks_like_any_other() {
    use crate::gui::app::SettingsSection;
    use crate::route::Route;
    use crate::settings::UiSettings;
    let size = (960, 600);
    let app = build_app(
        &mock_backend_dev(),
        &Scenario {
            size,
            route: Route::Settings,
            section: SettingsSection::About,
            settings: UiSettings {
                tos_acknowledged: true,
                ..UiSettings::default()
            },
            ..Scenario::default()
        },
    );
    assert!(!app.dev.open);
    let mut sim = simulator(&app, size);
    let version = format!("Version {}", env!("CARGO_PKG_VERSION"));
    sim.click(version.as_str()).expect("the version text");
    let sent: Vec<Message> = sim.into_messages().collect();
    assert!(
        matches!(sent.as_slice(), [Message::Dev(DevMsg::Click)]),
        "{sent:?}"
    );
}
