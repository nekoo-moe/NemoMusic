//! Tests for the style dispatch of the Now Playing surfaces. The headless one builds a renderer:
//! run it with `ICED_TEST_BACKEND=tiny-skia` and `--ignored`.

use iced::{Pixels, Size};
use iced_test::Simulator;
use iced_test::selector::Candidate;

use super::*;
use crate::gui::assets;
use crate::gui::headless::{Scenario, build_app};
use crate::gui::views::kit::badge_text;
use crate::mock::mock_backend;
use crate::route::Route;

/// The window sizes of the three surfaces: wide full screen, compact full screen, and Home with
/// the right pane open.
const SURFACES: [(Route, (u32, u32)); 3] = [
    (Route::NowPlaying, (1366, 697)),
    (Route::NowPlaying, (1000, 640)),
    (Route::Home, (1920, 1040)),
];

/// The `App` state every style reads and none may reset.
fn kept(app: &App) -> (std::time::Duration, i64, f32, f32, CompactTab) {
    (
        app.backend.player.position(),
        app.ui.lyrics_offset_ms,
        app.ui.lyric_scroll.value,
        app.ui.lyric_scroll.target,
        app.compact_tab,
    )
}

#[test]
fn a_style_change_keeps_position_offset_scroll_and_tab() {
    for (route, size) in SURFACES {
        let backend = mock_backend();
        let mut app = build_app(
            &backend,
            &Scenario {
                route,
                size,
                ..Scenario::default()
            },
        );
        app.compact_tab = CompactTab::Queue;
        app.ui.lyrics_offset_ms = 350;
        app.ui.lyric_scroll.snap(120.0);
        let before = kept(&app);
        for style in NowPlayingStyle::ALL {
            app.ui.settings.now_playing_style = style;
            drop(app.view());
            assert_eq!(kept(&app), before, "{style:?} at {size:?}");
        }
    }
}

/// Every text drawn in `view` at `size`.
fn texts(view: Element<'_, Message>, size: (u32, u32)) -> Vec<String> {
    let settings = iced::Settings {
        fonts: vec![
            assets::FONT_REGULAR.into(),
            assets::FONT_MEDIUM.into(),
            assets::FONT_SEMIBOLD.into(),
        ],
        default_font: assets::font(false),
        default_text_size: Pixels(14.0),
        ..iced::Settings::default()
    };
    // Window sizes are small positive numbers.
    #[allow(clippy::cast_precision_loss)]
    let logical = Size::new(size.0 as f32, size.1 as f32);
    let mut sim = Simulator::with_size(settings, logical, view);
    let mut out = Vec::new();
    // The selector never matches, so it visits the whole tree; the error is expected.
    let _ = sim.find(|candidate: Candidate<'_>| -> Option<()> {
        if let Candidate::Text { content, .. } = candidate {
            out.push(content.to_owned());
        }
        None
    });
    out
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn every_style_shows_the_same_facts_on_every_surface() {
    for (route, size) in SURFACES {
        let backend = mock_backend();
        let mut app = build_app(
            &backend,
            &Scenario {
                route,
                size,
                ..Scenario::default()
            },
        );
        let track = app.ui.snapshot.current.clone().expect("a song plays");
        let mut facts = vec![track.title.clone(), badge_text(&app)];
        facts.extend(track.artists.first().cloned());
        if let LyricsState::Ready(doc) = &app.ui.lyrics {
            facts.push(crate::lyrics_view::footer(Some(doc)));
        }
        for style in NowPlayingStyle::ALL {
            app.ui.settings.now_playing_style = style;
            let shown = texts(app.view(), size);
            for fact in &facts {
                assert!(
                    shown.iter().any(|t| t.contains(fact.as_str())),
                    "{style:?} at {size:?}: {fact}"
                );
            }
        }
    }
}
