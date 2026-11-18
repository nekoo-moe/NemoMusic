//! Shots of the lyrics rebuild: the scroll cascade in the middle of a line change, the word
//! sweep and the loading state, with motion on (the other lyric shots run with reduced motion).
//! The song is a made-up line-timed document (no CJK line: the sandbox that renders the shots has no CJK font). Render only these with their names as arguments.

use std::path::Path;
use std::time::Duration;

use nemo_core::lyrics::{Granularity, LyricLine, LyricsDoc, ProviderId};
use nemo_core::services::LyricsLookup;
use nemo_ui::gui::app::NpTab;
use nemo_ui::gui::headless::{Scenario, build_app, render_png};
use nemo_ui::mock::mock_backend;
use nemo_ui::route::Route;
use nemo_ui::settings::UiSettings;
use nemo_ui::state::Event;

/// Invented lines, one every 4 s from 2 s: Latin and Vietnamese.
const LINES: [&str; 12] = [
    "this line was made up",
    "v\u{e0} t\u{e2}m h\u{1ed3}n t\u{f4}i l\u{e0} m\u{1ed9}t b\u{1ea3}n nh\u{1ea1}c ch\u{1b0}a bao gi\u{1edd} \u{111}\u{1b0}\u{1ee3}c h\u{e1}t h\u{1ebf}t",
    "counting clocks that do not tick",
    "and when a line is long it wraps onto a second row and stays together",
    "oh, the fake chorus starts",
    "we are only test data",
    "\u{1ea8}N S\u{c2}U TRONG \u{110}\u{d4}I M\u{1eae}T \u{1ea4}Y",
    "and the bridge goes nowhere",
    "one more line for scrolling",
    "the lights stay low",
    "a last made-up line",
    "the end",
];

fn song() -> LyricsDoc {
    LyricsDoc {
        lines: LINES
            .iter()
            .zip(0_u64..)
            .map(|(text, i)| LyricLine {
                start_ms: Some(2_000 + 4_000 * i),
                end_ms: Some(2_000 + 4_000 * i + 3_500),
                text: (*text).to_owned(),
                words: Vec::new(),
            })
            .collect(),
        granularity: Granularity::Line,
        provider: ProviderId::new("Mock Lyrics"),
        matched: None,
        instrumental: false,
    }
}

/// `(name, window, pane open, [(position ms, frames of 16 ms after it)])`.
type Spec = (&'static str, (u32, u32), bool, &'static [(u64, u32)]);

/// A seek to 17.5 s (line 4 is active), then play on (steps under a second, so no seek) to the
/// change to line 5 and 30 frames (0.48 s) into its cascade.
const MID_CASCADE: &[(u64, u32)] = &[
    (17_500, 3),
    (18_400, 0),
    (19_300, 0),
    (20_200, 0),
    (21_100, 0),
    (21_400, 30),
];
/// Settled on line 5.
const SETTLED: &[(u64, u32)] = &[(21_000, 3), (23_000, 100)];

fn specs() -> Vec<Spec> {
    vec![
        (
            "lyrics-cascade-mid-full-1366x697",
            (1366, 697),
            false,
            MID_CASCADE,
        ),
        ("lyrics-settled-full-1366x697", (1366, 697), false, SETTLED),
        (
            "lyrics-settled-full-1920x1040",
            (1920, 1040),
            false,
            SETTLED,
        ),
        (
            "lyrics-cascade-mid-pane-1366x697",
            (1366, 697),
            true,
            MID_CASCADE,
        ),
    ]
}

/// Render the shots named in `only` (all when empty) into `out_dir`.
pub(super) fn render(only: &[String], out_dir: &Path) -> Result<(), String> {
    for (name, size, pane, steps) in specs() {
        if !only.is_empty() && !only.iter().any(|n| n == name) {
            continue;
        }
        let backend = mock_backend();
        let scenario = Scenario {
            route: if pane { Route::Home } else { Route::NowPlaying },
            size,
            np_tab: NpTab::Lyrics,
            pane_open: Some(pane),
            full_motion: true,
            settings: UiSettings {
                tos_acknowledged: true,
                ..UiSettings::default()
            },
            ..Scenario::default()
        };
        let mut app = build_app(&backend, &scenario);
        let track = backend
            .player
            .snapshot()
            .current
            .ok_or_else(|| format!("{name}: nothing playing"))?
            .id;
        app.dispatch(Event::LyricsReady {
            track,
            lookup: LyricsLookup {
                doc: Some(song()),
                tried: Vec::new(),
            },
            offset_ms: 0,
        });
        for (at_ms, frames) in steps {
            app.dispatch(Event::Position(Duration::from_millis(*at_ms)));
            // The song plays on while the frames pass.
            for k in 0..*frames {
                let t = at_ms + 16 * u64::from(k);
                app.dispatch(Event::Position(Duration::from_millis(t)));
                app.dispatch(Event::Frame { dt_ms: 16.0 });
            }
        }
        let path = out_dir.join(format!("{name}.png"));
        render_png(&app, size, &path).map_err(|e| format!("{name}: {e}"))?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
