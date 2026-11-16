//! Shots of the grey-to-bright syllable fill (`.kiro/specs/syllable-motion`): the right pane and
//! the full-screen lyrics in the middle of a sung word of a syllable-timed song.

use nemo_ui::gui::app::NpTab;
use nemo_ui::gui::headless::Scenario;
use nemo_ui::route::Route;

use super::{Shot, named, shot};

/// Track 1 is syllable-timed; its third line is sung from 16 s.
const TRACK: usize = 1;

/// The syllable shots for the 1366 x 697 window.
pub fn shots() -> Vec<Shot> {
    let size = (1366, 697);
    let at = |route: Route| Scenario {
        route,
        size,
        track: TRACK,
        position_secs: 18,
        full_motion: true,
        position_extra_ms: std::env::var("NEMO_SYLLABLE_EXTRA_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(105),
        ..Scenario::default()
    };
    vec![
        shot(
            named("syllable-mid-word-pane", size),
            Scenario {
                np_tab: NpTab::Lyrics,
                pane_open: Some(true),
                ..at(Route::Home)
            },
        ),
        shot(named("syllable-mid-word-full", size), at(Route::NowPlaying)),
    ]
}
