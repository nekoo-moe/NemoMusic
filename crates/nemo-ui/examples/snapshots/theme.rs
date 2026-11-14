//! Shots of the surface modes and palettes, on Settings and on Home.

use nemo_ui::UiSettings;
use nemo_ui::gui::app::SettingsSection;
use nemo_ui::gui::headless::Scenario;
use nemo_ui::route::Route;
use nemo_ui::scheme::{BaseMode, ThemeChoice};

use super::{SIZES, Shot, named, shot};

/// Every surface mode and a few palettes.
#[allow(clippy::too_many_lines)] // a flat list of scenarios
pub(super) fn themes(out: &mut Vec<Shot>) {
    // Themes (round 3): every surface mode and a few palettes, on Settings and on Home.
    for size in SIZES {
        let themed =
            |base: BaseMode, theme: ThemeChoice, route: Route, section: SettingsSection| Scenario {
                route,
                size,
                section,
                settings: UiSettings {
                    tos_acknowledged: true,
                    base,
                    theme,
                    ..UiSettings::default()
                },
                ..Scenario::default()
            };
        for (stem, base, theme) in [
            ("dark", BaseMode::Dark, ThemeChoice::Dynamic),
            ("amoled", BaseMode::Amoled, ThemeChoice::Ocean),
            ("light", BaseMode::Light, ThemeChoice::Sakura),
            ("forest", BaseMode::Dark, ThemeChoice::Forest),
        ] {
            out.push(shot(
                named(&format!("appearance-{stem}"), size),
                themed(base, theme, Route::Settings, SettingsSection::Appearance),
            ));
            out.push(shot(
                named(&format!("theme-home-{stem}"), size),
                themed(base, theme, Route::Home, SettingsSection::Playback),
            ));
        }
    }
}
