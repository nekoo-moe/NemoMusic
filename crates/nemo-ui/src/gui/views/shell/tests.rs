//! The sidebar caption drawn headless (source-workspaces spec). Ignored by default: run with
//! `ICED_TEST_BACKEND=tiny-skia cargo test -p nemo-ui --features gui -- sidebar_caption
//! --include-ignored`.

use iced::Size;
use iced_test::Simulator;
use nemo_core::source_account::SourceAccountState;

use crate::copy::soundcloud as text;
use crate::gui::app::{App, Message, Source};
use crate::gui::assets;
use crate::gui::headless::soundcloud::ScScene;
use crate::gui::headless::{Scenario, build_app};
use crate::mock::{MockScAccount, mock_backend_soundcloud};
use crate::settings::UiSettings;

fn app(area: Source) -> App {
    build_app(
        &mock_backend_soundcloud(SourceAccountState::SignedIn(MockScAccount::info())),
        &Scenario {
            size: (1366, 697),
            soundcloud: Some(ScScene::Account),
            area: Some(area),
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
    Simulator::with_size(settings, Size::new(1366.0, 697.0), app.view())
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn sidebar_caption_names_soundcloud_only_while_it_is_the_area() {
    let yt = app(Source::YouTube);
    assert!(simulator(&yt).find(text::AREA_CAPTION).is_err());
    let sc = app(Source::SoundCloud);
    assert!(simulator(&sc).find(text::AREA_CAPTION).is_ok());
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn no_switch_is_drawn_on_home_the_top_bar_the_sidebar_or_the_player_bar() {
    // The only places the control is drawn are the accounts sheet and Settings; the help line
    // under it appears nowhere on a normal screen.
    for area in Source::ALL {
        let app = app(area);
        assert!(simulator(&app).find(text::AREA_HELP).is_err(), "{area:?}");
        assert!(simulator(&app).find(text::AREA_TITLE).is_err(), "{area:?}");
    }
}
