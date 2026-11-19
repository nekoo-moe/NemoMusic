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
    build_app(
        &mock_backend(),
        &Scenario {
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
fn the_page_names_the_restart_and_every_target_is_32_px() {
    for size in [(960.0, 2100.0), (1366.0, 2100.0)] {
        let (focusable, texts) = scan(&app(UiSettings::default()), size);
        assert!(
            texts.iter().any(|t| t == words::WINDOW_GLASS_RESTART),
            "the restart note is on the page at {size:?}"
        );
        assert!(
            texts.iter().any(|t| t == "On") && texts.iter().any(|t| t == "Off"),
            "switches show On and Off as text"
        );
        // The settings column starts right of the section list; the page's own controls are
        // the focusable boxes there.
        let page: Vec<_> = focusable
            .iter()
            .filter(|b| b.x > 300.0 && b.y > 200.0)
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
