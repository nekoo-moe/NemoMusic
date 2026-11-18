//! Shaping of one lyric line into a cached paragraph (`lyrics-rebuild` spec). The paragraph is
//! built once per line, document, width and size; drawing only moves, clips and tints it. The
//! pieces of a timed line are found once too: their boxes, and for held pieces their letters.

use iced::advanced::text::{self, Alignment, LineHeight, Paragraph as _, Shaping, Span, Text};
use iced::{Font, Pixels, Rectangle, Size};
use nemo_core::lyrics::{Granularity, LyricLine};

use crate::lyric_pieces::{self, Timing, motion};
use crate::lyrics_view::metrics::Metrics;

/// The renderer's paragraph type.
pub(super) type Para = <iced::Renderer as text::Renderer>::Paragraph;

/// One letter of a held piece, shaped alone so it can move on its own.
pub(super) struct Letter {
    /// The letter (with its combining marks).
    pub para: Para,
    /// Distance from the start of the piece, px.
    pub x: f32,
    /// Advance width, px.
    pub width: f32,
}

/// One timed piece of a line.
pub(super) struct Word {
    /// Its boxes, one per row it wraps onto, relative to the paragraph.
    pub boxes: Vec<Rectangle>,
    /// Sum of the widths of the boxes, px.
    pub total: f32,
    /// Start and end.
    pub timing: Timing,
    /// The letters, for a held piece on one row that the letter wave may lift; else empty.
    pub letters: Vec<Letter>,
}

/// One shaped line.
pub(super) struct Line {
    /// The whole line, with no colour of its own: it takes the colour it is drawn with.
    pub para: Para,
    /// The timed pieces; empty for a line without word timing.
    pub words: Vec<Word>,
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
        let words = lyric_pieces::timings(line)
            .into_iter()
            .zip(&texts)
            .enumerate()
            .map(|(i, (timing, text))| {
                let boxes = para.span_bounds(i);
                let total = boxes.iter().map(|b| b.width).sum();
                let letters = if boxes.len() == 1 && motion::is_long(timing) && waves(text) {
                    letters_of(text.trim_end(), m, font, bounds)
                } else {
                    Vec::new()
                };
                Word {
                    boxes,
                    total,
                    timing,
                    letters,
                }
            })
            .collect();
        return Line { para, words, rtl };
    }
    Line {
        para: Para::with_text(make(line.text.as_str(), bounds, m, font)),
        words: Vec::new(),
        rtl,
    }
}

/// `true` when a piece may wave letter by letter: letters that stand alone, in scripts written
/// left to right without joining (Latin, Cyrillic, Greek, Vietnamese). Joined scripts, right to
/// left scripts and CJK keep their word whole.
fn waves(text: &str) -> bool {
    let word = text.trim();
    !word.is_empty()
        && word
            .chars()
            .all(|c| (c as u32) < 0x0590 || (0x1E00..=0x1FFF).contains(&(c as u32)))
}

/// A combining mark that belongs to the letter before it.
fn combining(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF)
}

/// The text of every letter of `word`: a base character with the marks that follow it.
fn letter_texts(word: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in word.chars() {
        match out.last_mut() {
            Some(last) if combining(c) => last.push(c),
            _ => out.push(c.to_string()),
        }
    }
    out
}

/// Shape the letters of `word`. The offset of a letter is the width of the text before it, so
/// the kerning of the word is kept.
fn letters_of(word: &str, m: &Metrics, font: Font, bounds: Size) -> Vec<Letter> {
    let single = |content: &str| {
        let mut t = make(content, bounds, m, font);
        t.wrapping = text::Wrapping::None;
        Para::with_text(t)
    };
    let parts = letter_texts(word);
    let mut before = String::new();
    let mut x = 0.0;
    parts
        .iter()
        .map(|part| {
            before.push_str(part);
            let reach = single(&before).min_bounds().width;
            let letter = Letter {
                para: single(part),
                x,
                width: (reach - x).max(0.0),
            };
            x = reach;
            letter
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_keep_their_marks_and_only_unjoined_scripts_wave() {
        assert_eq!(letter_texts("abc"), ["a", "b", "c"]);
        // A decomposed Vietnamese letter: o, circumflex, acute.
        assert_eq!(letter_texts("o\u{302}\u{301}k"), ["o\u{302}\u{301}", "k"]);
        assert!(waves("hello") && waves("h\u{1ebf}t") && waves("\u{44f}\u{437}"));
        assert!(!waves("\u{5e9}\u{5dc}\u{5d5}\u{5dd}"), "Hebrew");
        assert!(!waves("\u{645}\u{631}\u{62d}\u{628}\u{627}"), "Arabic");
        assert!(!waves("\u{4f60}\u{597d}"), "CJK");
        assert!(!waves("   "));
    }
}
