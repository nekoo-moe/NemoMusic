//! Renders the reference screens headless to `docs/ui/snapshots/*.png` (tiny-skia, no GPU).
//!
//! ```text
//! $env:ICED_TEST_BACKEND = 'tiny-skia'
//! cargo run -p nemo-ui --example snapshots --features gui
//! ```
//!
//! Images are 2x scale (2560 x 1600 for a 1280 x 800 window). Pass names as arguments to render
//! only those (for example `search-results-1280x800`).

use std::path::PathBuf;

use nemo_core::catalog::BrowseId;
use nemo_core::services::Backend;
use nemo_ui::UiSettings;
use nemo_ui::gui::app::SettingsSection;
use nemo_ui::gui::headless::{Scenario, build_app, render_png};
use nemo_ui::mock::{mock_backend, mock_backend_seeded};
use nemo_ui::route::Route;

/// One image to render.
struct Shot {
    name: &'static str,
    seeded: bool,
    scenario: Scenario,
}

fn shot(name: &'static str, scenario: Scenario) -> Shot {
    Shot {
        name,
        seeded: false,
        scenario,
    }
}

#[allow(clippy::too_many_lines)] // a flat list of scenarios
fn scenarios() -> Vec<Shot> {
    let base = Scenario::default();
    let at = |route: Route| Scenario {
        route,
        ..Scenario::default()
    };
    vec![
        shot("home-1280x800", at(Route::Home)),
        Shot {
            name: "home-seeded-1280x800",
            seeded: true,
            scenario: at(Route::Home),
        },
        shot(
            "home-960x600",
            Scenario {
                size: (960, 600),
                ..at(Route::Home)
            },
        ),
        shot(
            "search-results-1280x800",
            Scenario {
                search: Some("velvet".to_owned()),
                ..at(Route::Search(String::new()))
            },
        ),
        shot(
            "album-1280x800",
            at(Route::Album(BrowseId("mock-album-2".to_owned()))),
        ),
        shot(
            "artist-1280x800",
            at(Route::Artist(BrowseId("mock-artist-1".to_owned()))),
        ),
        shot(
            "playlist-1280x800",
            at(Route::Playlist(BrowseId("mock-playlist-0".to_owned()))),
        ),
        shot(
            "queue-open-1280x800",
            Scenario {
                queue_open: true,
                ..at(Route::Home)
            },
        ),
        shot(
            "library-1280x800",
            Scenario {
                music_folders: vec!["D:\\Music".to_owned(), "E:\\Rips".to_owned()],
                ..at(Route::Library)
            },
        ),
        shot(
            "settings-1280x800",
            Scenario {
                settings: UiSettings {
                    sponsorblock: true,
                    tos_acknowledged: true,
                    volume: 80,
                    ..UiSettings::default()
                },
                ..at(Route::Settings)
            },
        ),
        shot(
            "settings-audio-1280x800",
            Scenario {
                section: SettingsSection::AudioOutput,
                ..at(Route::Settings)
            },
        ),
        shot(
            "settings-privacy-1280x800",
            Scenario {
                section: SettingsSection::Privacy,
                ..at(Route::Settings)
            },
        ),
        shot(
            "tos-1280x800",
            Scenario {
                settings: UiSettings::default(),
                ..at(Route::Home)
            },
        ),
        shot(
            "buffering-1280x800",
            Scenario {
                buffering: true,
                playing: true,
                ..at(Route::Home)
            },
        ),
        shot(
            "signal-path-1280x800",
            Scenario {
                signal_open: true,
                ..at(Route::Home)
            },
        ),
        shot(
            "toast-1280x800",
            Scenario {
                toast: Some("This song could not be played. Skipping to the next one.".to_owned()),
                ..at(Route::Home)
            },
        ),
        shot(
            "menu-1280x800",
            Scenario {
                menu_track: Some(1),
                ..at(Route::Home)
            },
        ),
        shot("now-playing-1280x800", at(Route::NowPlaying)),
        shot(
            "now-playing-compact-960x600",
            Scenario {
                size: (960, 600),
                ..at(Route::NowPlaying)
            },
        ),
        shot(
            "local-flac-1280x800",
            Scenario {
                track: 3,
                signal_open: true,
                ..base
            },
        ),
    ]
}

fn main() {
    if std::env::var("ICED_TEST_BACKEND").as_deref() != Ok("tiny-skia") {
        eprintln!("set ICED_TEST_BACKEND=tiny-skia first (headless rendering without a GPU)");
        std::process::exit(2);
    }
    let only: Vec<String> = std::env::args().skip(1).collect();
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/ui/snapshots");
    for s in scenarios() {
        if !only.is_empty() && !only.iter().any(|n| n == s.name) {
            continue;
        }
        let backend: Backend = if s.seeded {
            mock_backend_seeded()
        } else {
            mock_backend()
        };
        let app = build_app(&backend, &s.scenario);
        let path = out_dir.join(format!("{}.png", s.name));
        match render_png(&app, s.scenario.size, &path) {
            Ok(()) => println!("wrote {}", path.display()),
            Err(e) => {
                eprintln!("{}: {e}", s.name);
                std::process::exit(1);
            }
        }
    }
}
