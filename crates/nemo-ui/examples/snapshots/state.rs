//! Shots of states of the app: sign-in sheets, filters, panes, toasts, menus and compact windows.

use nemo_core::catalog::BrowseId;
use nemo_core::player::RepeatMode;
use nemo_ui::UiSettings;
use nemo_ui::gui::app::{HomeFilter, NpTab, SettingsSection};
use nemo_ui::gui::headless::{Scenario, Sheet};
use nemo_ui::route::Route;

use super::{SIZES, Shot, named, shot};

/// Sheets, filters, panes, toasts, menus and compact windows.
#[allow(clippy::too_many_lines)] // a flat list of scenarios
pub(super) fn states(out: &mut Vec<Shot>) {
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
}
