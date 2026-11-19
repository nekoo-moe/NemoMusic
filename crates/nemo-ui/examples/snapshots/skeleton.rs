//! Shots of the skeleton-loading spec: pages waiting for their first data, dark and light, at
//! the small and the laptop window, plus one frame in the middle of the shimmer sweep.

use nemo_core::catalog::BrowseId;
use nemo_ui::UiSettings;
use nemo_ui::gui::headless::Scenario;
use nemo_ui::route::Route;
use nemo_ui::scheme::BaseMode;

use super::{Shot, named, shot};

/// The two window sizes of the spec.
const SIZES: [(u32, u32); 2] = [(960, 600), (1366, 697)];

/// A scenario for a page that waits for its data: past the 150 ms delay, motion off (still).
fn waiting(route: Route, size: (u32, u32), base: BaseMode) -> Scenario {
    Scenario {
        route,
        size,
        settings: UiSettings {
            tos_acknowledged: true,
            base,
            ..UiSettings::default()
        },
        skeleton_phase: Some(0.0),
        ..Scenario::default()
    }
}

/// Playlist page loading, dark and light, at both sizes; one mid-shimmer frame.
pub(super) fn shots(out: &mut Vec<Shot>) {
    let playlist = || Route::Playlist(BrowseId("mock-playlist-1".to_owned()));
    for size in SIZES {
        for (stem, base) in [("dark", BaseMode::Dark), ("light", BaseMode::Light)] {
            out.push(shot(
                named(&format!("skeleton-playlist-{stem}"), size),
                Scenario {
                    loading_detail: true,
                    ..waiting(playlist(), size, base)
                },
            ));
        }
    }
    let size = (1366, 697);
    out.push(shot(
        named("skeleton-playlist-shimmer", size),
        Scenario {
            loading_detail: true,
            full_motion: true,
            skeleton_phase: Some(0.3),
            ..waiting(playlist(), size, BaseMode::Dark)
        },
    ));
}
