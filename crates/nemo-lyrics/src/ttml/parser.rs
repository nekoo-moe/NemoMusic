//! The parse state machine: collects pieces, lines and singers from the token stream.

use nemo_core::lyrics::LyricWord;

use super::scan::{attr, decode_entities};
use super::{TtmlAgent, TtmlLineMeta, parse_time};

/// Deepest element nesting read.
const MAX_DEPTH: usize = 64;
/// Most lines kept.
const MAX_LINES: usize = 20_000;

#[derive(Default)]
struct RawWord {
    start: u64,
    end: Option<u64>,
    text: String,
    /// Layout whitespace (with a line break) followed this span.
    gap: bool,
}

/// One side of a `<p>`: the main vocals or the background vocals.
#[derive(Default)]
struct Side {
    words: Vec<RawWord>,
    plain: String,
    begin: Option<u64>,
    end: Option<u64>,
}

impl Side {
    fn add_text(&mut self, text: &str, in_word: Option<usize>) {
        if let Some(w) = in_word.and_then(|i| self.words.get_mut(i)) {
            w.text.push_str(text);
            return;
        }
        let layout_only = text.trim().is_empty() && text.contains(['\n', '\r']);
        if layout_only {
            if let Some(last) = self.words.last_mut() {
                last.gap = true;
            }
            return;
        }
        if text.trim().is_empty() {
            if let Some(last) = self.words.last_mut()
                && !last.text.ends_with(char::is_whitespace)
            {
                last.text.push(' ');
            }
            if !self.plain.ends_with(' ') && !self.plain.is_empty() {
                self.plain.push(' ');
            }
        } else {
            self.plain.push_str(text);
        }
    }
}

enum Frame {
    /// Anything that is not interesting.
    Other,
    /// Inside a `<p>`.
    P,
    /// A span. `word` is the index of the word it started, `bg` marks background vocals and
    /// `skip` marks translations that are ignored.
    Span {
        word: Option<usize>,
        bg: bool,
        skip: bool,
    },
}

pub(super) struct Line {
    pub(super) start: Option<u64>,
    pub(super) end: Option<u64>,
    pub(super) text: String,
    pub(super) words: Vec<LyricWord>,
    /// Some word is split into several timed pieces.
    pub(super) split: bool,
    pub(super) meta: TtmlLineMeta,
}

/// Add a space to the end of the last piece, unless it has one.
fn space_after_last(out: &mut [LyricWord]) {
    if let Some(last) = out.last_mut()
        && !last.text.ends_with(' ')
    {
        last.text.push(' ');
    }
}

/// Collapse spaces inside pieces and move whitespace to the end of the previous piece. Also
/// says whether some word is split into several pieces: a piece with no trailing space, no
/// layout gap after it, and another piece right behind it.
fn tidy_words(raw: Vec<RawWord>) -> (Vec<LyricWord>, bool) {
    let mut out: Vec<LyricWord> = Vec::new();
    let mut split = false;
    // A layout gap follows the last piece pushed (or a skipped empty span after it).
    let mut gap = false;
    for w in raw {
        let lead = w.text.starts_with(char::is_whitespace);
        let trail = w.text.ends_with(char::is_whitespace);
        let core = w.text.split_whitespace().collect::<Vec<_>>().join(" ");
        if core.is_empty() {
            space_after_last(&mut out);
            gap |= w.gap;
            continue;
        }
        if lead {
            space_after_last(&mut out);
        }
        if let Some(last) = out.last()
            && !last.text.ends_with(' ')
            && !gap
        {
            split = true;
        }
        gap = w.gap;
        out.push(LyricWord {
            start_ms: w.start,
            end_ms: w.end,
            text: if trail { format!("{core} ") } else { core },
        });
    }
    if let Some(last) = out.last_mut() {
        let t = last.text.trim_end().len();
        last.text.truncate(t);
    }
    (out, split)
}

fn finish_side(
    side: Side,
    fallback_begin: Option<u64>,
    fallback_end: Option<u64>,
    meta: TtmlLineMeta,
) -> Option<Line> {
    let (words, split) = tidy_words(side.words);
    let text = if words.is_empty() {
        side.plain.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        words.iter().map(|w| w.text.as_str()).collect::<String>()
    };
    let text = text.trim().to_owned();
    if text.is_empty() {
        return None;
    }
    let start = side
        .begin
        .or_else(|| words.first().map(|w| w.start_ms))
        .or(fallback_begin);
    let end = side
        .end
        .or_else(|| words.last().and_then(|w| w.end_ms))
        .or(fallback_end);
    Some(Line {
        start,
        end,
        text,
        words,
        split,
        meta,
    })
}

