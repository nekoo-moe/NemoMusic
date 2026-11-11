//! LRC and enhanced LRC parser.
//!
//! Supported: `[mm:ss.xx]`, `[mm:ss.xxx]`, `[mm:ss]`, several timestamps on one line, CRLF, a
//! leading BOM, blank lines, the `[offset:+/-ms]` header and inline `<mm:ss.xx>` word tags.
//! A word split by tags into several pieces (`<00:01.00>Hel<00:01.40>lo`) makes the doc
//! syllable synced.
//! `[ar:]`, `[ti:]`, `[al:]`, `[by:]`, `[length:]` and other `key:value` tags are ignored.
//!
//! Offset sign (LRC convention): a positive `[offset:+500]` shifts lyrics EARLIER, so
//! `time = stamp - offset`. A negative offset shifts them later. Results never go below 0.
//! If several offset tags exist, the first one wins.
//!
//! End times are never invented. The only end time set is the one a trailing inline word tag
//! (`<mm:ss.xx>` after the last word) gives for that word.

use nemo_core::lyrics::{Granularity, LyricLine, LyricWord, LyricsDoc, ProviderId};

/// Parse digits, saturating at `u64::MAX`. `None` if empty or not all ASCII digits.
fn parse_digits(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(s.bytes().fold(0u64, |acc, b| {
        acc.saturating_mul(10).saturating_add(u64::from(b - b'0'))
    }))
}

/// Parse `mm:ss`, `mm:ss.x`, `mm:ss.xx`, `mm:ss.xxx` into milliseconds (saturating).
/// Fraction digits beyond the third are ignored.
fn parse_timestamp(s: &str) -> Option<u64> {
    let (mm, rest) = s.split_once(':')?;
    let (ss, frac) = match rest.split_once('.') {
        Some((ss, frac)) => (ss, Some(frac)),
        None => (rest, None),
    };
    let mm = parse_digits(mm)?;
    let ss = parse_digits(ss)?;
    let mut frac_ms = 0u64;
    if let Some(frac) = frac {
        if frac.is_empty() || !frac.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let mut scale = 100u64;
        for b in frac.bytes().take(3) {
            frac_ms += u64::from(b - b'0') * scale;
            scale /= 10;
        }
    }
    Some(
        mm.saturating_mul(60)
            .saturating_add(ss)
            .saturating_mul(1000)
            .saturating_add(frac_ms),
    )
}

/// Parse the value of an `offset:` tag. `None` if the tag is not an offset tag.
fn parse_offset_tag(tag: &str) -> Option<i64> {
    let (key, value) = tag.split_once(':')?;
    if !key.trim().eq_ignore_ascii_case("offset") {
        return None;
    }
    let value = value.trim();
    let (negative, digits) = match value.as_bytes().first() {
        Some(b'-') => (true, &value[1..]),
        Some(b'+') => (false, &value[1..]),
        _ => (false, value),
    };
    // A malformed offset counts as 0 rather than failing the whole file.
    let magnitude = i64::try_from(parse_digits(digits.trim()).unwrap_or(0)).unwrap_or(i64::MAX);
    Some(if negative { -magnitude } else { magnitude })
}

/// Shift a time earlier by `offset` ms (later when negative), clamped to `0..=u64::MAX`.
fn apply_offset(t: u64, offset: i64) -> u64 {
    if offset >= 0 {
        t.saturating_sub(offset.unsigned_abs())
    } else {
        t.saturating_add(offset.unsigned_abs())
    }
}

/// One timed piece as `(start, end, text)`.
type Piece = (u64, Option<u64>, String);

/// One source line before the offset is applied.
struct RawLine {
    start: u64,
    text: String,
    words: Vec<Piece>,
    /// Some word is split into several timed pieces.
    split: bool,
}

/// Split a line body into segments: each is the text after a word tag; the first has no tag.
fn split_segments(body: &str) -> Vec<(Option<u64>, String)> {
    let mut segs: Vec<(Option<u64>, String)> = vec![(None, String::new())];
    let mut rest = body;
    while let Some(i) = rest.find('<') {
        let (head, tail) = rest.split_at(i);
        let tail = &tail[1..];
        if let Some(last) = segs.last_mut() {
            last.1.push_str(head);
        }
        if let Some((tag, after)) = tail.split_once('>')
            && let Some(t) = parse_timestamp(tag)
        {
            segs.push((Some(t), String::new()));
            rest = after;
        } else {
            if let Some(last) = segs.last_mut() {
                last.1.push('<');
            }
            rest = tail;
        }
    }
    if let Some(last) = segs.last_mut() {
        last.1.push_str(rest);
    }
    segs
}

