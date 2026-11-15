//! Mid-transition frames of the app-motion spec: a surface or page drawn part-way into its enter.

use nemo_ui::gui::headless::Scenario;
use nemo_ui::route::Route;

use super::{Shot, named, shot};

/// The menu, the signal popover and the full-window Now Playing, 20 ms into their enter (emphasized-decelerate covers most of the way early).
pub(super) fn shots(out: &mut Vec<Shot>) {
    let size = (1366, 697);
    let mid = Some(20);
    out.push(shot(
        named("motion-menu-mid", size),
        Scenario {
            size,
            menu_track: Some(2),
            mid_motion_ms: mid,
            ..Scenario::default()
        },
    ));
    out.push(shot(
        named("motion-signal-mid", size),
        Scenario {
            size,
            signal_open: true,
            mid_motion_ms: mid,
            ..Scenario::default()
        },
    ));
    out.push(shot(
        named("motion-now-playing-mid", size),
        Scenario {
            size,
            route: Route::NowPlaying,
            playing: true,
            mid_motion_ms: mid,
            ..Scenario::default()
        },
    ));
}
