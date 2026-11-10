//! Renders the reference screens headless to `docs/ui/snapshots/*.png` (tiny-skia, no GPU).
//!
//! ```text
//! $env:ICED_TEST_BACKEND = 'tiny-skia'
//! cargo run -p nemo-ui --example snapshots --features gui
//! ```
//!
//! Images are 2x scale (2560 x 1600 for a 1280 x 800 window).

use std::path::PathBuf;

use nemo_ui::gui::headless::{Scenario, build_app, render_png};
use nemo_ui::mock::mock_backend;
use nemo_ui::route::Route;

fn main() {
    if std::env::var("ICED_TEST_BACKEND").as_deref() != Ok("tiny-skia") {
        eprintln!("set ICED_TEST_BACKEND=tiny-skia first (headless rendering without a GPU)");
        std::process::exit(2);
    }
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/ui/snapshots");
    let scenarios = [
        ("home-1280x800", Route::Home, (1280, 800), 1),
        ("home-960x600", Route::Home, (960, 600), 1),
        ("now-playing-1280x800", Route::NowPlaying, (1280, 800), 1),
        (
            "now-playing-compact-960x600",
            Route::NowPlaying,
            (960, 600),
            1,
        ),
    ];
    for (name, route, size, track) in scenarios {
        let backend = mock_backend();
        let scenario = Scenario {
            route,
            size,
            position_secs: 83,
            track,
            playing: false,
        };
        let app = build_app(&backend, &scenario);
        let path = out_dir.join(format!("{name}.png"));
        match render_png(&app, size, &path) {
            Ok(()) => println!("wrote {}", path.display()),
            Err(e) => {
                eprintln!("{name}: {e}");
                std::process::exit(1);
            }
        }
    }
}
