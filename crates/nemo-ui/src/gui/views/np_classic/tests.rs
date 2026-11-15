//! The Classic style shows the same facts as the other styles (criterion 5). These tests
//! build a headless renderer: run them with `ICED_TEST_BACKEND=tiny-skia` and `--ignored`.

use iced::{Pixels, Size};
use iced_test::Simulator;
use iced_test::selector::Candidate;

use super::*;
use crate::gui::assets;
use crate::gui::headless::{Scenario, build_app};
use crate::gui::views::kit::badge_text;
use crate::mock::mock_backend;
use crate::route::Route;
use crate::settings::NowPlayingStyle;

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

/// Title, artist, the badge (the signal-path entry), the offset value, the lyrics footer and
/// the Lyrics tab.
fn facts(app: &App) -> Vec<String> {
    let track = app.ui.snapshot.current.clone().expect("a song plays");
    let mut facts = vec![
        track.title.clone(),
        badge_text(app),
        crate::format::offset_seconds(app.ui.lyrics_offset_ms),
        "Lyrics".to_owned(),
    ];
    facts.extend(track.artists.first().cloned());
    if let LyricsState::Ready(doc) = &app.ui.lyrics {
        facts.push(crate::lyrics_view::footer(Some(doc)));
    }
    facts
}

fn app_at(route: Route, size: (u32, u32)) -> App {
    let backend = mock_backend();
    let mut app = build_app(
        &backend,
        &Scenario {
            route,
            size,
            ..Scenario::default()
        },
    );
    app.ui.settings.now_playing_style = NowPlayingStyle::Classic;
    app
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn the_full_window_shows_every_fact_wide_and_compact() {
    for size in [(1366, 697), (1920, 1040), (1000, 640)] {
        let app = app_at(Route::NowPlaying, size);
        let shown = texts(view(&app, size.0, size.1), size);
        for fact in facts(&app) {
            assert!(
                shown.iter().any(|t| t.contains(fact.as_str())),
                "{size:?}: {fact}"
            );
        }
    }
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn the_pane_shows_every_fact() {
    let size = (1920, 1040);
    let app = app_at(Route::Home, size);
    let shown = texts(pane(&app, 360.0), (360, 1040));
    for fact in facts(&app) {
        assert!(shown.iter().any(|t| t.contains(fact.as_str())), "{fact}");
    }
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn the_queue_tab_replaces_the_lyrics_and_keeps_the_player() {
    let size = (1366, 697);
    let mut app = app_at(Route::NowPlaying, size);
    app.compact_tab = CompactTab::Queue;
    let shown = texts(view(&app, size.0, size.1), size);
    let title = app.ui.snapshot.current.clone().expect("a song plays").title;
    assert!(shown.iter().any(|t| t.contains(title.as_str())));
    assert!(shown.iter().any(|t| t == "Queue"));
}
