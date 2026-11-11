//! Choose the best lyrics candidate (ADR 0002).

use nemo_core::lyrics::LyricsDoc;

/// One accepted result from one provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The lyrics.
    pub doc: LyricsDoc,
    /// Match score, 0 to 100.
    pub score: u8,
    /// Position of the provider in the chain; lower is earlier.
    pub chain_order: usize,
}

/// Pick the best candidate: best granularity (Syllable > Word > Line > Unsynced), then higher
/// score, then lower chain order. If all three tie, the earliest in `candidates` wins. Pure and
/// deterministic. `None` for an empty slice.
#[must_use]
pub fn pick_best(candidates: &[Candidate]) -> Option<&Candidate> {
    candidates.iter().reduce(|best, c| {
        let key = |x: &Candidate| (x.doc.granularity, x.score, std::cmp::Reverse(x.chain_order));
        if key(c) > key(best) { c } else { best }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nemo_core::lyrics::{Granularity, ProviderId};

    fn cand(g: Granularity, score: u8, order: usize, name: &'static str) -> Candidate {
        Candidate {
            doc: LyricsDoc {
                lines: vec![],
                granularity: g,
                provider: ProviderId::new(name),
                matched: None,
                instrumental: false,
            },
            score,
            chain_order: order,
        }
    }

    fn name(c: Option<&Candidate>) -> &'static str {
        c.expect("some").doc.provider.as_str()
    }

    #[test]
    fn empty_is_none() {
        assert!(pick_best(&[]).is_none());
    }

    #[test]
    fn granularity_beats_score() {
        let c = [
            cand(Granularity::Line, 100, 0, "line"),
            cand(Granularity::Word, 70, 2, "word"),
            cand(Granularity::Unsynced, 100, 1, "plain"),
        ];
        assert_eq!(name(pick_best(&c)), "word");
    }

    #[test]
    fn syllable_beats_word_line_and_score() {
        let c = [
            cand(Granularity::Word, 100, 0, "word"),
            cand(Granularity::Line, 100, 1, "line"),
            cand(Granularity::Syllable, 70, 3, "syllable"),
            cand(Granularity::Unsynced, 100, 2, "plain"),
        ];
        assert_eq!(name(pick_best(&c)), "syllable");
        let tie = [
            cand(Granularity::Syllable, 80, 4, "late"),
            cand(Granularity::Syllable, 90, 5, "better"),
            cand(Granularity::Syllable, 90, 1, "early"),
        ];
        assert_eq!(name(pick_best(&tie)), "early");
    }

    #[test]
    fn score_beats_chain_order() {
        let c = [
            cand(Granularity::Line, 80, 0, "a"),
            cand(Granularity::Line, 90, 5, "b"),
        ];
        assert_eq!(name(pick_best(&c)), "b");
    }

    #[test]
    fn lower_chain_order_breaks_tie_in_any_input_order() {
        let a = cand(Granularity::Line, 90, 1, "a");
        let b = cand(Granularity::Line, 90, 2, "b");
        assert_eq!(name(pick_best(&[a.clone(), b.clone()])), "a");
        assert_eq!(name(pick_best(&[b, a])), "a");
    }

    #[test]
    fn full_tie_takes_first() {
        let c = [
            cand(Granularity::Line, 90, 1, "first"),
            cand(Granularity::Line, 90, 1, "second"),
        ];
        assert_eq!(name(pick_best(&c)), "first");
    }
}
