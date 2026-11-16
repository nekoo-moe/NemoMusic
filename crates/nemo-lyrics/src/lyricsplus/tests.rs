use nemo_core::lyrics::{Granularity, LyricsCredit, LyricsDoc, LyricsQuery, ProviderId};
use nemo_core::lyrics_plus::LineMeta;

use super::parse::MAX_NAME_CHARS;
use super::*;
use crate::pick::{Candidate, pick_best};
use crate::score::score_match;

/// Recorded answer, cut down to three lines with invented text (see `tests/fixtures/README.md`).
const RECORDED: &str = include_str!("../../tests/fixtures/lyricsplus-v2-word.json");
const MISS_SKIPPED: &str = include_str!("../../tests/fixtures/lyricsplus-404-skipped.json");
const MISS_ALL_BAD: &str = include_str!("../../tests/fixtures/lyricsplus-404-miss.json");

fn query() -> LyricsQuery {
    LyricsQuery {
        title: "Test Song".into(),
        artists: vec!["Test Artist".into()],
        album: Some("Test Album - Single".into()),
        duration_ms: Some(301_000),
        ytm_video_id: None,
        local_path: None,
    }
}

fn parse(body: &str) -> (LyricsDoc, LyricsCredit, Vec<LineMeta>) {
    let (doc, credit, meta) = parse_response(body).expect("parses").expect("lyrics");
    (doc, credit.expect("credit"), meta)
}

/// A one-off answer in the documented shape. Not real lyrics.
fn answer(kind: &str, lines: &serde_json::Value) -> String {
    serde_json::json!({ "type": kind, "metadata": { "source": "Apple" }, "lyrics": lines })
        .to_string()
}

#[test]
fn recorded_answer_keeps_its_timing_background_and_match() {
    let (doc, credit, meta) = parse(RECORDED);
    // Vietnamese writes one syllable per word, so Apple's word timing is whole words here.
    assert_eq!(doc.granularity, Granularity::Word);
    assert_eq!(doc.provider, LYRICSPLUS_ID);
    let texts: Vec<&str> = doc.lines.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "Sáng nay trời trong xanh lắm nhé",
            "Mình đi dạo quanh hồ",
            "Một hai ba bốn năm sáu bảy",
            "(tám chín mười một hai)",
        ]
    );
    let words: Vec<usize> = doc.lines.iter().map(|l| l.words.len()).collect();
    assert_eq!(words, [7, 5, 7, 5]);
    let first = &doc.lines[0];
    assert_eq!((first.start_ms, first.end_ms), (Some(25_865), Some(29_660)));
    assert_eq!(first.words[0].text, "Sáng ");
    assert_eq!(
        (first.words[6].start_ms, first.words[6].end_ms),
        (28_243, Some(29_660))
    );
    let bg = &doc.lines[3];
    assert_eq!((bg.start_ms, bg.end_ms), (Some(198_206), Some(199_844)));
    let flags: Vec<bool> = meta.iter().map(|m| m.background).collect();
    assert_eq!(flags, [false, false, false, true]);
    assert!(meta.iter().all(|m| m.singer == Some(0)));

    let m = doc.matched.expect("matched");
    assert_eq!(m.title, "Test Song");
    assert_eq!(m.artists, ["Test Artist"]);
    assert_eq!(m.album.as_deref(), Some("Test Album - Single"));
    assert_eq!(m.duration_ms, Some(301_538));

    assert_eq!(credit.label, "Lyrics via LyricsPlus, from Apple Music");
    assert_eq!(credit.url.as_deref(), Some(CREDIT_URL));
    assert_eq!(credit.note.as_deref(), Some("source: Apple Music"));
    assert_eq!(credit.by, None);
}

