//! Renders the reference screens headless to `docs/ui/snapshots/*.png` (tiny-skia, no GPU).
//!
//! ```text
//! $env:ICED_TEST_BACKEND = 'tiny-skia'
//! cargo run -p nemo-ui --example snapshots --features gui
//! ```
//!
//! Images are 2x scale (2732 x 1394 for a 1366 x 697 window). Pass names as arguments to render
//! only those (for example `home-1366x697`).

use std::path::PathBuf;

use nemo_core::catalog::BrowseId;
use nemo_core::player::RepeatMode;
use nemo_core::services::Backend;
use nemo_ui::UiSettings;
use nemo_ui::explore::{ContentFilter, LibLayout, LibTab};
use nemo_ui::gui::app::{HomeFilter, NpTab, SettingsSection};
use nemo_ui::gui::headless::{Scenario, Sheet, build_app, render_png, render_view_png, specimen};
use nemo_ui::mock::{mock_backend, mock_backend_seeded, mock_backend_signed_in};
use nemo_ui::route::Route;
use nemo_ui::scheme::{BaseMode, ThemeChoice};

/// The two window sizes the owner cares about: the maximized 1366 x 768 laptop and full HD.
const SIZES: [(u32, u32); 2] = [(1366, 697), (1920, 1040)];

/// One image to render.
struct Shot {
    name: String,
    seeded: bool,
    signed_in: bool,
    scenario: Scenario,
}

fn shot(name: String, scenario: Scenario) -> Shot {
    Shot {
        name,
        seeded: false,
        signed_in: false,
        scenario,
    }
}

fn named(stem: &str, size: (u32, u32)) -> String {
    format!("{stem}-{}x{}", size.0, size.1)
}

