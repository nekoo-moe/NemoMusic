//! Shots of the rebuilt Appearance page (appearance-glass). Each mode is drawn twice at 200%
//! scale (the renderer fixes the scale at 2): at 960 x 600, the smallest window, and as the whole
//! page in one tall window, since the page scrolls inside its pane. Two more show the breathing switch
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
const TALL: (u32, u32) = (960, 2100);

fn scenario(size: (u32, u32), settings: UiSettings) -> Scenario {
    Scenario {
        // Motion on, so the breathing switch shows usable; the overridden shot turns it off.
        full_motion: true,
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

/// The Window glass card in the tall page, in 2x px: left, top, width, height.
const GLASS_CARD: (u32, u32, u32, u32) = (672, 2840, 1216, 620);

/// The Window glass card with the `Restart now` pill (bar-material spec): the page after the
/// switch was turned on in a window created solid. The first four are key frames of the pill's
/// entrance, at 20, 60, 120 and 220 ms (it rises 8 px and fades in over 200 ms); the last two are
/// settled, in dark and light.
fn restart_shots(wanted: &dyn Fn(&str) -> bool, out_dir: &Path) -> Result<(), String> {
    let frames: [(&str, BaseMode, Option<u32>); 6] = [
        ("restart-now-frame-1-20ms", BaseMode::Dark, Some(20)),
        ("restart-now-frame-2-60ms", BaseMode::Dark, Some(60)),
        ("restart-now-frame-3-120ms", BaseMode::Dark, Some(120)),
        ("restart-now-frame-4-220ms", BaseMode::Dark, Some(220)),
        ("restart-now-dark", BaseMode::Dark, None),
        ("restart-now-light", BaseMode::Light, None),
    ];
    for (name, base, ms) in frames {
        if !wanted(name) {
            continue;
        }
        let mut sc = scenario(TALL, with_base(base));
        sc.glass_switched_on = true;
        sc.restart_ms = ms;
        let app = build_app(&mock_backend(), &sc);
        let path = out_dir.join(format!("{name}.png"));
        render_png(&app, sc.size, &path).map_err(|e| format!("{name}: {e}"))?;
        let (x, y, w, h) = GLASS_CARD;
        let img = image::open(&path).map_err(|e| format!("{name}: {e}"))?;
        img.crop_imm(x, y, w, h)
            .save(&path)
            .map_err(|e| format!("{name}: {e}"))?;
        println!("wrote {}", path.display());
    }
    Ok(())
}

/// Render the Appearance shots named in `only` (all when it is empty).
pub fn render(only: &[String], out_dir: &Path) -> Result<(), String> {
    let wanted = |name: &str| only.is_empty() || only.iter().any(|n| n == name);
    restart_shots(&wanted, out_dir)?;
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
                reduce_motion: true,
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
