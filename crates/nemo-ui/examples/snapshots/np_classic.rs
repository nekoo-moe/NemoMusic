//! Shots of the Classic Now Playing style (now-playing-styles task 5), drawn from its builders
//! directly, so they show Classic whatever the style dispatch picks.

use std::path::Path;

use nemo_ui::gui::headless::{Scenario, build_app, render_view_png};
use nemo_ui::gui::views::np_classic;
use nemo_ui::mock::mock_backend;
use nemo_ui::route::Route;
use nemo_ui::settings::{NowPlayingStyle, UiSettings};

/// Width of the right pane in the pane shot.
const PANE_W: u32 = 360;

/// Render the Classic shots named in `only` (all when empty) into `out_dir`.
pub(super) fn render(only: &[String], out_dir: &Path) -> Result<(), String> {
    let shots = [
        ("np-classic-1366x697", (1366, 697), false),
        ("np-classic-1920x1040", (1920, 1040), false),
        ("np-classic-compact-1000x640", (1000, 640), false),
        ("np-classic-pane-1920x1040", (1920, 1040), true),
    ];
    for (name, size, in_pane) in shots {
        if !only.is_empty() && !only.iter().any(|n| n == name) {
            continue;
        }
        let backend = mock_backend();
        let scenario = Scenario {
            route: if in_pane {
                Route::Home
            } else {
                Route::NowPlaying
            },
            size,
            settings: UiSettings {
                now_playing_style: NowPlayingStyle::Classic,
                ..UiSettings::default()
            },
            ..Scenario::default()
        };
        let app = build_app(&backend, &scenario);
        let path = out_dir.join(format!("{name}.png"));
        // Pane width is a small exact number.
        #[allow(clippy::cast_precision_loss)]
        let (view, frame) = if in_pane {
            (np_classic::pane(&app, PANE_W as f32), (PANE_W, size.1))
        } else {
            (np_classic::view(&app, size.0, size.1), size)
        };
        render_view_png(view, frame, &path).map_err(|e| format!("{name}: {e}"))?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
