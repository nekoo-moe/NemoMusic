//! Plain (unsynced) lyrics: USLT, `plainLyrics`.

use nemo_core::lyrics::{Granularity, LyricLine, LyricsDoc, ProviderId};

/// Parse unsynced text into a [`LyricsDoc`].
///
/// Every line has `start_ms == None`. Timing is never guessed. A leading BOM, CRLF line ends and
/// leading or trailing blank lines are dropped; blank lines inside are kept as stanza breaks.
#[must_use]
pub fn parse_plain(text: &str, provider: ProviderId) -> LyricsDoc {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines: Vec<LyricLine> = text
        .split('\n')
        .map(|l| LyricLine {
            start_ms: None,
            end_ms: None,
            text: l.trim().to_owned(),
            words: Vec::new(),
        })
        .collect();
    while lines.last().is_some_and(|l| l.text.is_empty()) {
        lines.pop();
    }
    let skip = lines.iter().take_while(|l| l.text.is_empty()).count();
    lines.drain(..skip);
    LyricsDoc {
        lines,
        granularity: Granularity::Unsynced,
        provider,
        matched: None,
        instrumental: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: ProviderId = ProviderId::new("test");

    #[test]
    fn plain_has_no_timing() {
        let doc = parse_plain("\u{feff}\r\nla la test line\r\n\r\nsecond line\r\n\r\n", P);
        assert_eq!(doc.granularity, Granularity::Unsynced);
        let texts: Vec<&str> = doc.lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["la la test line", "", "second line"]);
        assert!(
            doc.lines
                .iter()
                .all(|l| l.start_ms.is_none() && l.end_ms.is_none())
        );
    }

    #[test]
    fn empty_input_gives_no_lines() {
        assert_eq!(parse_plain("  \n\n", P).lines.len(), 0);
    }
}
