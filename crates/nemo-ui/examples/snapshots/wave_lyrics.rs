//! Shots of the lyrics and song-canvas wave (`.kiro/specs/app-mvp/wave-lyrics-brief.md`):
//! a long wrapped active line in the right pane, the full-screen canvas in AMOLED + Sakura
//! (and Dark and Light for comparison), the Tools panel with the lyrics source section and the
//! Settings source list. Render only these with `wave-` names as arguments.

use nemo_ui::UiSettings;
use nemo_ui::gui::app::{NpTab, SettingsSection};
use nemo_ui::gui::headless::Scenario;
use nemo_ui::route::Route;
use nemo_ui::scheme::{BaseMode, ThemeChoice};

use super::{SIZES, Shot, named, shot};

/// Track 1, 18 s in: its third line (the long Vietnamese one) is being sung, syllable-timed.
const TRACK: usize = 1;
const AT_SECS: u64 = 18;

fn themed(base: BaseMode, theme: ThemeChoice) -> UiSettings {
    UiSettings {
        tos_acknowledged: true,
        base,
        theme,
        ..UiSettings::default()
    }
}

/// The wave's shots for both window sizes.
pub fn shots() -> Vec<Shot> {
    let mut out = Vec::new();
    for size in SIZES {
        let at = |route: Route| Scenario {
            route,
            size,
            track: TRACK,
            position_secs: AT_SECS,
            ..Scenario::default()
        };
        out.push(shot(
            named("wave-pane-wrapped", size),
            Scenario {
                np_tab: NpTab::Lyrics,
                pane_open: Some(true),
                ..at(Route::Home)
            },
        ));
        out.push(shot(
            named("wave-pane-amoled-sakura", size),
            Scenario {
                np_tab: NpTab::Lyrics,
                pane_open: Some(true),
                settings: themed(BaseMode::Amoled, ThemeChoice::Sakura),
                ..at(Route::Home)
            },
        ));
        for (stem, base) in [
            ("wave-full-amoled-sakura", BaseMode::Amoled),
            ("wave-full-dark-sakura", BaseMode::Dark),
            ("wave-full-light-sakura", BaseMode::Light),
        ] {
            out.push(shot(
                named(stem, size),
                Scenario {
                    settings: themed(base, ThemeChoice::Sakura),
                    ..at(Route::NowPlaying)
                },
            ));
        }
        out.push(shot(
            named("wave-tools-lyrics-source", size),
            Scenario {
                tools: true,
                ..at(Route::Home)
            },
        ));
        out.push(shot(
            named("wave-tools-picked", size),
            Scenario {
                tools: true,
                lyrics_pick: Some(1),
                ..at(Route::Home)
            },
        ));
        out.push(shot(
            named("wave-settings-lyrics", size),
            Scenario {
                section: SettingsSection::Lyrics,
                ..at(Route::Settings)
            },
        ));
    }
    out
}
