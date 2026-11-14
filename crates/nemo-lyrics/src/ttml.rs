//! TTML parser for Apple-style lyrics (line and syllable timing).
//!
//! Supported (checked against the structure Better Lyrics and Unison document; fixtures are
//! synthetic):
//! - `<p begin end ttm:agent>` is a line. `<span begin end>` inside it is a timed word or
//!   syllable; each becomes one [`LyricWord`] (a piece).
//! - Level: [`Granularity::Syllable`] when some line splits a word into several timed pieces,
//!   that is a timed span with no trailing whitespace followed in the same line by another timed
//!   span with no whitespace text between them (`<span>Hel</span><span>lo</span>`). Timed
//!   whole words give [`Granularity::Word`], timed lines only [`Granularity::Line`], no times
//!   [`Granularity::Unsynced`].
//! - A piece's `text` keeps a trailing space when the source has one (a space text node between
//!   spans, or a space at the end of the span). Join piece texts without a separator.
//!   Whitespace-only text that contains a line break is layout: it adds no space, but it still
//!   separates two spans, so it never makes them pieces of one word.
//! - `<span ttm:role="x-bg">` is background vocals. It becomes its own line right after the main
//!   line, flagged in [`TtmlLineMeta::background`].
//! - `<ttm:agent xml:id>` in the head and `ttm:agent` on a `<p>` give singers ([`TtmlDoc::agents`],
//!   [`TtmlLineMeta::agent`]); [`TtmlDoc::is_duet`] is true with two or more singers.
//! - `x-translation` and `x-roman` spans are skipped.
//! - Times: `HH:MM:SS.fff`, `MM:SS.fff`, `SS.fff`, `12.5s`, `1500ms`, `2m`, `1h`. Frame counts
//!   are not supported (the time is dropped).
//! - Times in nested spans are read as absolute media time, as Apple writes them.
//!
//! The scanner is hand written (no XML crate is allowed). It never panics, never fails, and
//! skips what it does not understand. Input over 4 MiB or 64 levels deep is cut off.

#[cfg(doc)]
use nemo_core::lyrics::LyricWord;
use nemo_core::lyrics::{Granularity, LyricLine, LyricsDoc, ProviderId};

/// Largest input read.
const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;

/// Extra facts about one line of a TTML document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtmlLineMeta {
    /// The singer id (`ttm:agent` of the `<p>`), for example `v1`.
    pub agent: Option<String>,
    /// The line is background vocals.
    pub background: bool,
}

/// One singer declared in the head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtmlAgent {
    /// The `xml:id`.
    pub id: String,
    /// The `type` (`person`, `group`, `other`), if given.
    pub kind: Option<String>,
}

/// A parsed TTML document with the facts [`LyricsDoc`] cannot hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtmlDoc {
    /// The lyrics.
    pub doc: LyricsDoc,
    /// One entry per `doc.lines`, same order.
    pub meta: Vec<TtmlLineMeta>,
    /// Singers declared in the head.
    pub agents: Vec<TtmlAgent>,
}

impl TtmlDoc {
    /// Two or more different singers sing lines of this song.
    #[must_use]
    pub fn is_duet(&self) -> bool {
        let mut first: Option<&str> = None;
        for m in &self.meta {
            if m.background {
                continue;
            }
            match (&m.agent, first) {
                (Some(a), None) => first = Some(a),
                (Some(a), Some(f)) if a != f => return true,
                _ => {}
            }
        }
        false
    }
}

/// Parse TTML into a document plus the line facts (background, singer) it carries.
///
/// Never fails and never panics. An empty or unusable input gives a document with no lines.
#[must_use]
pub fn parse_ttml_detailed(text: &str, provider: ProviderId) -> TtmlDoc {
    let mut end = text.len().min(MAX_INPUT_BYTES);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let src = &text[..end];

    let mut pos = 0usize;
    let mut parser = Parser::default();
    while let Some(tok) = next_token(src, &mut pos) {
        let go_on = match tok {
            Tok::Start { name, attrs, empty } => {
                parser.start(local(name), &parse_attrs(attrs), empty)
            }
            Tok::End { name } => parser.end(local(name)),
            Tok::Text { text, raw } => {
                parser.text(text, raw);
                true
            }
        };
        if !go_on {
            break;
        }
    }
    let Parser {
        agents, mut lines, ..
    } = parser;

    // Sort by start when every line is timed. The sort is stable, so a background line stays
    // after its main line when they start together.
    let all_timed = !lines.is_empty() && lines.iter().all(|l| l.start.is_some());
    if all_timed {
        lines.sort_by_key(|l| l.start);
    }
    let any_words = lines.iter().any(|l| !l.words.is_empty());
    let granularity = if lines.iter().any(|l| l.split) {
        Granularity::Syllable
    } else if any_words {
        Granularity::Word
    } else if lines.iter().any(|l| l.start.is_some()) {
        Granularity::Line
    } else {
        Granularity::Unsynced
    };
    let timed = granularity != Granularity::Unsynced;

    let mut meta = Vec::with_capacity(lines.len());
    let out_lines = lines
        .into_iter()
        .map(|l| {
            meta.push(l.meta);
            LyricLine {
                start_ms: if timed { l.start } else { None },
                end_ms: if timed { l.end } else { None },
                text: l.text,
                words: l.words,
            }
        })
        .collect();
    TtmlDoc {
        doc: LyricsDoc {
            lines: out_lines,
            granularity,
            provider,
            matched: None,
            instrumental: false,
        },
        meta,
        agents,
    }
}

/// Parse TTML into a [`LyricsDoc`]. See [`parse_ttml_detailed`] for the background and singer
/// facts.
#[must_use]
pub fn parse_ttml(text: &str, provider: ProviderId) -> LyricsDoc {
    parse_ttml_detailed(text, provider).doc
}

mod parser;
mod scan;
mod time;

use parser::Parser;
use scan::{Tok, local, next_token, parse_attrs};
pub use time::parse_time;

#[cfg(test)]
mod tests;
