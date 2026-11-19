//! Shots of the floating player capsule (player-bar-redesign, bar-material). Each bar shot renders the whole
//! window and keeps the bottom strip, so the capsule shows over the real window colour; two
//! full-window shots show how the panes end above it.

use std::path::Path;

use nemo_ui::UiSettings;
use nemo_ui::gui::headless::{Scenario, build_app, render_png, render_png_pointer};
use nemo_ui::layout::bar::{PADDING, ZONE_GAP, width, zones};
use nemo_ui::mock::mock_backend;
use nemo_ui::scheme::BaseMode;

/// Logical px kept from the bottom of the window: the capsule, its margin and the shadow.
const STRIP: u32 = 124;
/// `iced_test` draws at this scale.
const SCALE: u32 = 2;

/// One shot: name, window size, scenario, whether to keep only the bottom strip, and where the
/// pointer is: a fraction of the seek track, and whether the button is held.
struct Shot {
    name: &'static str,
    scenario: Scenario,
    strip: bool,
    pointer: Option<(f32, bool)>,
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

/// A strip shot: the bottom of the window only.
fn strip(name: &'static str, scenario: Scenario) -> Shot {
    Shot {
        name,
        scenario,
        strip: true,
        pointer: None,
    }
}

/// A strip shot with the pointer on the seek track.
fn pointed(name: &'static str, scenario: Scenario, frac: f32, pressed: bool) -> Shot {
    Shot {
        pointer: Some((frac, pressed)),
        ..strip(name, scenario)
    }
}

/// A whole-window shot.
fn window(name: &'static str, scenario: Scenario) -> Shot {
    Shot {
        strip: false,
        ..strip(name, scenario)
    }
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
    let mut out = vec![
        strip("player-bar-playing-1280", scenario(wide, 0)),
        strip("player-bar-paused-1280", paused),
        strip("player-bar-buffering-1280", buffering),
        strip("player-bar-repeat-one-1280", repeat_one),
        strip("player-bar-1100", scenario((1100, 700), 0)),
        strip("player-bar-960", scenario((960, 600), 0)),
        strip(
            "player-bar-amoled-1280",
            with_base(scenario(wide, 0), BaseMode::Amoled),
        ),
        strip(
            "player-bar-light-1280",
            with_base(scenario(wide, 0), BaseMode::Light),
        ),
        strip("player-bar-solid-1280", off),
        strip("player-bar-cover-b-1280", scenario(wide, 1)),
        strip(
            "player-bar-cover-c-light-1280",
            with_base(scenario(wide, 2), BaseMode::Light),
        ),
        pointed("player-bar-seek-hover-1280", scenario(wide, 0), 0.35, false),
        pointed("player-bar-seek-drag-1280", scenario(wide, 0), 0.553, true),
        window("player-bar-window-1280x800", scenario(wide, 0)),
        window("player-bar-window-960x600", scenario((960, 600), 0)),
    ];
    out.extend(material_shots());
    out
}

/// The Material capsule over four backdrops (bar-material spec): a dark red cover (album 8), a
/// yellow-green one (album 6), the red one in light mode and in AMOLED, at 1280 and 960 px wide,
/// and the red one with Reduce transparency (fully opaque).
fn material_shots() -> Vec<Shot> {
    let mut out = Vec::new();
    for (stem, track, base) in [
        ("red", 32, BaseMode::Dark),
        ("yellow", 24, BaseMode::Dark),
        ("light", 32, BaseMode::Light),
        ("amoled", 32, BaseMode::Amoled),
    ] {
        for size in [(1280, 800), (960, 600)] {
            // The queue holds tracks 0 to 11: switch to the cover wanted, the way a song change
            // does, and draw after the bloom.
            let mut s = with_base(scenario(size, 0), base);
            s.bloom = Some((track, 700));
            // A handful of names for the run of one example: leaking them is harmless.
            let name: &'static str =
                Box::leak(format!("bar-material-{stem}-{}", size.0).into_boxed_str());
            out.push(strip(name, s));
        }
    }
    let mut solid = with_base(scenario((1280, 800), 0), BaseMode::Dark);
    solid.bloom = Some((32, 700));
    solid.settings.reduce_transparency = true;
    out.push(strip("bar-material-red-solid-1280", solid));
    out
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
        match s.pointer {
            Some((frac, pressed)) => {
                let at = seek_point(s.scenario.size, frac);
                render_png_pointer(&app, s.scenario.size, &path, at, pressed)
            }
            None => render_png(&app, s.scenario.size, &path),
        }
        .map_err(|e| format!("{}: {e}", s.name))?;
        if s.strip {
            crop_bottom(&path, STRIP * SCALE).map_err(|e| format!("{}: {e}", s.name))?;
        }
        println!("wrote {}", path.display());
    }
    Ok(())
}

/// Where the pointer is for `frac` of the seek track: logical px in a window of `size`.
fn seek_point(size: (u32, u32), frac: f32) -> (f32, f32) {
    // Layout numbers are small, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let px = |n: u32| n as f32;
    let bar = width(size.0);
    let z = zones(bar);
    let centre_x = px((size.0 - bar) / 2 + PADDING + z.left + ZONE_GAP);
    // 40 px time label and an 8 px gap on each side, then the thumb radius.
    let start = centre_x + 48.0 + 7.0;
    let end = centre_x + px(z.centre) - 48.0 - 7.0;
    // Window bottom, the 12 px margin, the 88 px capsule (6 px of air above the 76 px column),
    // and the seek row at the bottom of the column.
    (
        start + frac * (end - start),
        px(size.1) - 12.0 - 88.0 + 6.0 + 56.0 + 4.0 + 8.0,
    )
}

/// Keep only the bottom `rows` pixel rows of the PNG at `path`.
fn crop_bottom(path: &Path, rows: u32) -> Result<(), String> {
    let img = image::open(path).map_err(|e| e.to_string())?;
    let rows = rows.min(img.height());
    let cut = img.crop_imm(0, img.height() - rows, img.width(), rows);
    cut.save(path).map_err(|e| e.to_string())
}