/// The state of one parse run.
#[derive(Default)]
pub(super) struct Parser {
    stack: Vec<Frame>,
    pub(super) agents: Vec<TtmlAgent>,
    pub(super) lines: Vec<Line>,
    in_p: bool,
    p_begin: Option<u64>,
    p_end: Option<u64>,
    p_agent: Option<String>,
    main: Side,
    bg: Side,
}

/// `end` or `begin + dur` of an element.
fn end_time(attrs: &[(&str, String)], begin: Option<u64>) -> Option<u64> {
    attr(attrs, "end").and_then(parse_time).or_else(|| {
        let dur = attr(attrs, "dur").and_then(parse_time)?;
        begin.map(|b| b.saturating_add(dur))
    })
}

impl Parser {
    fn skipping(&self) -> bool {
        self.stack
            .iter()
            .any(|f| matches!(f, Frame::Span { skip: true, .. }))
    }

    fn in_background(&self) -> bool {
        self.stack
            .iter()
            .any(|f| matches!(f, Frame::Span { bg: true, .. }))
    }

    /// Handle a start tag. Returns false when parsing should stop (too deep).
    pub(super) fn start(&mut self, name: &str, attrs: &[(&str, String)], empty: bool) -> bool {
        if name == "agent"
            && let Some(id) = attr(attrs, "id")
        {
            self.agents.push(TtmlAgent {
                id: id.to_owned(),
                kind: attr(attrs, "type").map(str::to_owned),
            });
        }
        if empty {
            return true;
        }
        if self.stack.len() >= MAX_DEPTH {
            return false;
        }
        match name {
            "p" => {
                self.in_p = true;
                self.p_begin = attr(attrs, "begin").and_then(parse_time);
                self.p_end = end_time(attrs, self.p_begin);
                self.p_agent = attr(attrs, "agent").map(str::to_owned);
                self.main = Side::default();
                self.bg = Side::default();
                self.stack.push(Frame::P);
            }
            "span" if self.in_p => self.start_span(attrs),
            _ => self.stack.push(Frame::Other),
        }
        true
    }

    fn start_span(&mut self, attrs: &[(&str, String)]) {
        let skipping = self.skipping();
        let in_bg = self.in_background();
        let role = attr(attrs, "role").unwrap_or("");
        let bg_role = role == "x-bg";
        let muted = role == "x-translation" || role == "x-roman";
        let begin = attr(attrs, "begin").and_then(parse_time);
        let end = end_time(attrs, begin);
        let mut word = None;
        if bg_role && !skipping {
            self.bg.begin = self.bg.begin.or(begin);
            self.bg.end = end.or(self.bg.end);
        } else if !muted
            && !skipping
            && let Some(start) = begin
        {
            let side = if in_bg { &mut self.bg } else { &mut self.main };
            side.words.push(RawWord {
                start,
                end,
                text: String::new(),
                gap: false,
            });
            word = Some(side.words.len() - 1);
        }
        self.stack.push(Frame::Span {
            word,
            bg: bg_role || in_bg,
            skip: muted || skipping,
        });
    }

    /// Handle an end tag. Returns false when parsing should stop (enough lines).
    pub(super) fn end(&mut self, name: &str) -> bool {
        if self.stack.pop().is_none() || name != "p" || !self.in_p {
            return true;
        }
        self.in_p = false;
        let meta = |background| TtmlLineMeta {
            agent: self.p_agent.clone(),
            background,
        };
        let main = finish_side(
            std::mem::take(&mut self.main),
            self.p_begin,
            self.p_end,
            meta(false),
        );
        let bg = finish_side(
            std::mem::take(&mut self.bg),
            self.p_begin,
            self.p_end,
            meta(true),
        );
        self.lines.extend(main);
        self.lines.extend(bg);
        self.lines.len() < MAX_LINES
    }

    pub(super) fn text(&mut self, text: &str, raw: bool) {
        if !self.in_p || self.skipping() {
            return;
        }
        let word = self.stack.iter().rev().find_map(|f| match f {
            Frame::Span { word: Some(w), .. } => Some(*w),
            _ => None,
        });
        let decoded = if raw {
            text.to_owned()
        } else {
            decode_entities(text)
        };
        let side = if self.in_background() {
            &mut self.bg
        } else {
            &mut self.main
        };
        side.add_text(&decoded, word);
    }
}
