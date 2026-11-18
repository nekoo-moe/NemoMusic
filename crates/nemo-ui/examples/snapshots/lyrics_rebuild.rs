//! Shots of the lyrics rebuild: the scroll cascade in the middle of a line change, the word
//! sweep and the loading state, with motion on (the other lyric shots run with reduced motion).
//! The song is a made-up line-timed document (no CJK line: the sandbox that renders the shots has no CJK font). Render only these with their names as arguments.

use std::path::Path;
use std::time::Duration;

use nemo_core::lyrics::{Granularity, LyricLine, LyricWord, LyricsDoc, ProviderId};
use nemo_core::services::LyricsLookup;
use nemo_ui::gui::app::NpTab;
use nemo_ui::gui::headless::{Scenario, build_app, render_png};
use nemo_ui::mock::mock_backend;
use nemo_ui::route::Route;
use nemo_ui::scheme::BaseMode;
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

/// A line of timed pieces `(text, start ms, end ms)`.
fn timed(pieces: &[(&str, u64, u64)]) -> LyricLine {
    let text: String = pieces.iter().map(|p| p.0).collect();
    LyricLine {
        start_ms: pieces.first().map(|p| p.1),
        end_ms: pieces.last().map(|p| p.2),
        text,
        words: pieces
            .iter()
            .map(|(t, s, e)| LyricWord {
                start_ms: *s,
                end_ms: Some(*e),
                text: (*t).to_owned(),
            })
            .collect(),
    }
}

/// A syllable-timed song: one line with a held word (2.1 s), a Vietnamese line that starts while
/// the first is still being sung, and a plain one.
fn sung() -> LyricsDoc {
    let v = |text: &'static str, i: u64| (text, 6_200 + 450 * i, 6_200 + 450 * (i + 1));
    LyricsDoc {
        lines: vec![
            timed(&[
                ("and ", 2_000, 2_300),
                ("the ", 2_300, 2_600),
                ("world ", 2_600, 3_100),
                ("keeps ", 3_100, 3_500),
                ("turning ", 3_500, 5_600),
                ("slowly", 5_600, 6_400),
            ]),
            timed(&[
                v("v\u{e0} ", 0),
                v("t\u{e2}m ", 1),
                v("h\u{1ed3}n ", 2),
                v("t\u{f4}i ", 3),
                v("l\u{e0} ", 4),
                v("m\u{1ed9}t ", 5),
                v("b\u{1ea3}n ", 6),
                v("nh\u{1ea1}c", 7),
            ]),
            LyricLine {
                start_ms: Some(11_000),
                end_ms: Some(14_000),
                text: "a line with no word timing".to_owned(),
                words: Vec::new(),
            },
        ],
        granularity: Granularity::Syllable,
        provider: ProviderId::new("Mock Lyrics"),
        matched: None,
        instrumental: false,
    }
}

/// Which document a shot shows.
#[derive(Clone, Copy)]
enum Doc {
    /// Line-timed lines.
    Plain,
    /// Syllable-timed lines.
    Sung,
    /// Syllable-timed lines with reduced motion.
    SungReduced,
    /// Syllable-timed lines on the light base.
    SungLight,
    /// No lyrics yet: the loading line.
    Loading,
}

/// `(name, window, pane open, document, [(position ms, frames of 16 ms after it)])`.
type Spec = (&'static str, (u32, u32), bool, Doc, &'static [(u64, u32)]);

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

/// Mid-sweep of "world" (audio 2.85 s; the audio clock is the position plus 150 ms).
const SWEEP: &[(u64, u32)] = &[(2_700, 3)];
/// The held word "turning", 0.8 s in: its letters wave and swell.
const HELD: &[(u64, u32)] = &[(4_150, 3)];
/// "slowly" is still being sung while the next line has started: two lines are lit.
const TWO_LIT: &[(u64, u32)] = &[(5_850, 3)];
/// Mid-word in the Vietnamese line.
const VIETNAMESE: &[(u64, u32)] = &[(7_700, 3)];

fn specs() -> Vec<Spec> {
    let full = (1366, 697);
    vec![
        (
            "lyrics-cascade-mid-full-1366x697",
            full,
            false,
            Doc::Plain,
            MID_CASCADE,
        ),
        (
            "lyrics-settled-full-1366x697",
            full,
            false,
            Doc::Plain,
            SETTLED,
        ),
        (
            "lyrics-settled-full-1920x1040",
            (1920, 1040),
            false,
            Doc::Plain,
            SETTLED,
        ),
        (
            "lyrics-cascade-mid-pane-1366x697",
            full,
            true,
            Doc::Plain,
            MID_CASCADE,
        ),
        ("lyrics-words-sweep-1366x697", full, false, Doc::Sung, SWEEP),
        ("lyrics-words-held-1366x697", full, false, Doc::Sung, HELD),
        (
            "lyrics-words-two-lit-1366x697",
            full,
            false,
            Doc::Sung,
            TWO_LIT,
        ),
        (
            "lyrics-words-vietnamese-1366x697",
            full,
            false,
            Doc::Sung,
            VIETNAMESE,
        ),
        (
            "lyrics-words-held-pane-1366x697",
            full,
            true,
            Doc::Sung,
            HELD,
        ),
        (
            "lyrics-words-held-1920x1040",
            (1920, 1040),
            false,
            Doc::Sung,
            HELD,
        ),
        (
            "lyrics-words-reduced-1366x697",
            full,
            false,
            Doc::SungReduced,
            SWEEP,
        ),
        (
            "lyrics-words-light-1366x697",
            full,
            false,
            Doc::SungLight,
            HELD,
        ),
        (
            "lyrics-loading-full-1366x697",
            full,
            false,
            Doc::Loading,
            &[],
        ),
        (
            "lyrics-loading-pane-1366x697",
            full,
            true,
            Doc::Loading,
            &[],
        ),
    ]
}

/// Render the shots named in `only` (all when empty) into `out_dir`.
pub(super) fn render(only: &[String], out_dir: &Path) -> Result<(), String> {
    for (name, size, pane, which, steps) in specs() {
        if !only.is_empty() && !only.iter().any(|n| n == name) {
            continue;
        }
        let backend = mock_backend();
        let scenario = Scenario {
            route: if pane { Route::Home } else { Route::NowPlaying },
            size,
            np_tab: NpTab::Lyrics,
            pane_open: Some(pane),
            full_motion: !matches!(which, Doc::SungReduced),
            loading_lyrics_ms: matches!(which, Doc::Loading).then_some(1_200),
            settings: UiSettings {
                tos_acknowledged: true,
                base: if matches!(which, Doc::SungLight) {
                    BaseMode::Light
                } else {
                    BaseMode::Dark
                },
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
        if !matches!(which, Doc::Loading) {
            app.dispatch(Event::LyricsReady {
                track,
                lookup: LyricsLookup {
                    doc: Some(match which {
                        Doc::Plain => song(),
                        _ => sung(),
                    }),
                    tried: Vec::new(),
                },
                offset_ms: 0,
            });
        }
        // The list slides in over 0.6 s when the lyrics arrive; the shots show it in place.
        for _ in 0..45 {
            app.dispatch(Event::Frame { dt_ms: 16.0 });
        }
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