#[allow(clippy::too_many_lines)] // a flat list of scenarios
fn scenarios() -> Vec<Shot> {
    let mut out = Vec::new();
    for size in SIZES {
        let at = |route: Route| Scenario {
            route,
            size,
            ..Scenario::default()
        };
        out.push(shot(named("home", size), at(Route::Home)));
        out.push(shot(
            named("album", size),
            at(Route::Album(BrowseId("mock-album-2".to_owned()))),
        ));
        out.push(shot(
            named("search", size),
            Scenario {
                search: Some("velvet".to_owned()),
                ..at(Route::Search(String::new()))
            },
        ));
        out.push(shot(named("now-playing", size), at(Route::NowPlaying)));
        out.push(shot(
            named("library", size),
            Scenario {
                music_folders: vec!["D:\\Music".to_owned(), "E:\\Rips".to_owned()],
                ..at(Route::Library)
            },
        ));
        out.push(shot(
            named("settings", size),
            Scenario {
                settings: UiSettings {
                    sponsorblock: true,
                    tos_acknowledged: true,
                    volume: 80,
                    ..UiSettings::default()
                },
                ..at(Route::Settings)
            },
        ));
        out.push(shot(
            named("signin", size),
            Scenario {
                sheet: Sheet::SignIn,
                ..at(Route::Home)
            },
        ));
    }
    let size = SIZES[0];
    let at = |route: Route| Scenario {
        route,
        size,
        ..Scenario::default()
    };
    out.push(shot(
        named("signin-cookie", size),
        Scenario {
            sheet: Sheet::Cookie,
            ..at(Route::Home)
        },
    ));
    out.push(shot(
        named("signin-error", size),
        Scenario {
            sheet: Sheet::Error,
            ..at(Route::Home)
        },
    ));
    out.push(Shot {
        signed_in: true,
        ..shot(
            named("signed-in", size),
            Scenario {
                sheet: Sheet::SignedIn,
                ..at(Route::Home)
            },
        )
    });
    out.push(Shot {
        signed_in: true,
        ..shot(
            named("signed-in-confirm", size),
            Scenario {
                sheet: Sheet::ConfirmSignOut,
                ..at(Route::Home)
            },
        )
    });
    out.push(Shot {
        signed_in: true,
        ..shot(named("home-signed-in", size), at(Route::Home))
    });
    out.push(Shot {
        seeded: true,
        ..shot(named("home-seeded", size), at(Route::Home))
    });
    out.push(shot(
        named("home-albums", size),
        Scenario {
            home_filter: HomeFilter::Albums,
            ..at(Route::Home)
        },
    ));
    out.push(shot(
        named("queue-tab", size),
        Scenario {
            np_tab: NpTab::Queue,
            ..at(Route::Home)
        },
    ));
    out.push(shot(
        named("info-tab", size),
        Scenario {
            np_tab: NpTab::Info,
            ..at(Route::Home)
        },
    ));
    out.push(shot(
        named("pane-closed", size),
        Scenario {
            pane_open: Some(false),
            ..at(Route::Home)
        },
    ));
    out.push(shot(
        named("artist", size),
        at(Route::Artist(BrowseId("mock-artist-1".to_owned()))),
    ));
    out.push(shot(
        named("playlist", size),
        at(Route::Playlist(BrowseId("mock-playlist-0".to_owned()))),
    ));
    out.push(shot(
        named("settings-audio", size),
        Scenario {
            section: SettingsSection::AudioOutput,
            ..at(Route::Settings)
        },
    ));
    out.push(shot(
        named("settings-privacy", size),
        Scenario {
            section: SettingsSection::Privacy,
            ..at(Route::Settings)
        },
    ));
    out.push(shot(
        named("tos", size),
        Scenario {
            settings: UiSettings::default(),
            ..at(Route::Home)
        },
    ));
    out.push(shot(
        named("buffering", size),
        Scenario {
            buffering: true,
            playing: true,
            ..at(Route::Home)
        },
    ));
    out.push(shot(
        named("signal-path", size),
        Scenario {
            signal_open: true,
            ..at(Route::Home)
        },
    ));
    out.push(shot(
        named("toast", size),
        Scenario {
            toast: Some("This song could not be played. Skipping to the next one.".to_owned()),
            ..at(Route::Home)
        },
    ));
    out.push(shot(
        named("menu", size),
        Scenario {
            menu_track: Some(1),
            ..at(Route::Home)
        },
    ));
    out.push(shot(
        named("local-flac", size),
        Scenario {
            track: 3,
            signal_open: true,
            ..at(Route::Home)
        },
    ));
    out.push(shot(
        named("no-lyrics", size),
        Scenario {
            track: 5,
            ..at(Route::Home)
        },
    ));
    out.push(shot(
        named("pane-empty", size),
        Scenario {
            stopped: Some(false),
            ..at(Route::Home)
        },
    ));
    out.push(shot(
        named("pane-recent", size),
        Scenario {
            stopped: Some(true),
            ..at(Route::Home)
        },
    ));
    out.push(shot(
        named("repeat-all", size),
        Scenario {
            repeat: RepeatMode::All,
            ..at(Route::Home)
        },
    ));
    out.push(shot(
        named("repeat-one", size),
        Scenario {
            repeat: RepeatMode::One,
            ..at(Route::Home)
        },
    ));
    out.push(Shot {
        signed_in: true,
        ..shot(
            named("signed-in-liked", size),
            Scenario {
                route: Route::Album(BrowseId("mock-album-2".to_owned())),
                size,
                ..Scenario::default()
            },
        )
    });
    {
        let size = SIZES[1];
        out.push(shot(
            named("pane-empty", size),
            Scenario {
                stopped: Some(false),
                size,
                ..Scenario::default()
            },
        ));
        out.push(shot(
            named("no-lyrics", size),
            Scenario {
                track: 5,
                size,
                ..Scenario::default()
            },
        ));
    }
    for compact in [(960, 600), (1100, 700)] {
        out.push(shot(
            named("home", compact),
            Scenario {
                size: compact,
                ..Scenario::default()
            },
        ));
        out.push(shot(
            named("now-playing", compact),
            Scenario {
                size: compact,
                route: Route::NowPlaying,
                ..Scenario::default()
            },
        ));
    }
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
    // Round 3 features.
    for size in SIZES {
        let at = |route: Route| Scenario {
            route,
            size,
            ..Scenario::default()
        };
        let signed = |name: String, sc: Scenario| Shot {
            signed_in: true,
            ..shot(name, sc)
        };
        out.push(shot(named("explore", size), at(Route::Explore)));
        out.push(signed(named("library-signed-in", size), at(Route::Library)));
        out.push(shot(named("library-signed-out", size), at(Route::Library)));
        out.push(signed(
            named("library-grid", size),
            Scenario {
                lib_layout: LibLayout::Grid,
                ..at(Route::Library)
            },
        ));
        out.push(signed(
            named("library-playlists", size),
            Scenario {
                lib_tab: LibTab::Playlists,
                ..at(Route::Library)
            },
        ));
        out.push(signed(
            named("library-songs", size),
            Scenario {
                lib_tab: LibTab::Songs,
                ..at(Route::Library)
            },
        ));
        out.push(shot(
            named("library-local", size),
            Scenario {
                lib_tab: LibTab::Local,
                music_folders: vec!["D:\\Music".to_owned()],
                ..at(Route::Library)
            },
        ));
        out.push(signed(
            named("library-create", size),
            Scenario {
                create: true,
                ..at(Route::Library)
            },
        ));
        out.push(Shot {
            seeded: true,
            ..shot(named("home-quick-access", size), at(Route::Home))
        });
        out.push(shot(
            named("home-videos", size),
            Scenario {
                content_filter: ContentFilter::Videos,
                ..at(Route::Home)
            },
        ));
        out.push(shot(
            named("tools", size),
            Scenario {
                tools: true,
                sleep: true,
                ..at(Route::Home)
            },
        ));
        out.push(shot(
            named("shortcuts", size),
            Scenario {
                shortcuts: true,
                ..at(Route::Home)
            },
        ));
        out.push(shot(
            named("menu-extended", size),
            Scenario {
                menu_track: Some(1),
                ..at(Route::Home)
            },
        ));
        out.push(shot(
            named("queue-move", size),
            Scenario {
                np_tab: NpTab::Queue,
                ..at(Route::Home)
            },
        ));
        out.push(shot(
            named("lyrics-switcher", size),
            Scenario {
                np_tab: NpTab::Lyrics,
                ..at(Route::Home)
            },
        ));
        out.push(shot(
            named("lyrics-unison", size),
            Scenario {
                np_tab: NpTab::Lyrics,
                lyrics_pick: Some(1),
                ..at(Route::Home)
            },
        ));
        out.push(shot(
            named("settings-lyrics", size),
            Scenario {
                section: SettingsSection::Lyrics,
                ..at(Route::Settings)
            },
        ));
    }
    out.push(shot(
        "mini-420x168".to_owned(),
        Scenario {
            mini: true,
            size: (420, 168),
            ..Scenario::default()
        },
    ));
    // Round 4: coloured background, daily Home, skeletons.
    for size in SIZES {
        let at = |route: Route| Scenario {
            route,
            size,
            ..Scenario::default()
        };
        let signed = |name: String, sc: Scenario| Shot {
            signed_in: true,
            ..shot(name, sc)
        };
        out.push(signed(named("home-daily-signed-in", size), at(Route::Home)));
        out.push(shot(named("home-daily-signed-out", size), at(Route::Home)));
        // The same screen with two different songs: the whole window changes colour.
        out.push(shot(
            named("bg-song-a", size),
            Scenario {
                track: 1,
                ..at(Route::Home)
            },
        ));
        out.push(shot(
            named("bg-song-b", size),
            Scenario {
                track: 9,
                ..at(Route::Home)
            },
        ));
        out.push(shot(
            named("bg-song-c", size),
            Scenario {
                track: 14,
                ..at(Route::Home)
            },
        ));
        // A page colour wins while the page is open.
        out.push(shot(
            named("bg-page-album", size),
            at(Route::Album(BrowseId("mock-album-4".to_owned()))),
        ));
        out.push(shot(
            named("album-loading", size),
            Scenario {
                loading_detail: true,
                ..at(Route::Album(BrowseId("mock-album-2".to_owned())))
            },
        ));
        out.push(shot(
            named("playlist-loading", size),
            Scenario {
                loading_detail: true,
                ..at(Route::Playlist(BrowseId("mock-playlist-1".to_owned())))
            },
        ));
        out.push(shot(
            named("home-videos-feed", size),
            Scenario {
                content_filter: ContentFilter::Videos,
                ..at(Route::Home)
            },
        ));
    }
    out
}

fn main() {
    if std::env::var("ICED_TEST_BACKEND").as_deref() != Ok("tiny-skia") {
        eprintln!("set ICED_TEST_BACKEND=tiny-skia first (headless rendering without a GPU)");
        std::process::exit(2);
    }
    let only: Vec<String> = std::env::args().skip(1).collect();
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/ui/snapshots");
    if only.is_empty() || only.iter().any(|n| n == "specimen-vietnamese") {
        let path = out_dir.join("specimen-vietnamese.png");
        match render_view_png(specimen(), (1100, 420), &path) {
            Ok(()) => println!("wrote {}", path.display()),
            Err(e) => {
                eprintln!("specimen: {e}");
                std::process::exit(1);
            }
        }
    }
    for s in scenarios() {
        if !only.is_empty() && !only.contains(&s.name) {
            continue;
        }
        let backend: Backend = match (s.seeded, s.signed_in) {
            (true, _) => mock_backend_seeded(),
            (false, true) => mock_backend_signed_in(),
            (false, false) => mock_backend(),
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
