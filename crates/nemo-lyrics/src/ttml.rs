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

use nemo_core::lyrics::{Granularity, LyricLine, LyricWord, LyricsDoc, ProviderId};

/// Largest input read.
const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
/// Deepest element nesting read.
const MAX_DEPTH: usize = 64;
/// Most lines kept.
const MAX_LINES: usize = 20_000;
/// Largest time value accepted (a day), in milliseconds.
const MAX_TIME_MS: u64 = 86_400_000;

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

enum Tok<'a> {
    Start {
        name: &'a str,
        attrs: &'a str,
        empty: bool,
    },
    End {
        name: &'a str,
    },
    Text {
        text: &'a str,
        raw: bool,
    },
}

/// Local part of a qualified name (`ttm:agent` gives `agent`).
fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// The next token, or `None` at the end. Comments, declarations and processing instructions are
/// skipped.
fn next_token<'a>(src: &'a str, pos: &mut usize) -> Option<Tok<'a>> {
    loop {
        let rest = src.get(*pos..)?;
        if rest.is_empty() {
            return None;
        }
        if !rest.starts_with('<') {
            let end = rest.find('<').unwrap_or(rest.len());
            *pos += end;
            return Some(Tok::Text {
                text: &rest[..end],
                raw: false,
            });
        }
        if let Some(body) = rest.strip_prefix("<!--") {
            *pos += match body.find("-->") {
                Some(i) => 4 + i + 3,
                None => rest.len(),
            };
            continue;
        }
        if let Some(body) = rest.strip_prefix("<![CDATA[") {
            let (text, used) = match body.find("]]>") {
                Some(i) => (&body[..i], 9 + i + 3),
                None => (body, rest.len()),
            };
            *pos += used;
            return Some(Tok::Text { text, raw: true });
        }
        if rest.starts_with("<?") || rest.starts_with("<!") {
            *pos += rest.find('>').map_or(rest.len(), |i| i + 1);
            continue;
        }
        if let Some(body) = rest.strip_prefix("</") {
            let close = body.find('>');
            let name = &body[..close.unwrap_or(body.len())];
            *pos += 2 + close.map_or(body.len(), |i| i + 1);
            return Some(Tok::End { name: name.trim() });
        }
        // A start tag. Find its closing '>' outside quotes.
        let body = &rest[1..];
        let mut quote: Option<char> = None;
        let mut close = None;
        for (i, c) in body.char_indices() {
            match (quote, c) {
                (Some(q), c) if c == q => quote = None,
                (None, '"' | '\'') => quote = Some(c),
                (None, '>') => {
                    close = Some(i);
                    break;
                }
                _ => {}
            }
        }
        let Some(close) = close else {
            // Unterminated tag: drop the rest.
            *pos += rest.len();
            return None;
        };
        *pos += 1 + close + 1;
        let inner = &body[..close];
        let (inner, empty) = match inner.strip_suffix('/') {
            Some(i) => (i, true),
            None => (inner, false),
        };
        let name_end = inner
            .find(|c: char| c.is_whitespace())
            .unwrap_or(inner.len());
        return Some(Tok::Start {
            name: &inner[..name_end],
            attrs: &inner[name_end..],
            empty,
        });
    }
}

/// Decode the five XML entities and numeric references. Unknown entities stay as written.
fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let decoded = tail.find(';').filter(|&j| j <= 10).and_then(|j| {
            let name = &tail[1..j];
            let c = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => name.strip_prefix('#').and_then(|n| {
                    let code = match n.strip_prefix(['x', 'X']) {
                        Some(h) => u32::from_str_radix(h, 16).ok()?,
                        None => n.parse::<u32>().ok()?,
                    };
                    char::from_u32(code)
                }),
            };
            c.map(|c| (c, j + 1))
        });
        if let Some((c, used)) = decoded {
            out.push(c);
            rest = &tail[used..];
        } else {
            out.push('&');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

/// Attributes as `(local name, decoded value)`.
fn parse_attrs(s: &str) -> Vec<(&str, String)> {
    let mut out = Vec::new();
    let mut rest = s.trim_start();
    while !rest.is_empty() {
        let Some(eq) = rest.find('=') else { break };
        let name = rest[..eq].trim();
        let after = rest[eq + 1..].trim_start();
        let Some(q) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            break;
        };
        let body = &after[1..];
        let Some(end) = body.find(q) else { break };
        if !name.is_empty() {
            out.push((local(name), decode_entities(&body[..end])));
        }
        rest = body[end + 1..].trim_start();
    }
    out
}

