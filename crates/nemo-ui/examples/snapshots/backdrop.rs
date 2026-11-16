//! Shots of the round 4 features: coloured background, daily Home and skeletons.

use nemo_core::catalog::BrowseId;
use nemo_ui::explore::ContentFilter;
use nemo_ui::gui::headless::Scenario;
use nemo_ui::route::Route;

use super::{SIZES, Shot, named, shot};

/// Coloured background, daily Home, skeletons.
#[allow(clippy::too_many_lines)] // a flat list of scenarios
pub(super) fn backdrops(out: &mut Vec<Shot>) {
    // Sort and header facts (collection-sort-meta spec): the menu open, a sorted page, an album
    // with the artist picture, and a playlist whose header gives nothing (all hidden).
    let mock_playlist = || Route::Playlist(BrowseId("mock-playlist-0".to_owned()));
    for size in SIZES {
        out.push(shot(
            named("playlist-sort-menu", size),
            Scenario {
                route: mock_playlist(),
                size,
                sort_menu: true,
                ..Scenario::default()
            },
        ));
    }
    let mut sorted = Scenario::default().settings;
    sorted.collection_sorts.insert(
        "playlist:mock-playlist-0".to_owned(),
        nemo_ui::collection_sort::CollectionSort::Title,
    );
    out.push(shot(
        named("playlist-sorted-title", (1366, 697)),
        Scenario {
            route: mock_playlist(),
            settings: sorted,
            ..Scenario::default()
        },
    ));
    out.push(shot(
        named("playlist-no-meta", (1366, 697)),
        Scenario {
            route: Route::Playlist(BrowseId("mock-playlist-1".to_owned())),
            ..Scenario::default()
        },
    ));
    // The collection page stacks below a 1280 px window.
    for size in [(1100, 700), (960, 600)] {
        out.push(shot(
            named("playlist-ytm-stacked", size),
            Scenario {
                route: Route::Playlist(BrowseId("mock-playlist-1".to_owned())),
                size,
                ..Scenario::default()
            },
        ));
    }
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
        // The YouTube Music style collection page (collection-page-ytm spec) in light mode.
        out.push(shot(
            named("album-ytm-light", size),
            Scenario {
                settings: nemo_ui::UiSettings {
                    base: nemo_ui::scheme::BaseMode::Light,
                    ..Scenario::default().settings
                },
                ..at(Route::Album(BrowseId("mock-album-4".to_owned())))
            },
        ));
        // Reduce transparency: one flat tonal colour from the cover, no texture.
        out.push(shot(
            named("bg-flat", size),
            Scenario {
                track: 1,
                settings: nemo_ui::UiSettings {
                    reduce_transparency: true,
                    ..Scenario::default().settings
                },
                ..at(Route::Home)
            },
        ));
        // The window follows the song on an open page too (living-backdrop spec).
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
