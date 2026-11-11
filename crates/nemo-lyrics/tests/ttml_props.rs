//! Property tests for the TTML parser. All inputs are generated; no real lyrics.

use std::fmt::Write as _;

use nemo_core::lyrics::{Granularity, ProviderId};
use nemo_lyrics::{parse_ttml, parse_ttml_detailed};
use proptest::prelude::*;

const P: ProviderId = ProviderId::new("test");

/// Pieces that make a string look like TTML, so the fuzzer reaches the tag handling.
fn ttml_soup() -> impl Strategy<Value = String> {
    let piece = prop_oneof![
        Just("<tt>".to_owned()),
        Just("</tt>".to_owned()),
        Just("<p begin=\"00:01.5\" end=\"2s\" ttm:agent=\"v1\">".to_owned()),
        Just("</p>".to_owned()),
        Just("<span begin=\"1.0\" end=\"2.0\">".to_owned()),
        Just("<span ttm:role=\"x-bg\">".to_owned()),
        Just("<span ttm:role=\"x-translation\">".to_owned()),
        Just("</span>".to_owned()),
        Just("<br/>".to_owned()),
        Just("<!-- c -->".to_owned()),
        Just("<![CDATA[x]]>".to_owned()),
        Just("&amp;".to_owned()),
        Just("&#x1F600;".to_owned()),
        Just("<".to_owned()),
        Just(">".to_owned()),
        Just("\"".to_owned()),
        Just("é 歌 ".to_owned()),
        "[a-z ]{0,6}",
    ];
    prop::collection::vec(piece, 0..40).prop_map(|v| v.concat())
}

/// A well-formed document: lines of syllables with increasing times.
fn clock(ms: u64) -> String {
    format!(
        "{:02}:{:02}.{:03}",
        ms / 60_000,
        (ms / 1000) % 60,
        ms % 1000
    )
}

proptest! {
    #[test]
    fn never_panics_on_any_string(s in any::<String>()) {
        let _ = parse_ttml(&s, P);
        let _ = parse_ttml_detailed(&s, P);
    }

    #[test]
    fn never_panics_on_ttml_soup(s in ttml_soup()) {
        let t = parse_ttml_detailed(&s, P);
        prop_assert_eq!(t.doc.lines.len(), t.meta.len());
        for line in &t.doc.lines {
            prop_assert!(!line.text.is_empty());
            if t.doc.granularity == Granularity::Unsynced {
                prop_assert!(line.start_ms.is_none() && line.words.is_empty());
            }
        }
        // Synced documents are in time order.
        let starts: Vec<u64> = t.doc.lines.iter().filter_map(|l| l.start_ms).collect();
        if starts.len() == t.doc.lines.len() {
            prop_assert!(starts.windows(2).all(|w| w[0] <= w[1]));
        }
    }

    #[test]
    fn well_formed_documents_round_trip(
        lines in prop::collection::vec(
            prop::collection::vec(("[a-z]{1,6}", 100u64..900), 1..6),
            1..12,
        )
    ) {
        let mut xml = String::from("<tt><body><div>");
        let mut t = 1000u64;
        let mut expected_words = 0usize;
        for line in &lines {
            let line_start = t;
            let mut spans = String::new();
            for (i, (word, len)) in line.iter().enumerate() {
                // A trailing space on every word but the last, like Apple's layout.
                let space = if i + 1 < line.len() { " " } else { "" };
                let _ = write!(
                    spans,
                    "<span begin=\"{}\" end=\"{}\">{word}{space}</span>",
                    clock(t),
                    clock(t + len)
                );
                t += len;
                expected_words += 1;
            }
            let _ = write!(
                xml,
                "<p begin=\"{}\" end=\"{}\">{spans}</p>",
                clock(line_start),
                clock(t)
            );
            t += 200;
        }
        xml.push_str("</div></body></tt>");

        let doc = parse_ttml(&xml, P);
        prop_assert_eq!(doc.granularity, Granularity::Word);
        prop_assert_eq!(doc.lines.len(), lines.len());
        prop_assert_eq!(doc.lines.iter().map(|l| l.words.len()).sum::<usize>(), expected_words);
        for (parsed, source) in doc.lines.iter().zip(&lines) {
            let joined: Vec<&str> = source.iter().map(|(w, _)| w.as_str()).collect();
            prop_assert_eq!(&parsed.text, &joined.join(" "));
            // Word starts never go backwards and stay inside the line.
            let starts: Vec<u64> = parsed.words.iter().map(|w| w.start_ms).collect();
            prop_assert!(starts.windows(2).all(|w| w[0] <= w[1]));
            prop_assert_eq!(parsed.start_ms, starts.first().copied());
            prop_assert!(parsed.end_ms >= parsed.words.last().and_then(|w| w.end_ms));
        }
    }
}