#[test]
fn split_words_make_a_syllable_doc() {
    let lines = serde_json::json!([{
        "time": 1000, "duration": 1000, "text": "Hello there",
        "syllabus": [
            { "time": 1000, "duration": 200, "text": "Hel" },
            { "time": 1200, "duration": 300, "text": "lo " },
            { "time": 1500, "duration": 500, "text": "there" }
        ],
        "element": { "singer": "v1" }
    }]);
    let (doc, ..) = parse(&answer("Word", &lines));
    assert_eq!(doc.granularity, Granularity::Syllable);
    let joined: String = doc.lines[0].words.iter().map(|w| w.text.as_str()).collect();
    assert_eq!(joined, "Hello there");
}

#[test]
fn spread_timing_is_never_shown_as_word_timing() {
    let lines = serde_json::json!([
        { "time": 1000, "duration": 1000, "text": "real words",
          "syllabus": [
              { "time": 1000, "duration": 500, "text": "real " },
              { "time": 1500, "duration": 500, "text": "words" }
          ] },
        { "time": 3000, "duration": 1000, "text": "spread words",
          "syllabus": [
              { "time": 3000, "duration": 500, "text": "spread ", "synthetic": true },
              { "time": 3500, "duration": 500, "text": "words" }
          ] }
    ]);
    let (doc, ..) = parse(&answer("Word", &lines));
    assert_eq!(doc.granularity, Granularity::Word);
    assert_eq!(doc.lines[0].words.len(), 2);
    let spread = &doc.lines[1];
    assert!(spread.words.is_empty());
    assert_eq!(spread.text, "spread words");
    assert_eq!((spread.start_ms, spread.end_ms), (Some(3000), Some(4000)));
}

#[test]
fn labels_never_add_timing() {
    let lines = serde_json::json!([{
        "time": 1000, "duration": 1000, "text": "one line",
        "syllabus": [{ "time": 1000, "duration": 1000, "text": "one line" }]
    }]);
    let (line, ..) = parse(&answer("Line", &lines));
    assert_eq!(line.granularity, Granularity::Line);
    assert!(line.lines[0].words.is_empty());
    assert_eq!(line.lines[0].start_ms, Some(1000));
    let (plain, ..) = parse(&answer("None", &lines));
    assert_eq!(plain.granularity, Granularity::Unsynced);
    assert_eq!(plain.lines[0].start_ms, None);
    assert!(plain.lines[0].words.is_empty());
}

#[test]
fn singers_romanization_and_background_lines() {
    let lines = serde_json::json!([
        { "time": 1000, "duration": 500, "text": "first",
          "syllabus": [{ "time": 1000, "duration": 500, "text": "first" }],
          "element": { "singer": "v2" },
          "transliteration": { "lang": "ko-Latn", "text": " cheot " } },
        { "time": 2000, "duration": 500, "text": "second",
          "syllabus": [{ "time": 2000, "duration": 500, "text": "second" }],
          "element": { "singer": "v1" } },
        { "time": 3000, "duration": 500, "text": "(echo)",
          "syllabus": [{ "time": 3000, "duration": 500, "text": "(echo)" }],
          "element": { "singer": "v2", "isBackground": true } }
    ]);
    let (doc, credit, meta) = parse(&answer("Word", &lines));
    assert_eq!(doc.lines.len(), 3);
    let singers: Vec<Option<u8>> = meta.iter().map(|m| m.singer).collect();
    assert_eq!(singers, [Some(0), Some(1), Some(0)]);
    assert_eq!(meta[0].roman.as_deref(), Some("cheot"));
    assert!(meta[2].background && !meta[0].background);
    // No title anywhere: the doc does not claim a match.
    assert!(doc.matched.is_none());
    assert_eq!(credit.label, "Lyrics via LyricsPlus, from Apple Music");
}

#[test]
fn empty_and_broken_answers() {
    assert_eq!(
        parse_response(&answer("Word", &serde_json::json!([]))),
        Ok(None)
    );
    let blank = serde_json::json!([{ "time": 1, "duration": 1, "text": "  " }]);
    assert_eq!(parse_response(&answer("Line", &blank)), Ok(None));
    assert!(parse_response("not json").is_err());
}

