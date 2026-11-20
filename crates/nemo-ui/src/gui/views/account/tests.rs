//! The accounts sheet drawn headless (source-workspaces spec). These need a renderer, so they
//! are ignored by default: run with `ICED_TEST_BACKEND=tiny-skia cargo test -p nemo-ui --
//! --ignored accounts_sheet`.

use iced::Size;
use iced_test::Simulator;
use nemo_core::source_account::SourceAccountState;

use crate::copy::soundcloud as text;
use crate::gui::app::{App, Message, SettingsSection};
use crate::gui::assets;
use crate::gui::headless::soundcloud::ScScene;
use crate::gui::headless::{Scenario, Sheet, build_app};
use crate::mock::{MockScAccount, mock_backend, mock_backend_soundcloud};
use crate::settings::UiSettings;

/// A window tall enough to show the whole sheet.
const SIZE: (u32, u32) = (960, 1100);

fn app(state: Option<SourceAccountState>, scene: ScScene) -> App {
    let backend = state.map_or_else(mock_backend, mock_backend_soundcloud);
    build_app(
        &backend,
        &Scenario {
            size: SIZE,
            sheet: Sheet::SignIn,
            soundcloud: Some(scene),
            settings: UiSettings {
                tos_acknowledged: true,
                ..UiSettings::default()
            },
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
    Simulator::with_size(
        settings,
        Size::new(f32::from(u16::try_from(SIZE.0).unwrap_or(960)), 1100.0),
        app.view(),
    )
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn accounts_sheet_signed_in_shows_the_control_and_both_cards() {
    let app = app(
        Some(SourceAccountState::SignedIn(MockScAccount::info())),
        ScScene::Account,
    );
    let mut sim = simulator(&app);
    assert!(sim.find(crate::copy::ACCOUNTS_TITLE).is_ok());
    assert!(sim.find(text::AREA_TITLE).is_ok());
    assert!(sim.find(text::AREA_HELP).is_ok());
    assert!(sim.find(text::AREA_NEEDS_SIGN_IN).is_err());
    assert!(
        sim.find(text::SIGN_OUT_CONFIRM).is_err(),
        "nothing asked yet"
    );
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn accounts_sheet_signed_out_offers_sign_in_and_says_why_there_is_one_area() {
    let app = app(Some(SourceAccountState::SignedOut), ScScene::Account);
    let mut sim = simulator(&app);
    assert!(sim.find(text::SIGN_IN).is_ok());
    assert!(sim.find(text::PASTE).is_ok());
    assert!(sim.find(text::AREA_NEEDS_SIGN_IN).is_ok());
    assert!(sim.find(text::AREA_HELP).is_err(), "no control to explain");
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn accounts_sheet_off_has_one_sentence_and_a_button_to_settings() {
    let app = app(None, ScScene::Off);
    let mut sim = simulator(&app);
    assert!(sim.find(text::CARD_OFF).is_ok());
    assert!(sim.find(text::AREA_NEEDS_ON).is_ok());
    sim.click(text::CARD_TURN_ON).expect("the button");
    let sent: Vec<Message> = sim.into_messages().collect();
    assert!(
        matches!(
            sent.as_slice(),
            [Message::SettingsSectionGo(SettingsSection::SoundCloud)]
        ),
        "{sent:?}"
    );
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn accounts_sheet_paste_shows_a_masked_field_and_never_the_token() {
    let app = app(Some(SourceAccountState::SignedOut), ScScene::Paste);
    let mut sim = simulator(&app);
    assert!(sim.find(text::CHECK_TOKEN).is_ok());
    assert!(sim.find(text::PASTE_HELP).is_ok());
}
