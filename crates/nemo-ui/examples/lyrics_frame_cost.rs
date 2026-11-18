//! Measures the CPU time of one redraw of the lyric list with lit lines (`lyrics-rebuild`): the
//! time to rebuild the widget, lay it out (the cached paragraphs are reused) and record its
//! drawing into the renderer, per frame, for 600 frames (10 s of a song) of a syllable-timed
//! song and of a line-timed one. Rasterizing the recorded primitives (GPU or CPU) is not part of
//! it. Run it with the profile you care about:
//!
//! ```text
//! ICED_TEST_BACKEND=tiny-skia cargo run -p nemo-ui --example lyrics_frame_cost --features gui --release
//! ```

use std::borrow::Cow;
use std::time::{Duration, Instant};

use iced::advanced::Renderer as _;
use iced::advanced::layout::{Layout, Limits};
use iced::advanced::renderer::{Headless, Style};
use iced::advanced::widget::Tree;
use iced::{Color, Element, Pixels, Rectangle, Size, mouse};
use nemo_core::lyrics::{Granularity, LyricLine, LyricWord, LyricsDoc, ProviderId};
use nemo_ui::gui::assets;
use nemo_ui::gui::lyric_list::LyricList;
use nemo_ui::lyrics_view::clock::{Clocks, Timeline};
use nemo_ui::lyrics_view::layout;
use nemo_ui::lyrics_view::metrics::Metrics;
use nemo_ui::lyrics_view::note::Focus;

/// Lines in the song, one every 3.5 s.
const LINES: u64 = 40;
/// Frames measured, at 60 per second.
const FRAMES: u32 = 600;

/// The words of every line `(text, length ms)`; the fifth is held for 1.8 s.
const WORDS: [(&str, u64); 8] = [
    ("and ", 300),
    ("the ", 300),
    ("world ", 350),
    ("keeps ", 400),
    ("turning ", 1_800),
    ("slowly ", 300),
    ("through ", 300),
    ("night", 350),
];

fn song(timed: bool) -> LyricsDoc {
    let lines = (0..LINES)
        .map(|i| {
            let start = 2_000 + 3_500 * i;
            let mut at = start;
            let words: Vec<LyricWord> = WORDS
                .iter()
                .map(|(text, len)| {
                    let word = LyricWord {
                        start_ms: at,
                        end_ms: Some(at + len),
                        text: (*text).to_owned(),
                    };
                    at += len;
                    word
                })
                .collect();
            LyricLine {
                start_ms: Some(start),
                end_ms: Some(at),
                text: WORDS.iter().map(|w| w.0).collect(),
                words: if timed { words } else { Vec::new() },
            }
        })
        .collect();
    LyricsDoc {
        lines,
        granularity: if timed {
            Granularity::Syllable
        } else {
            Granularity::Line
        },
        provider: ProviderId::new("Probe"),
        matched: None,
        instrumental: false,
    }
}

fn percentile(sorted: &[Duration], p: f64) -> Duration {
    // The index of a few hundred samples: exact in f64 and in range.
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let at = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted.get(at).copied().unwrap_or_default()
}

fn measure(renderer: &mut iced::Renderer, timed: bool) -> Vec<Duration> {
    let doc = song(timed);
    let timeline = Timeline::new(&doc);
    let metrics = Metrics::full(1366.0);
    let tops = layout::tops(&vec![1; doc.lines.len()], &metrics);
    let (width, height) = (837.0, 569.0);
    let limits = Limits::new(Size::ZERO, Size::new(width, height));
    let theme = nemo_ui::gui::style::theme();
    let style = Style {
        text_color: Color::WHITE,
    };
    let viewport = Rectangle::with_size(Size::new(width, height));
    let mut tree: Option<Tree> = None;
    let mut samples = Vec::new();
    // From the start of line 5, 16.7 ms a frame.
    let first_ms: u64 = 2_000 + 3_500 * 5;
    for frame in 0..FRAMES {
        let position = i64::try_from(first_ms + u64::from(frame) * 1_000 / 60).unwrap_or(0);
        let clocks = Clocks::new(doc.granularity, position, 0);
        let focus = Focus {
            active: timeline.active(&clocks),
            note: None,
        };
        let scroll = layout::scroll_target(&tops, &metrics, &focus, height);
        let started = Instant::now();
        let mut element: Element<'_, (), iced::Theme, iced::Renderer> = LyricList {
            doc: &doc,
            rev: 1,
            first: 0,
            tops: Cow::Borrowed(&tops),
            metrics,
            wrap_w: width - 4.0,
            scroll,
            timeline: &timeline,
            clocks,
            note: None,
            ink: Color::WHITE,
            font: assets::display_font(true),
            reduced: false,
            fade_edges: true,
            cascade: None,
            enter: 1.0,
        }
        .into();
        let tree = tree.get_or_insert_with(|| Tree::new(&element));
        tree.diff(&element);
        let node = element.as_widget_mut().layout(tree, renderer, &limits);
        element.as_widget().draw(
            tree,
            renderer,
            &theme,
            &style,
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &viewport,
        );
        let spent = started.elapsed();
        renderer.reset(viewport);
        // The first frames shape the paragraphs: they are the warm-up, not a frame of the song.
        if frame >= 30 {
            samples.push(spent);
        }
    }
    samples.sort();
    samples
}

fn report(name: &str, times: &[Duration]) {
    let total: Duration = times.iter().sum();
    let mean = total / u32::try_from(times.len().max(1)).unwrap_or(1);
    println!(
        "{name}: mean {:.3} ms, p95 {:.3} ms, p99 {:.3} ms, max {:.3} ms ({} frames)",
        mean.as_secs_f64() * 1000.0,
        percentile(times, 0.95).as_secs_f64() * 1000.0,
        percentile(times, 0.99).as_secs_f64() * 1000.0,
        times.last().copied().unwrap_or_default().as_secs_f64() * 1000.0,
        times.len()
    );
}

fn main() {
    let made = iced_test::futures::futures::executor::block_on(<iced::Renderer as Headless>::new(
        assets::font(false),
        Pixels(14.0),
        Some("tiny-skia"),
    ));
    let Some(mut renderer) = made else {
        eprintln!("no headless renderer: set ICED_TEST_BACKEND=tiny-skia");
        std::process::exit(2);
    };
    if let Ok(mut fonts) = iced::advanced::graphics::text::font_system().write() {
        for font in assets::ALL_FONTS {
            fonts.load_font((*font).into());
        }
    }
    let profile = if cfg!(debug_assertions) {
        "dev profile (unoptimized)"
    } else {
        "optimized"
    };
    println!("lyric list redraw, 1366 x 697 window, {profile}");
    report("syllable-timed song", &measure(&mut renderer, true));
    report("line-timed song", &measure(&mut renderer, false));
}