#[test]
fn a_miss_is_temporary_only_when_a_source_did_not_answer() {
    assert!(miss_is_temporary(MISS_SKIPPED));
    assert!(!miss_is_temporary(MISS_ALL_BAD));
    assert!(!miss_is_temporary(r#"{"error":"Lyrics not found"}"#));
    assert!(!miss_is_temporary("<html>busy</html>"));
}

#[test]
fn query_sends_known_fields_and_the_fixed_source_list() {
    let p = query_params(&query()).expect("params");
    let got: Vec<(&str, &str)> = p.iter().map(|(k, v)| (*k, v.as_str())).collect();
    assert_eq!(
        got,
        [
            ("title", "Test Song"),
            ("artist", "Test Artist"),
            ("album", "Test Album - Single"),
            ("duration", "301"),
            ("source", "apple,lyricsplus,deezer,qq"),
        ]
    );
    let mut bare = query();
    bare.album = Some("  ".into());
    bare.duration_ms = None;
    let keys: Vec<&str> = query_params(&bare)
        .expect("params")
        .iter()
        .map(|(k, _)| *k)
        .collect();
    assert_eq!(keys, ["title", "artist", "source"]);
    let mut no_artist = query();
    no_artist.artists.clear();
    assert!(query_params(&no_artist).is_none());
}

#[test]
fn a_good_syllable_match_beats_a_line_answer_and_a_bad_one_is_rejected() {
    let (doc, ..) = parse(RECORDED);
    let score = score_match(&query(), doc.matched.as_ref().expect("matched")).expect("scored");
    assert!(score.is_acceptable());
    let mut lrclib = doc.clone();
    lrclib.granularity = Granularity::Line;
    lrclib.provider = ProviderId::new("LRCLIB");
    let candidates = [
        Candidate {
            doc: lrclib,
            score: 100,
            chain_order: 1,
        },
        Candidate {
            doc,
            score: score.total,
            chain_order: 3,
        },
    ];
    let best = pick_best(&candidates).expect("best");
    assert_eq!(best.doc.provider, LYRICSPLUS_ID);

    let mut other = query();
    other.title = "Completely Different".into();
    other.artists = vec!["Someone Else".into()];
    let (doc, ..) = parse(RECORDED);
    let bad = score_match(&other, doc.matched.as_ref().expect("matched"));
    assert!(!bad.is_some_and(|s| s.is_acceptable()));
}

#[test]
fn upstream_names_and_curator_credit() {
    assert_eq!(upstream_name("apple").as_deref(), Some("Apple Music"));
    assert_eq!(upstream_name("QQ").as_deref(), Some("QQ Music"));
    assert_eq!(
        upstream_name("Apple with QQ").as_deref(),
        Some("Apple Music with QQ Music timing")
    );
    assert_eq!(
        upstream_name("lyricsplus").as_deref(),
        Some("LyricsPlus community")
    );
    assert_eq!(upstream_name(" "), None);
    let long = "x".repeat(100);
    assert_eq!(upstream_name(&long).map(|s| s.len()), Some(MAX_NAME_CHARS));
    let c = credit(None, Some(" maker "));
    assert_eq!(c.label, "Lyrics via LyricsPlus");
    assert_eq!(c.by.as_deref(), Some("maker"));
    assert_eq!(c.note, None);
}

#[test]
fn provider_is_opt_in_syllable_with_one_host() {
    let p = LyricsPlusProvider::new().expect("client");
    assert!(!p.info().default_enabled);
    assert!(p.info().risk_note.is_some_and(|r| r.contains("Unofficial")));
    assert_eq!(p.best_granularity(), Granularity::Syllable);
    assert_eq!(p.hosts(), ["lyricsplus.prjktla.my.id"]);
    assert_eq!(p.id(), LYRICSPLUS_ID);
}
