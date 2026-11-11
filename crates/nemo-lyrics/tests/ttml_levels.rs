//! Timing level of TTML documents. All fixtures are synthetic; no real lyrics.

use nemo_core::lyrics::{Granularity, ProviderId};
use nemo_lyrics::{parse_ttml, parse_ttml_detailed};

const P: ProviderId = ProviderId::new("test");

fn doc(body: &str) -> String {
    format!("<tt><body><div>{body}</div></body></tt>")
}

fn pieces(src: &str, line: usize) -> Vec<String> {
    parse_ttml(src, P).lines[line]
        .words
        .iter()
        .map(|w| w.text.clone())
        .collect()
}

#[test]
fn split_word_is_syllable_and_keeps_piece_whitespace() {
    let src = doc(concat!(
        r#"<p begin="1.0" end="3.0"><span begin="1.0" end="1.3">Sun</span>"#,
        r#"<span begin="1.3" end="1.6">shine </span>"#,
        r#"<span begin="1.6" end="2.0">on</span> <span begin="2.0" end="3.0">me</span></p>"#,
    ));
    let d = parse_ttml(&src, P);
    assert_eq!(d.granularity, Granularity::Syllable);
    assert_eq!(pieces(&src, 0), ["Sun", "shine ", "on ", "me"]);
    assert_eq!(pieces(&src, 0).concat(), d.lines[0].text);
    assert_eq!(d.lines[0].text, "Sunshine on me");
}

#[test]
fn whole_words_are_word_synced() {
    let src = doc(concat!(
        r#"<p begin="1.0" end="3.0"><span begin="1.0" end="2.0">one </span>"#,
        r#"<span begin="2.0" end="2.5">two</span> <span begin="2.5" end="3.0">three</span></p>"#,
    ));
    assert_eq!(parse_ttml(&src, P).granularity, Granularity::Word);
    assert_eq!(pieces(&src, 0), ["one ", "two ", "three"]);
}

#[test]
fn one_timed_span_per_line_is_word_synced() {
    let src = doc(r#"<p begin="1.0" end="2.0"><span begin="1.0" end="2.0">alone</span></p>"#);
    assert_eq!(parse_ttml(&src, P).granularity, Granularity::Word);
}

#[test]
fn layout_line_break_between_spans_is_not_a_split() {
    let src = "<tt><body><div><p begin=\"1.0\" end=\"3.0\">\n  <span begin=\"1.0\" end=\"2.0\">one</span>\n  <span begin=\"2.0\" end=\"3.0\">two</span>\n</p></div></body></tt>";
    assert_eq!(parse_ttml(src, P).granularity, Granularity::Word);
}

#[test]
fn line_and_unsynced_documents() {
    let line = doc(r#"<p begin="1.0" end="2.0">a line</p><p begin="2.0">next</p>"#);
    assert_eq!(parse_ttml(&line, P).granularity, Granularity::Line);
    let plain = doc("<p>a line</p><p>next</p>");
    assert_eq!(parse_ttml(&plain, P).granularity, Granularity::Unsynced);
}

#[test]
fn background_vocal_syllables_count_and_stay_on_their_own_line() {
    let src = doc(concat!(
        r#"<p begin="1.0" end="3.0"><span begin="1.0" end="2.0">Lead </span>"#,
        r#"<span begin="2.0" end="2.5">words</span>"#,
        r#"<span ttm:role="x-bg"><span begin="2.5" end="2.7">(oo</span>"#,
        r#"<span begin="2.7" end="3.0">oh)</span></span></p>"#,
    ));
    let t = parse_ttml_detailed(&src, P);
    assert_eq!(t.doc.granularity, Granularity::Syllable);
    let texts: Vec<&str> = t.doc.lines.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(texts, ["Lead words", "(oooh)"]);
    assert!(t.meta[1].background);
    assert_eq!(pieces(&src, 1), ["(oo", "oh)"]);
}

#[test]
fn main_line_end_and_background_start_are_not_one_word() {
    // The last main piece and the first background piece are on different lines.
    let src = doc(concat!(
        r#"<p begin="1.0" end="3.0"><span begin="1.0" end="2.0">Lead</span>"#,
        r#"<span ttm:role="x-bg"><span begin="2.0" end="3.0">(ooh)</span></span></p>"#,
    ));
    assert_eq!(parse_ttml(&src, P).granularity, Granularity::Word);
}
