use super::*;
use nemo_core::lyrics::LyricWord;

fn line(text: &str, words: &[(&str, u64, Option<u64>)]) -> LyricLine {
    LyricLine {
        start_ms: words.first().map(|w| w.1),
        end_ms: Some(5_000),
        text: text.to_owned(),
        words: words
            .iter()
            .map(|(t, s, e)| LyricWord {
                start_ms: *s,
                end_ms: *e,
                text: (*t).to_owned(),
            })
            .collect(),
    }
}

fn tm(start: i64, end: i64) -> Timing {
    Timing {
        start_ms: start,
        end_ms: end,
    }
}

#[test]
fn syllables_of_one_word_stay_joined_and_cover_the_line() {
    let l = line(
        "beau-tiful day",
        &[
            ("beau", 1000, None),
            ("-ti", 1200, None),
            ("ful ", 1400, None),
            ("day", 1800, None),
        ],
    );
    let t = texts(&l);
    assert_eq!(t, ["beau", "-ti", "ful ", "day"]);
    assert_eq!(t.concat(), l.text);
}

#[test]
fn word_texts_without_spaces_keep_the_line_spaces() {
    let l = line(
        "la  la, la",
        &[("la", 0, None), ("la,", 500, None), ("la", 900, None)],
    );
    assert_eq!(texts(&l).concat(), "la  la, la");
    // Texts that do not match fall back to the pieces themselves.
    let odd = line("other text", &[("x ", 0, None), ("y", 500, None)]);
    assert_eq!(texts(&odd), ["x ", "y"]);
}

#[test]
fn timings_use_the_next_start_and_a_minimum_length() {
    let l = line(
        "a b c",
        &[("a ", 0, None), ("b ", 1000, Some(1030)), ("c", 2000, None)],
    );
    let t = timings(&l);
    assert_eq!(t[0], tm(0, 1000));
    assert_eq!(t[1], tm(1000, 1100), "a 30 ms word still takes 100 ms");
    assert_eq!(t[2], tm(2000, 5000), "the last one runs to the line end");
    assert_eq!(t[2].len_ms(), 3000);
}

#[test]
fn an_unsung_word_and_an_unlit_line_share_one_alpha() {
    assert!((UNSUNG_ALPHA - crate::lyrics_view::metrics::INACTIVE_ALPHA).abs() < 1e-6);
    assert!((UNSUNG_ALPHA - 0.38).abs() < 1e-6);
}

#[test]
fn rtl_text_is_found_by_its_first_letter() {
    assert!(is_rtl("שלום עולם"));
    assert!(is_rtl("  مرحبا"));
    assert!(!is_rtl("hello שלום"));
    assert!(!is_rtl("Tiếng Việt 你好"));
}
