//! Layout checks of the SoundCloud page that need a headless renderer, so they are ignored by
//! default: run with `ICED_TEST_BACKEND=tiny-skia cargo test -p nemo-ui --features gui --
//! --ignored soundcloud`.

use iced::Size;
use iced_test::Simulator;
use nemo_core::source_account::SourceAccountState;

use crate::copy::appearance as words;
use crate::copy::soundcloud as text;
use crate::gui::app::{App, Message, ScMsg, SettingsSection};
use crate::gui::assets;
use crate::gui::headless::soundcloud::ScScene;
use crate::gui::headless::{Scenario, build_app};
use crate::mock::{MockScAccount, mock_backend, mock_backend_soundcloud};
use crate::route::Route;
use crate::settings::UiSettings;

fn app(scene: ScScene, running: bool) -> App {
    let backend = if running {
        mock_backend_soundcloud(SourceAccountState::SignedIn(MockScAccount::info()))
    } else {
        mock_backend()
    };
    build_app(
        &backend,
        &Scenario {
            size: (960, 1800),
            route: Route::Settings,
            section: SettingsSection::SoundCloud,
            settings: UiSettings {
                tos_acknowledged: true,
                ..UiSettings::default()
            },
            soundcloud: Some(scene),
            ..Scenario::default()
        },
    )
}

fn simulator(app: &App) -> Simulator<'_, Message> {
    let settings = iced::Settings {
        fonts: assets::ALL_FONTS.iter().map(|f| (*f).into()).collect(),
        default_font: assets::font(false),
        default_text_size: iced::Pixels(14.0),
        ..iced::Settings::default()
    };
    Simulator::with_size(settings, Size::new(960.0, 1800.0), app.view())
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn a_click_on_the_soundcloud_pill_sends_one_restart_request() {
    let app = app(ScScene::Restart, false);
    assert!(app.sc_restart_offered());
    let mut sim = simulator(&app);
    sim.click(words::RESTART_PILL).expect("the pill");
    let sent: Vec<Message> = sim.into_messages().collect();
    assert!(matches!(sent.as_slice(), [Message::Restart]), "{sent:?}");
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn no_pill_while_the_services_run() {
    let app = app(ScScene::Account, true);
    let mut sim = simulator(&app);
    assert!(sim.find(words::RESTART_PILL).is_err());
    assert!(sim.find(text::OFF_CONFIRM).is_err(), "nothing asked yet");
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn turning_off_while_signed_in_shows_the_question_under_the_switch() {
    let app = app(ScScene::ConfirmOff, true);
    assert!(app.sc.confirm_off);
    let mut sim = simulator(&app);
    assert!(sim.find(text::OFF_CONFIRM).is_ok(), "the sentence");
    sim.click(text::OFF_CONFIRM_BUTTON).expect("the button");
    let sent: Vec<Message> = sim.into_messages().collect();
    assert!(
        matches!(sent.as_slice(), [Message::Sc(ScMsg::ConfirmOff)]),
        "{sent:?}"
    );
}