fn attr<'a>(attrs: &'a [(&str, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, v)| v.as_str())
}

/// `"12"`, `"12.5"` to milliseconds, where the number counts `unit_ms` milliseconds.
fn decimal_to_ms(s: &str, unit_ms: u64) -> Option<u64> {
    let (int, frac) = s.split_once('.').unwrap_or((s, ""));
    if (int.is_empty() && frac.is_empty())
        || !int.bytes().all(|b| b.is_ascii_digit())
        || !frac.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let whole = if int.is_empty() {
        0
    } else {
        int.parse::<u64>().ok()?
    };
    let mut milli = 0u64;
    for (i, b) in frac.bytes().take(3).enumerate() {
        milli += u64::from(b - b'0') * [100, 10, 1][i];
    }
    let ms = whole
        .checked_mul(unit_ms)?
        .checked_add(milli * unit_ms / 1000)?;
    (ms <= MAX_TIME_MS).then_some(ms)
}

/// Parse a TTML time expression into milliseconds.
#[must_use]
pub fn parse_time(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(n) = s.strip_suffix("ms") {
        return decimal_to_ms(n, 1);
    }
    if let Some(n) = s.strip_suffix('s') {
        return decimal_to_ms(n, 1000);
    }
    if let Some(n) = s.strip_suffix('m') {
        return decimal_to_ms(n, 60_000);
    }
    if let Some(n) = s.strip_suffix('h') {
        return decimal_to_ms(n, 3_600_000);
    }
    let parts: Vec<&str> = s.split(':').collect();
    let (h, m, sec) = match parts.as_slice() {
        [sec] => ("0", "0", *sec),
        [m, sec] => ("0", *m, *sec),
        [h, m, sec] => (*h, *m, *sec),
        _ => return None,
    };
    let int = |t: &str| {
        (!t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()))
            .then(|| t.parse::<u64>().ok())
            .flatten()
    };
    let total = int(h)?
        .checked_mul(3_600_000)?
        .checked_add(int(m)?.checked_mul(60_000)?)?
        .checked_add(decimal_to_ms(sec, 1000)?)?;
    (total <= MAX_TIME_MS).then_some(total)
}

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

