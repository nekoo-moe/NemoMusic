//! Shots of the round 3 features: Explore, Library, Tools, shortcuts, queue, lyrics and the mini player.

use nemo_ui::explore::{ContentFilter, LibLayout, LibTab};
use nemo_ui::gui::app::{NpTab, SettingsSection};
use nemo_ui::gui::headless::Scenario;
use nemo_ui::route::Route;

use super::{SIZES, Shot, named, shot};

/// Round 3 features.
#[allow(clippy::too_many_lines)] // a flat list of scenarios
pub(super) fn features(out: &mut Vec<Shot>) {
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
}
