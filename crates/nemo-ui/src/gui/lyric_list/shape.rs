//! Shaping of one lyric line into a cached paragraph (`lyrics-rebuild` spec). The paragraph is
//! built once per line, document, width and size; drawing only moves, clips and tints it.

use iced::advanced::text::{self, Alignment, LineHeight, Paragraph as _, Shaping, Span, Text};
use iced::{Font, Pixels, Rectangle, Size};
use nemo_core::lyrics::{Granularity, LyricLine};

use crate::lyric_pieces::{self, Timing};
use crate::lyrics_view::metrics::Metrics;

/// The renderer's paragraph type.
pub(super) type Para = <iced::Renderer as text::Renderer>::Paragraph;

/// One shaped line.
pub(super) struct Line {
    /// The whole line, with no colour of its own: it takes the colour it is drawn with.
    pub para: Para,
    /// For a word- or syllable-timed line: the boxes of every piece (one per row it wraps onto),
    /// relative to the paragraph.
    pub boxes: Vec<Vec<Rectangle>>,
    /// Timing of every piece, parallel to `boxes`.
    pub timings: Vec<Timing>,
    /// The text reads right to left: the sweep runs from the right, and it scales around its
    /// right edge.
    pub rtl: bool,
}

/// The text description of a line: all lines share size, line height, font and wrapping.
fn make<C>(content: C, bounds: Size, m: &Metrics, font: Font) -> Text<C, Font> {
    Text {
        content,
        bounds,
        size: Pixels(m.size),
        line_height: LineHeight::Absolute(Pixels(m.line_h)),
        font,
        align_x: Alignment::Default,
        align_y: iced::alignment::Vertical::Top,
        shaping: Shaping::Advanced,
        wrapping: text::Wrapping::WordOrGlyph,
    }
}

/// Shape `line` for `wrap_w` px at the size of `m`. A line whose level is below word timing, or
/// that has no timed pieces, is one plain paragraph.
pub(super) fn shape(
    line: &LyricLine,
    level: Granularity,
    m: &Metrics,
    wrap_w: f32,
    font: Font,
) -> Line {
    let bounds = Size::new(wrap_w, m.line_h * 12.0);
    let rtl = lyric_pieces::is_rtl(&line.text);
    if level >= Granularity::Word && !line.words.is_empty() {
        let texts = lyric_pieces::texts(line);
        let spans: Vec<Span<'_, (), Font>> = texts
            .iter()
            .map(|t| Span::<(), Font>::new(t.clone()))
            .collect();
        let para = Para::with_spans(make(spans.as_slice(), bounds, m, font));
        let boxes = (0..texts.len()).map(|i| para.span_bounds(i)).collect();
        return Line {
            para,
            boxes,
            timings: lyric_pieces::timings(line),
            rtl,
        };
    }
    Line {
        para: Para::with_text(make(line.text.as_str(), bounds, m, font)),
        boxes: Vec::new(),
        timings: Vec::new(),
        rtl,
    }
}