/// Add a space to the end of the last piece, unless it has one.
fn space_after_last(words: &mut [Piece]) {
    if let Some(last) = words.last_mut()
        && !last.2.ends_with(' ')
    {
        last.2.push(' ');
    }
}

/// Split inline word tags out of a line body.
/// Returns the clean line text, the pieces as `(start, end, text)` and whether some word is
/// split into several pieces. A piece keeps one trailing space when whitespace follows it in
/// the source, so the pieces join without a separator; the last piece has none.
fn parse_body(body: &str, line_start: u64) -> (String, Vec<Piece>, bool) {
    let segs = split_segments(body);
    let joined: String = segs.iter().map(|s| s.1.as_str()).collect();
    let text = joined.split_whitespace().collect::<Vec<_>>().join(" ");
    if segs.len() == 1 {
        return (text, Vec::new(), false);
    }

    let mut words: Vec<Piece> = Vec::new();
    let mut split = false;
    let last_index = segs.len() - 1;
    for (i, (tag, seg)) in segs.into_iter().enumerate() {
        let core = seg.split_whitespace().collect::<Vec<_>>().join(" ");
        if core.is_empty() {
            // A trailing tag with no text after it closes the previous piece.
            if let Some(t) = tag
                && i == last_index
                && let Some(prev) = words.last_mut()
            {
                prev.1 = Some(t);
            }
            if !seg.is_empty() {
                space_after_last(&mut words);
            }
            continue;
        }
        if seg.starts_with(char::is_whitespace) {
            space_after_last(&mut words);
        }
        if words.last().is_some_and(|w| !w.2.ends_with(' ')) {
            split = true;
        }
        let piece = if seg.ends_with(char::is_whitespace) {
            format!("{core} ")
        } else {
            core
        };
        words.push((tag.unwrap_or(line_start), None, piece));
    }
    if let Some(last) = words.last_mut() {
        let t = last.2.trim_end().len();
        last.2.truncate(t);
    }
    (text, words, split)
}

