//! Shots of the rebuilt Appearance page (appearance-glass). Each mode is drawn twice at 200%
//! scale (the renderer fixes the scale at 2): at 960 x 600, the smallest window, and as the whole
//! page in one tall window, since the page scrolls inside its pane. Two more show the heartbeat
//! greyed out and Window glass overridden by Reduce transparency, and the page with the window
//! glass clear over the mock wallpaper.

use std::path::Path;

use nemo_ui::UiSettings;
use nemo_ui::gui::app::SettingsSection;
use nemo_ui::gui::headless::glass::render_over;
use nemo_ui::gui::headless::{Scenario, build_app, render_png};
use nemo_ui::mock::mock_backend;
use nemo_ui::route::Route;
use nemo_ui::scheme::BaseMode;

use super::glass::wallpaper;

/// The smallest window.
const SMALL: (u32, u32) = (960, 600);
/// A window tall enough for the whole page.
const TALL: (u32, u32) = (960, 1900);

fn scenario(size: (u32, u32), settings: UiSettings) -> Scenario {
    Scenario {
        route: Route::Settings,
        section: SettingsSection::Appearance,
        size,
        settings: UiSettings {
            tos_acknowledged: true,
            ..settings
        },
        ..Scenario::default()
    }
}

fn with_base(base: BaseMode) -> UiSettings {
    UiSettings {
        base,
        ..UiSettings::default()
    }
}

/// Render the Appearance shots named in `only` (all when it is empty).
pub fn render(only: &[String], out_dir: &Path) -> Result<(), String> {
    let wanted = |name: &str| only.is_empty() || only.iter().any(|n| n == name);
    let mut plain: Vec<(String, Scenario)> = Vec::new();
    for (tag, base) in [
        ("dark", BaseMode::Dark),
        ("amoled", BaseMode::Amoled),
        ("light", BaseMode::Light),
    ] {
        plain.push((
            format!("appearance-page-{tag}-960x600"),
            scenario(SMALL, with_base(base)),
        ));
        plain.push((
            format!("appearance-page-{tag}-whole-960"),
            scenario(TALL, with_base(base)),
        ));
    }
    plain.push((
        "appearance-page-overridden-whole-960".to_owned(),
        scenario(
            TALL,
            UiSettings {
                window_glass: true,
                reduce_transparency: true,
                song_colour_theme: false,
                ..UiSettings::default()
            },
        ),
    ));
    for (name, sc) in plain {
        if !wanted(&name) {
            continue;
        }
        let app = build_app(&mock_backend(), &sc);
        let path = out_dir.join(format!("{name}.png"));
        render_png(&app, sc.size, &path).map_err(|e| format!("{name}: {e}"))?;
        println!("wrote {}", path.display());
    }
    let name = "appearance-page-glass-960x600";
    if wanted(name) {
        let sc = scenario(
            SMALL,
            UiSettings {
                window_glass: true,
                window_glass_opacity: 60,
                ..UiSettings::default()
            },
        );
        let app = build_app(&mock_backend(), &sc);
        let path = out_dir.join(format!("{name}.png"));
        render_over(&app, SMALL, &wallpaper(SMALL, false), &path, false)
            .map_err(|e| format!("{name}: {e}"))?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
