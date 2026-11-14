//! Shots of the main screens at both window sizes.

use nemo_core::catalog::BrowseId;
use nemo_ui::UiSettings;
use nemo_ui::gui::headless::{Scenario, Sheet};
use nemo_ui::route::Route;

use super::{SIZES, Shot, named, shot};

/// The main screens at both window sizes.
#[allow(clippy::too_many_lines)] // a flat list of scenarios
pub(super) fn screens(out: &mut Vec<Shot>) {
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
}
