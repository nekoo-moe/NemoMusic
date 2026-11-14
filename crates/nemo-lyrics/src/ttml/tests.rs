//! Unit tests for the TTML parser.

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
    let src = "<tt><body><div><p begin=\"1\"><![CDATA[a < b]]> &#65;&#x42;</p></div></body></tt>";
    assert_eq!(parse_ttml(src, P).lines[0].text, "a < b AB");
}
