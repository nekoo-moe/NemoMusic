//! Layout checks of the Appearance page. They build a headless renderer, so they are ignored by
//! default: run with `ICED_TEST_BACKEND=tiny-skia cargo test -p nemo-ui -- --ignored appearance`.

use iced::{Pixels, Rectangle, Size};
use iced_test::Simulator;
use iced_test::selector::Candidate;

use super::controls::{TARGET, segmented_width};
use crate::copy::appearance as words;
use crate::gui::app::{App, Message, SettingsSection};
use crate::gui::assets;
use crate::gui::headless::{Scenario, build_app};
use crate::mock::mock_backend;
use crate::route::Route;
use crate::settings::UiSettings;

fn app(settings: UiSettings) -> App {
    app_with(settings, false)
}

/// The page after the Window glass switch was turned on in a window created solid.
fn app_needing_restart() -> App {
    app_with(UiSettings::default(), true)
}

fn app_with(settings: UiSettings, glass_switched_on: bool) -> App {
    build_app(
        &mock_backend(),
        &Scenario {
            size: (960, 2100),
            glass_switched_on,
            route: Route::Settings,
            section: SettingsSection::Appearance,
            settings: UiSettings {
                tos_acknowledged: true,
                ..settings
            },
            ..Scenario::default()
        },
    )
}

/// Every focusable control and every text of the whole window at `size`.
fn scan(app: &App, size: (f32, f32)) -> (Vec<Rectangle>, Vec<String>) {
    let settings = iced::Settings {
        fonts: assets::ALL_FONTS.iter().map(|f| (*f).into()).collect(),
        default_font: assets::font(false),
        default_text_size: Pixels(14.0),
        ..iced::Settings::default()
    };
    let mut sim: Simulator<'_, Message> =
        Simulator::with_size(settings, Size::new(size.0, size.1), app.view());
    let (mut focusable, mut texts) = (Vec::new(), Vec::new());
    // The selector never matches, so it visits the whole tree; the error is expected.
    let _ = sim.find(|candidate: Candidate<'_>| -> Option<()> {
        match candidate {
            Candidate::Focusable { bounds, .. } => focusable.push(bounds),
            Candidate::Text { content, .. } => texts.push(content.to_owned()),
            _ => {}
        }
        None
    });
    (focusable, texts)
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn every_target_is_32_px_and_the_switches_say_on_or_off() {
    for size in [(960.0, 2100.0), (1366.0, 2100.0)] {
        let (focusable, texts) = scan(&app(UiSettings::default()), size);
        assert!(
            !texts.iter().any(|t| t == words::RESTART_PILL),
            "no pill while nothing changed, at {size:?}"
        );
        assert!(
            texts.iter().any(|t| t == "On") && texts.iter().any(|t| t == "Off"),
            "switches show On and Off as text"
        );
        // The settings column starts right of the section list and ends above the player bar;
        // the page's own controls are the focusable boxes there.
        let page: Vec<_> = focusable
            .iter()
            .filter(|b| b.x > 300.0 && b.y > 200.0 && b.y < 1950.0)
            .collect();
        assert!(page.len() > 20, "{} controls found", page.len());
        for b in page {
            assert!(b.height >= TARGET - 0.5, "{b:?} is under 32 px high");
        }
    }
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn reduce_transparency_shows_the_window_glass_switch_as_overridden() {
    let on = UiSettings {
        window_glass: true,
        reduce_transparency: true,
        ..UiSettings::default()
    };
    let (_, texts) = scan(&app(on), (960.0, 2100.0));
    assert!(texts.iter().any(|t| t == "Overridden"));
    assert!(
        texts
            .iter()
            .any(|t| t.contains("Reduce transparency is on"))
    );
    let (_, texts) = scan(&app(UiSettings::default()), (960.0, 2100.0));
    assert!(!texts.iter().any(|t| t == "Overridden"));
}

#[test]
fn the_width_estimate_grows_with_the_labels_and_the_gaps() {
    assert!(
        segmented_width(&["Dark", "Light"]) < segmented_width(&["Dark", "AMOLED black", "Light"])
    );
    assert!((segmented_width(&["Soft"]) - (4.0 * 7.5 + 54.0)).abs() < 1e-3);
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn the_pill_shows_with_its_helper_only_when_the_switch_needs_a_restart() {
    let size = (960.0, 2100.0);
    let (focusable, texts) = scan(&app_needing_restart(), size);
    assert!(texts.iter().any(|t| t == words::RESTART_PILL));
    assert!(texts.iter().any(|t| t == words::RESTART_HELPER));
    let pill = focusable
        .iter()
        .find(|b| (b.height - 40.0).abs() < 0.5 && b.width > 100.0)
        .expect("a 40 px target");
    assert!(pill.width >= 100.0, "{pill:?}");
    // Nothing changed: no pill. Reduce transparency overrides the switch: no pill.
    for settings in [
        UiSettings::default(),
        UiSettings {
            window_glass: true,
            reduce_transparency: true,
            ..UiSettings::default()
        },
    ] {
        let (_, texts) = scan(&app(settings), size);
        assert!(!texts.iter().any(|t| t == words::RESTART_PILL));
        assert!(!texts.iter().any(|t| t == words::RESTART_HELPER));
    }
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn a_click_on_the_pill_sends_one_restart_request() {
    let app = app_needing_restart();
    let settings = iced::Settings {
        fonts: assets::ALL_FONTS.iter().map(|f| (*f).into()).collect(),
        default_font: assets::font(false),
        default_text_size: Pixels(14.0),
        ..iced::Settings::default()
    };
    let mut sim: Simulator<'_, Message> =
        Simulator::with_size(settings, Size::new(960.0, 2100.0), app.view());
    sim.click(words::RESTART_PILL).expect("the pill");
    let sent: Vec<Message> = sim.into_messages().collect();
    assert!(matches!(sent.as_slice(), [Message::Restart]), "{sent:?}");
}
