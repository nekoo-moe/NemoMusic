//! Shots of the floating player capsule (player-bar-redesign). Each bar shot renders the whole
//! window and keeps the bottom strip, so the capsule shows over the real window colour; two
//! full-window shots show how the panes end above it.

use std::path::Path;

use nemo_ui::UiSettings;
use nemo_ui::gui::headless::{Scenario, build_app, render_png};
use nemo_ui::mock::mock_backend;
use nemo_ui::scheme::BaseMode;

/// Logical px kept from the bottom of the window: the capsule, its margin and the shadow.
const STRIP: u32 = 124;
/// `iced_test` draws at this scale.
const SCALE: u32 = 2;

/// One shot: name, window size, scenario, and whether to keep only the bottom strip.
struct Shot {
    name: &'static str,
    scenario: Scenario,
    strip: bool,
}

fn scenario(size: (u32, u32), track: usize) -> Scenario {
    Scenario {
        size,
        track,
        position_secs: 83,
        playing: true,
        settings: UiSettings {
            tos_acknowledged: true,
            ..UiSettings::default()
        },
        ..Scenario::default()
    }
}

fn with_base(mut s: Scenario, base: BaseMode) -> Scenario {
    s.settings.base = base;
    s
}

fn shots() -> Vec<Shot> {
    let wide = (1280, 800);
    let mut paused = scenario(wide, 0);
    paused.playing = false;
    let mut buffering = scenario(wide, 0);
    buffering.buffering = true;
    let mut off = scenario(wide, 0);
    off.settings.reduce_transparency = true;
    let mut repeat_one = scenario(wide, 0);
    repeat_one.repeat = nemo_core::player::RepeatMode::One;
    vec![
        Shot {
            name: "player-bar-playing-1280",
            scenario: scenario(wide, 0),
            strip: true,
        },
        Shot {
            name: "player-bar-paused-1280",
            scenario: paused,
            strip: true,
        },
        Shot {
            name: "player-bar-buffering-1280",
            scenario: buffering,
            strip: true,
        },
        Shot {
            name: "player-bar-repeat-one-1280",
            scenario: repeat_one,
            strip: true,
        },
        Shot {
            name: "player-bar-1100",
            scenario: scenario((1100, 700), 0),
            strip: true,
        },
        Shot {
            name: "player-bar-960",
            scenario: scenario((960, 600), 0),
            strip: true,
        },
        Shot {
            name: "player-bar-amoled-1280",
            scenario: with_base(scenario(wide, 0), BaseMode::Amoled),
            strip: true,
        },
        Shot {
            name: "player-bar-light-1280",
            scenario: with_base(scenario(wide, 0), BaseMode::Light),
            strip: true,
        },
        Shot {
            name: "player-bar-glass-off-1280",
            scenario: off,
            strip: true,
        },
        Shot {
            name: "player-bar-cover-b-1280",
            scenario: scenario(wide, 1),
            strip: true,
        },
        Shot {
            name: "player-bar-cover-c-light-1280",
            scenario: with_base(scenario(wide, 2), BaseMode::Light),
            strip: true,
        },
        Shot {
            name: "player-bar-window-1280x800",
            scenario: scenario(wide, 0),
            strip: false,
        },
        Shot {
            name: "player-bar-window-960x600",
            scenario: scenario((960, 600), 0),
            strip: false,
        },
    ]
}

/// Render the capsule shots named in `only` (all when empty) into `out_dir`.
pub(super) fn render(only: &[String], out_dir: &Path) -> Result<(), String> {
    for s in shots() {
        if !only.is_empty() && !only.iter().any(|n| n == s.name) {
            continue;
        }
        let backend = mock_backend();
        let app = build_app(&backend, &s.scenario);
        let path = out_dir.join(format!("{}.png", s.name));
        render_png(&app, s.scenario.size, &path).map_err(|e| format!("{}: {e}", s.name))?;
        if s.strip {
            crop_bottom(&path, STRIP * SCALE).map_err(|e| format!("{}: {e}", s.name))?;
        }
        println!("wrote {}", path.display());
    }
    Ok(())
}

/// Keep only the bottom `rows` pixel rows of the PNG at `path`.
fn crop_bottom(path: &Path, rows: u32) -> Result<(), String> {
    let img = image::open(path).map_err(|e| e.to_string())?;
    let rows = rows.min(img.height());
    let cut = img.crop_imm(0, img.height() - rows, img.width(), rows);
    cut.save(path).map_err(|e| e.to_string())
}