struct Line {
    start: Option<u64>,
    end: Option<u64>,
    text: String,
    words: Vec<LyricWord>,
    /// Some word is split into several timed pieces.
    split: bool,
    meta: TtmlLineMeta,
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
struct Parser {
    stack: Vec<Frame>,
    agents: Vec<TtmlAgent>,
    lines: Vec<Line>,
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
    fn start(&mut self, name: &str, attrs: &[(&str, String)], empty: bool) -> bool {
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
    fn end(&mut self, name: &str) -> bool {
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

    fn text(&mut self, text: &str, raw: bool) {
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

#[cfg(test)]
mod tests {
    use super::*;

    const P: ProviderId = ProviderId::new("test");

    // Synthetic fixture in the Apple layout. Not real lyrics.
    const SYLLABLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:ttm="http://www.w3.org/ns/ttml#metadata" xmlns:itunes="http://music.apple.com/lyric-ttml-internal" itunes:timing="Word">
<head><metadata><ttm:agent type="person" xml:id="v1"/><ttm:agent type="person" xml:id="v2"/></metadata></head>
<body dur="00:10.000"><div begin="00:01.000" end="00:09.000">
<p begin="00:01.000" end="00:03.000" ttm:agent="v1"><span begin="00:01.000" end="00:01.400">Hel</span><span begin="00:01.400" end="00:02.000">lo</span> <span begin="00:02.000" end="00:03.000">you</span><span ttm:role="x-bg"><span begin="00:02.500" end="00:03.000">(ooh)</span></span></p>
<p begin="00:03.500" end="00:05.000" ttm:agent="v2"><span begin="00:03.500" end="00:04.000">Hi</span> <span begin="00:04.000" end="00:05.000">&amp; bye</span><span ttm:role="x-translation">xin chao</span></p>
</div></body></tt>"#;

    #[test]
    fn split_words_make_the_doc_syllable_synced() {
        let t = parse_ttml_detailed(SYLLABLE, P);
        assert_eq!(t.doc.granularity, Granularity::Syllable);
        let texts: Vec<&str> = t.doc.lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["Hello you", "(ooh)", "Hi & bye"]);
        let first = &t.doc.lines[0];
        assert_eq!(first.start_ms, Some(1000));
        assert_eq!(first.end_ms, Some(3000));
        let words: Vec<(&str, u64)> = first
            .words
            .iter()
            .map(|w| (w.text.as_str(), w.start_ms))
            .collect();
        assert_eq!(words, [("Hel", 1000), ("lo ", 1400), ("you", 2000)]);
        assert_eq!(first.words[0].end_ms, Some(1400));
    }

    #[test]
    fn background_and_agents_are_reported() {
        let t = parse_ttml_detailed(SYLLABLE, P);
        let bg: Vec<bool> = t.meta.iter().map(|m| m.background).collect();
        assert_eq!(bg, [false, true, false]);
        assert_eq!(t.meta[0].agent.as_deref(), Some("v1"));
        assert_eq!(t.meta[2].agent.as_deref(), Some("v2"));
        assert_eq!(t.agents.len(), 2);
        assert!(t.is_duet());
        assert_eq!(t.doc.lines[1].start_ms, Some(2500));
    }

    #[test]
    fn line_only_ttml_is_line_granularity() {
        let src = r#"<tt><body><div>
            <p begin="0:10.5" end="0:12">First line</p>
            <p begin="12.5s">Second &lt;line&gt;</p>
        </div></body></tt>"#;
        let doc = parse_ttml(src, P);
        assert_eq!(doc.granularity, Granularity::Line);
        assert_eq!(doc.lines[0].start_ms, Some(10_500));
        assert_eq!(doc.lines[0].end_ms, Some(12_000));
        assert_eq!(doc.lines[1].start_ms, Some(12_500));
        assert_eq!(doc.lines[1].text, "Second <line>");
        assert!(doc.lines.iter().all(|l| l.words.is_empty()));
    }

    #[test]
    fn untimed_ttml_is_unsynced() {
        let doc = parse_ttml("<tt><body><div><p>one</p><p>two</p></div></body></tt>", P);
        assert_eq!(doc.granularity, Granularity::Unsynced);
        assert_eq!(doc.lines.len(), 2);
        assert!(doc.lines.iter().all(|l| l.start_ms.is_none()));
    }

    #[test]
    fn pretty_printed_spans_use_their_own_spaces() {
        let src = "<tt><body><div><p begin=\"1.0\" end=\"3.0\">\n  <span begin=\"1.0\" end=\"2.0\">One </span>\n  <span begin=\"2.0\" end=\"3.0\">two</span>\n</p></div></body></tt>";
        let doc = parse_ttml(src, P);
        assert_eq!(doc.lines[0].text, "One two");
        assert_eq!(doc.lines[0].words.len(), 2);
    }

    #[test]
    fn lines_are_sorted_by_start() {
        let src = r#"<tt><body><div><p begin="5.0">b</p><p begin="1.0">a</p></div></body></tt>"#;
        let doc = parse_ttml(src, P);
        let texts: Vec<&str> = doc.lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["a", "b"]);
    }

    #[test]
    fn times_parse() {
        assert_eq!(parse_time("00:01:02.345"), Some(62_345));
        assert_eq!(parse_time("1:02.5"), Some(62_500));
        assert_eq!(parse_time("7"), Some(7000));
        assert_eq!(parse_time("1.5s"), Some(1500));
        assert_eq!(parse_time("250ms"), Some(250));
        assert_eq!(parse_time("2m"), Some(120_000));
        assert_eq!(parse_time("00:00:01:12"), None);
        assert_eq!(parse_time("abc"), None);
        assert_eq!(parse_time(""), None);
        assert_eq!(parse_time("99999999999999999999"), None);
    }

    #[test]
    fn garbage_is_survivable() {
        for src in [
            "",
            "<",
            "<p",
            "<p begin=",
            "</p></p>",
            "<![CDATA[",
            "<!--",
            "&#xFFFFFFFF;",
        ] {
            let _ = parse_ttml(src, P);
        }
        assert!(matches!(
            parse_ttml("not xml at all", P).lines.as_slice(),
            []
        ));
    }

    #[test]
    fn cdata_and_numeric_entities() {
        let src =
            "<tt><body><div><p begin=\"1\"><![CDATA[a < b]]> &#65;&#x42;</p></div></body></tt>";
        assert_eq!(parse_ttml(src, P).lines[0].text, "a < b AB");
    }
}
