//! Shots of the lyrics-polish wave (`.kiro/specs/lyrics-polish/`): the intro and gap note in
//! the right pane, the full-screen lyrics, the compact layout and the Classic style. The song
//! is a made-up line-timed document with an 8 s intro and an 11 s instrumental break, swapped
//! in for the mock track's lyrics. Render only these with `lyrics-` names as arguments.

use std::path::Path;
use std::time::Duration;

use nemo_core::lyrics::{Granularity, LyricLine, LyricsDoc, ProviderId};
use nemo_core::services::LyricsLookup;
use nemo_ui::gui::app::NpTab;
use nemo_ui::gui::headless::{Scenario, build_app, render_png};
use nemo_ui::mock::mock_backend;
use nemo_ui::route::Route;
use nemo_ui::settings::{NowPlayingStyle, UiSettings};
use nemo_ui::state::Event;

/// Invented lines `(start s, text)`, each sung for 3 s; the break runs from 0:19 to 0:30.
const LINES: [(u64, &str); 9] = [
    (8, "this line was made up"),
    (
        12,
        "v\u{e0} t\u{e2}m h\u{1ed3}n t\u{f4}i l\u{e0} m\u{1ed9}t b\u{1ea3}n nh\u{1ea1}c",
    ),
    (16, "counting clocks that do not tick"),
    (30, "oh, the fake chorus starts"),
    (34, "we are only test data"),
    (
        38,
        "\u{1ea8}N S\u{c2}U TRONG \u{110}\u{d4}I M\u{1eae}T \u{1ea4}Y",
    ),
    (
        42,
        "ng\u{1b0}\u{1edd}i \u{1ea5}y \u{111}\u{e3} r\u{1edd}i xa, \u{111}\u{1ec3} l\u{1ea1}i nh\u{1eef}ng ng\u{e0}y d\u{e0}i",
    ),
    (46, "and the bridge goes nowhere"),
    (50, "one more line for scrolling"),
];

fn song() -> LyricsDoc {
    LyricsDoc {
        lines: LINES
            .iter()
            .map(|(s, text)| LyricLine {
                start_ms: Some(s * 1000),
                end_ms: Some(s * 1000 + 3000),
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

/// `(name, window, route, style, pane open, position ms)`.
type Spec = (&'static str, (u32, u32), Route, NowPlayingStyle, bool, u64);

fn specs() -> Vec<Spec> {
    let m3 = NowPlayingStyle::M3Expressive;
    vec![
        (
            "lyrics-intro-pane-1366x697",
            (1366, 697),
            Route::Home,
            m3,
            true,
            5_000,
        ),
        (
            "lyrics-gap-pane-1366x697",
            (1366, 697),
            Route::Home,
            m3,
            true,
            24_000,
        ),
        (
            "lyrics-intro-full-1366x697",
            (1366, 697),
            Route::NowPlaying,
            m3,
            false,
            5_000,
        ),
        (
            "lyrics-gap-full-1366x697",
            (1366, 697),
            Route::NowPlaying,
            m3,
            false,
            24_000,
        ),
        (
            "lyrics-gap-full-1920x1040",
            (1920, 1040),
            Route::NowPlaying,
            m3,
            false,
            24_000,
        ),
        (
            "lyrics-after-gap-full-1366x697",
            (1366, 697),
            Route::NowPlaying,
            m3,
            false,
            31_000,
        ),
        (
            "lyrics-after-gap-full-1920x1040",
            (1920, 1040),
            Route::NowPlaying,
            m3,
            false,
            39_000,
        ),
        (
            "lyrics-timing-full-1366x697",
            (1366, 697),
            Route::NowPlaying,
            m3,
            false,
            39_000,
        ),
        (
            "lyrics-gap-compact-1000x640",
            (1000, 640),
            Route::NowPlaying,
            m3,
            false,
            24_000,
        ),
        (
            "lyrics-gap-classic-1366x697",
            (1366, 697),
            Route::NowPlaying,
            NowPlayingStyle::Classic,
            false,
            24_000,
        ),
    ]
}

/// Render the shots named in `only` (all when empty) into `out_dir`.
pub(super) fn render(only: &[String], out_dir: &Path) -> Result<(), String> {
    for (name, size, route, style, pane, at_ms) in specs() {
        if !only.is_empty() && !only.iter().any(|n| n == name) {
            continue;
        }
        let backend = mock_backend();
        let scenario = Scenario {
            route,
            size,
            np_tab: NpTab::Lyrics,
            pane_open: Some(pane),
            settings: UiSettings {
                tos_acknowledged: true,
                now_playing_style: style,
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
        app.dispatch(Event::Position(Duration::from_millis(at_ms)));
        app.dispatch(Event::Frame { dt_ms: 16.0 });
        if name.starts_with("lyrics-timing") {
            app.dispatch(Event::ToggleLyricsTiming);
        }
        let path = out_dir.join(format!("{name}.png"));
        render_png(&app, size, &path).map_err(|e| format!("{name}: {e}"))?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