/// Parse LRC (plain or enhanced) text into a [`LyricsDoc`].
///
/// Never fails and never panics: unparseable lines are skipped, huge numbers saturate.
/// Lines are sorted by start time; lines with equal times keep input order. Granularity is
/// [`Granularity::Syllable`] when a word is split into several tagged pieces
/// (`<00:01.00>Hel<00:01.40>lo`), [`Granularity::Word`] when lines have inline word tags,
/// otherwise [`Granularity::Line`]. Piece texts keep their trailing space; join them without a
/// separator. See the module docs for the offset sign.
#[must_use]
pub fn parse_lrc(text: &str, provider: ProviderId) -> LyricsDoc {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut offset: Option<i64> = None;
    let mut raw: Vec<RawLine> = Vec::new();

    for line in text.split('\n') {
        let mut rest = line.trim();
        let mut stamps: Vec<u64> = Vec::new();
        // Leading `[...]` tags: timestamps and `key:value` metadata. Anything else is text.
        while let Some(after) = rest.strip_prefix('[') {
            let Some((tag, next)) = after.split_once(']') else {
                break;
            };
            if let Some(t) = parse_timestamp(tag) {
                stamps.push(t);
            } else if tag.contains(':') {
                if let Some(o) = parse_offset_tag(tag) {
                    offset.get_or_insert(o);
                }
            } else {
                break;
            }
            rest = next;
        }
        for &start in &stamps {
            let (text, words, split) = parse_body(rest, start);
            raw.push(RawLine {
                start,
                text,
                words,
                split,
            });
        }
    }

    let offset = offset.unwrap_or(0);
    let syllable = raw.iter().any(|r| r.split);
    let mut lines: Vec<LyricLine> = raw
        .into_iter()
        .map(|r| LyricLine {
            start_ms: Some(apply_offset(r.start, offset)),
            end_ms: None,
            text: r.text,
            words: r
                .words
                .into_iter()
                .map(|(start, end, text)| LyricWord {
                    start_ms: apply_offset(start, offset),
                    end_ms: end.map(|e| apply_offset(e, offset)),
                    text,
                })
                .collect(),
        })
        .collect();
    // Stable sort: equal times keep input order.
    lines.sort_by_key(|l| l.start_ms);

    let granularity = if syllable {
        Granularity::Syllable
    } else if lines.iter().any(|l| !l.words.is_empty()) {
        Granularity::Word
    } else {
        Granularity::Line
    };
    LyricsDoc {
        lines,
        granularity,
        provider,
        matched: None,
        instrumental: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: ProviderId = ProviderId::new("test");

    type Row<'a> = (Option<u64>, &'a str);

    fn starts_and_texts(doc: &LyricsDoc) -> Vec<(Option<u64>, &str)> {
        doc.lines
            .iter()
            .map(|l| (l.start_ms, l.text.as_str()))
            .collect()
    }

    #[test]
    fn timestamp_formats() {
        let cases: &[(&str, Option<u64>)] = &[
            ("00:00.00", Some(0)),
            ("01:02.34", Some(62_340)),
            ("01:02.345", Some(62_345)),
            ("01:02.3", Some(62_300)),
            ("01:02", Some(62_000)),
            ("1:2", Some(62_000)),
            ("01:02.3456", Some(62_345)),
            ("ab:cd", None),
            ("01:02.", None),
            ("01", None),
            ("", None),
            ("-1:02", None),
        ];
        for (input, want) in cases {
            assert_eq!(parse_timestamp(input), *want, "input {input:?}");
        }
    }

    #[test]
    fn line_cases() {
        let cases: &[(&str, &[Row])] = &[
            (
                "[00:01.50]la la test line",
                &[(Some(1500), "la la test line")],
            ),
            ("[00:01.500]x", &[(Some(1500), "x")]),
            ("[00:01]x", &[(Some(1000), "x")]),
            (
                "[00:02.00][00:01.00]chorus test",
                &[(Some(1000), "chorus test"), (Some(2000), "chorus test")],
            ),
            ("\u{feff}[00:01.00]bom", &[(Some(1000), "bom")]),
            (
                "[00:01.00]one\r\n\r\n[00:02.00]two\r\n",
                &[(Some(1000), "one"), (Some(2000), "two")],
            ),
            ("[00:01.00]", &[(Some(1000), "")]),
            ("no tags here\n\n   \n", &[]),
            (
                "[00:01.00]keep [brackets] and <bad tag>",
                &[(Some(1000), "keep [brackets] and <bad tag>")],
            ),
            ("[00:01.00][chorus] text", &[(Some(1000), "[chorus] text")]),
            ("[00:01.00 broken", &[]),
            ("", &[]),
        ];
        for (input, want) in cases {
            let doc = parse_lrc(input, P);
            assert_eq!(starts_and_texts(&doc), *want, "input {input:?}");
            assert_eq!(doc.granularity, Granularity::Line, "input {input:?}");
            assert!(
                doc.lines
                    .iter()
                    .all(|l| l.end_ms.is_none() && l.words.is_empty())
            );
        }
    }

    #[test]
    fn sorts_and_keeps_order_for_equal_times() {
        let doc = parse_lrc(
            "[00:05.00]late\n[00:01.00]first b\n[00:01.00]first c\n[00:01.00]first d\n[00:03.00]mid",
            P,
        );
        let texts: Vec<&str> = doc.lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["first b", "first c", "first d", "mid", "late"]);
    }

    #[test]
    fn metadata_is_ignored() {
        let src = "[ar:Test Artist]\n[ti:Test Title]\n[al:Test Album]\n[by:me]\n[length:03:00]\n[xx:unknown]\n[00:01.00]la";
        let doc = parse_lrc(src, P);
        assert_eq!(starts_and_texts(&doc), [(Some(1000), "la")]);
    }

    #[test]
    fn offset_sign_convention() {
        // Positive offset: lyrics appear earlier (time = stamp - offset).
        let cases: &[(&str, u64)] = &[
            ("[offset:+500]\n[00:02.00]x", 1500),
            ("[offset:500]\n[00:02.00]x", 1500),
            ("[offset:-500]\n[00:02.00]x", 2500),
            ("[offset: -500 ]\n[00:02.00]x", 2500),
            ("[00:02.00]x\n[offset:+500]", 1500),
            ("[offset:+5000]\n[00:02.00]x", 0),
            ("[offset:junk]\n[00:02.00]x", 2000),
            ("[offset:+100]\n[offset:+900]\n[00:02.00]x", 1900),
            ("[OFFSET:+500][00:02.00]x", 1500),
        ];
        for (input, want) in cases {
            let doc = parse_lrc(input, P);
            assert_eq!(
                doc.lines.first().and_then(|l| l.start_ms),
                Some(*want),
                "input {input:?}"
            );
        }
    }

    #[test]
    fn offset_applies_to_words_and_keeps_sorted() {
        let doc = parse_lrc(
            "[offset:+1000]\n[00:03.00]<00:03.00>a <00:03.50>b <00:04.00>",
            P,
        );
        let line = &doc.lines[0];
        assert_eq!(line.start_ms, Some(2000));
        assert_eq!(line.words[0].start_ms, 2000);
        assert_eq!(line.words[1].start_ms, 2500);
        assert_eq!(line.words[1].end_ms, Some(3000));
    }

    #[test]
    fn enhanced_word_tags() {
        let doc = parse_lrc(
            "[00:01.00]<00:01.00>la <00:01.40>la <00:01.80>test\n[00:03.00]plain line",
            P,
        );
        assert_eq!(doc.granularity, Granularity::Word);
        let l0 = &doc.lines[0];
        assert_eq!(l0.text, "la la test");
        let words: Vec<(u64, &str, Option<u64>)> = l0
            .words
            .iter()
            .map(|w| (w.start_ms, w.text.as_str(), w.end_ms))
            .collect();
        assert_eq!(
            words,
            [
                (1000, "la ", None),
                (1400, "la ", None),
                (1800, "test", None)
            ]
        );
        assert_eq!(doc.lines[1].words.len(), 0);
        assert_eq!(doc.lines[1].text, "plain line");
    }

    #[test]
    fn pieces_of_one_word_make_it_syllable_synced() {
        let doc = parse_lrc("[00:01.00]<00:01.00>Hel<00:01.30>lo <00:01.80>you", P);
        assert_eq!(doc.granularity, Granularity::Syllable);
        let l = &doc.lines[0];
        let texts: Vec<&str> = l.words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, ["Hel", "lo ", "you"]);
        assert_eq!(texts.concat(), l.text);
        // A space before the next tag also ends a word.
        let word = parse_lrc("[00:01.00]<00:01.00>Hello<00:01.80> you", P);
        assert_eq!(word.granularity, Granularity::Word);
        assert_eq!(word.lines[0].words[0].text, "Hello ");
    }

    #[test]
    fn enhanced_trailing_end_tag_sets_only_last_word_end() {
        let doc = parse_lrc("[00:01.00]<00:01.00>la <00:01.40>la <00:02.00>", P);
        let l = &doc.lines[0];
        assert_eq!(l.words.len(), 2);
        assert_eq!(l.words[0].end_ms, None);
        assert_eq!(l.words[1].end_ms, Some(2000));
        assert_eq!(l.end_ms, None);
    }

    #[test]
    fn lines_without_inline_tags_stay_line_granularity() {
        let doc = parse_lrc("[00:01.00]a <not a tag> b", P);
        assert_eq!(doc.granularity, Granularity::Line);
        assert_eq!(doc.lines[0].text, "a <not a tag> b");
    }

    #[test]
    fn huge_numbers_do_not_overflow() {
        let big = "9".repeat(40);
        let src = format!(
            "[offset:-{big}]\n[{big}:{big}.999]x\n[00:01.00]<{big}:00>w <{big}:{big}>\n[offset:+{big}]"
        );
        let doc = parse_lrc(&src, P);
        assert_eq!(doc.lines.len(), 2);
        assert_eq!(doc.lines[1].start_ms, Some(u64::MAX));
    }

    #[test]
    fn non_ascii_text_is_kept() {
        let doc = parse_lrc("[00:01.00]ラ ラ テスト <00:02.00>é", P);
        assert_eq!(doc.lines[0].text, "ラ ラ テスト é");
    }
}
