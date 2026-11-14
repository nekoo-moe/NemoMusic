//! Shots of the round 4 features: coloured background, daily Home and skeletons.

use nemo_core::catalog::BrowseId;
use nemo_ui::explore::ContentFilter;
use nemo_ui::gui::headless::Scenario;
use nemo_ui::route::Route;

use super::{SIZES, Shot, named, shot};

/// Coloured background, daily Home, skeletons.
#[allow(clippy::too_many_lines)] // a flat list of scenarios
pub(super) fn backdrops(out: &mut Vec<Shot>) {
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
}
