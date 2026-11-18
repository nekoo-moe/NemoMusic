//! Shots of the song colour theme (song-colour-theme spec): the two switches, the rule (song
//! colours on and off), and later the mesh, the heartbeat and the bloom.

use nemo_ui::UiSettings;
use nemo_ui::gui::app::SettingsSection;
use nemo_ui::gui::headless::Scenario;
use nemo_ui::route::Route;
use nemo_ui::scheme::BaseMode;

use super::{Shot, named, shot};

/// The window size of every shot here.
const SIZE: (u32, u32) = (1366, 697);

fn settings(base: BaseMode, song_colour_theme: bool) -> UiSettings {
    UiSettings {
        tos_acknowledged: true,
        base,
        song_colour_theme,
        ..UiSettings::default()
    }
}

/// The switches on Settings, and Home with the song colours on and off.
pub(super) fn shots(out: &mut Vec<Shot>) {
    for (stem, base, on) in [
        ("dark", BaseMode::Dark, true),
        ("off-dark", BaseMode::Dark, false),
    ] {
        out.push(shot(
            named(&format!("song-settings-{stem}"), SIZE),
            Scenario {
                route: Route::Settings,
                size: SIZE,
                section: SettingsSection::Appearance,
                settings: settings(base, on),
                // Motion on, so the heartbeat switch is usable unless the song colours are off.
                full_motion: true,
                ..Scenario::default()
            },
        ));
    }
    for (stem, base, on) in [
        ("on-dark", BaseMode::Dark, true),
        ("off-dark", BaseMode::Dark, false),
        ("on-light", BaseMode::Light, true),
    ] {
        out.push(shot(
            named(&format!("song-home-{stem}"), SIZE),
            Scenario {
                route: Route::Home,
                size: SIZE,
                track: 1,
                settings: settings(base, on),
                ..Scenario::default()
            },
        ));
    }
}
