//! Property tests for the LRC parser and normalizer.

use std::fmt::Write as _;

use nemo_core::lyrics::ProviderId;
use nemo_lyrics::{normalize, parse_lrc, parse_plain};
use proptest::prelude::*;

const P: ProviderId = ProviderId::new("test");

fn assert_sorted(text: &str) {
    let doc = parse_lrc(text, P);
    let starts: Vec<u64> = doc.lines.iter().filter_map(|l| l.start_ms).collect();
    assert!(
        starts.windows(2).all(|w| w[0] <= w[1]),
        "not sorted: {starts:?}"
    );
}

proptest! {
    #[test]
    fn lrc_never_panics_on_any_string(s in any::<String>()) {
        let _ = parse_lrc(&s, P);
        let _ = parse_plain(&s, P);
        let _ = normalize(&s);
    }

    #[test]
    fn lrc_never_panics_and_is_sorted_on_tag_soup(s in "[\\[\\]<>:.0-9a-z+\\- \r\n]{0,300}") {
        assert_sorted(&s);
    }

    #[test]
    fn lrc_starts_non_decreasing_on_any_string(s in any::<String>()) {
        assert_sorted(&s);
    }

    #[test]
    fn lrc_starts_non_decreasing_for_generated_lines(
        lines in proptest::collection::vec((0u64..10_000_000_000_000, 0u32..100, "[a-z ]{0,12}"), 0..30),
        offset in -100_000i64..100_000,
    ) {
        let mut src = format!("[offset:{offset}]\n");
        for (ms, cs, text) in &lines {
            let secs = ms / 1000;
            let _ = writeln!(src, "[{:02}:{:02}.{:02}]{text}", secs / 60, secs % 60, cs);
        }
        assert_sorted(&src);
        prop_assert_eq!(parse_lrc(&src, P).lines.len(), lines.len());
    }

    #[test]
    fn normalize_is_idempotent(s in any::<String>()) {
        let once = normalize(&s);
        prop_assert_eq!(normalize(&once), once);
    }
}
