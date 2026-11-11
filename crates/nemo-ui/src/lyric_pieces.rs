//! Per-piece highlight of word- and syllable-timed lyrics, Better Lyrics style (wave-lyrics
//! criterion 8, an owner extension of `design.md` section 10). Pure: it splits the active line
//! into its timed pieces and gives each one an ink opacity for the playback position. Pieces
//! already sung are `on-surface` at [`SUNG_ALPHA`]; the piece being sung fades from the
//! "coming" opacity to full over its own duration (at least [`MIN_FADE_MS`]); pieces still to
//! come stay at the "coming" opacity. No bounce and no hard-edged sweep: the whole piece fades.

use nemo_core::lyrics::LyricLine;

pub use crate::canvas::SUNG_ALPHA;

/// The shortest fade of one piece, ms.
pub const MIN_FADE_MS: u64 = 100;
/// Fade length of the last piece when the provider gives no end time, ms.
const LAST_PIECE_MS: u64 = 600;

/// One timed piece of a line and how bright it is now.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    /// The text, including any spaces and punctuation that follow it in the line. Pieces join
    /// without a separator, so pieces of one word stay joined.
    pub text: String,
    /// Opacity of `on-surface` for this piece, from the "coming" opacity to 1.0.
    pub alpha: f32,
}

/// The texts of the timed pieces of `line`, covering the whole line text. Each piece is found in
/// `line.text` in order and runs to the start of the next one, so the spaces between words stay
/// exactly as the provider wrote them. When a piece cannot be found (the provider's word texts
/// differ from the line text) the word texts are used as they are, joined without a separator.
#[must_use]
pub fn texts(line: &LyricLine) -> Vec<String> {
    let text = line.text.as_str();
    let mut starts = Vec::with_capacity(line.words.len());
    let mut cursor = 0;
    for w in &line.words {
        let needle = w.text.trim();
        let found = if needle.is_empty() {
            Some(cursor)
        } else {
            text.get(cursor..)
                .and_then(|rest| rest.find(needle))
                .map(|p| cursor + p)
        };
        let Some(at) = found else {
            return line.words.iter().map(|w| w.text.clone()).collect();
        };
        starts.push(at);
        cursor = at + needle.len();
    }
    if let Some(first) = starts.first_mut() {
        // Leading spaces belong to the first piece.
        *first = 0;
    }
    let mut out = Vec::with_capacity(starts.len());
    for (i, s) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(text.len());
        out.push(text.get(*s..end).unwrap_or_default().to_owned());
    }
    out
}

/// The pieces of `line` at `t_ms` (position plus the lyrics offset). `coming` is the opacity of
/// pieces still to come; with `reduced` motion the sung piece is at full opacity at once.
#[must_use]
pub fn pieces(line: &LyricLine, t_ms: i64, coming: f32, reduced: bool) -> Vec<Piece> {
    let texts = texts(line);
    let n = line.words.len();
    line.words
        .iter()
        .zip(texts)
        .enumerate()
        .map(|(i, (w, text))| {
            let next = line.words.get(i + 1).map(|n| n.start_ms);
            let fallback_end = if i + 1 == n {
                line.end_ms.unwrap_or(w.start_ms + LAST_PIECE_MS)
            } else {
                next.unwrap_or(w.start_ms + LAST_PIECE_MS)
            };
            let end = w.end_ms.unwrap_or(fallback_end);
            let dur = end.saturating_sub(w.start_ms).max(MIN_FADE_MS);
            Piece {
                text,
                alpha: alpha_at(t_ms, w.start_ms, dur, coming, reduced),
            }
        })
        .collect()
}

fn alpha_at(t_ms: i64, start: u64, dur: u64, coming: f32, reduced: bool) -> f32 {
    let start = i64::try_from(start).unwrap_or(i64::MAX);
    let dur = i64::try_from(dur).unwrap_or(i64::MAX);
    if t_ms < start {
        return coming;
    }
    let into = t_ms - start;
    if into >= dur {
        return SUNG_ALPHA;
    }
    if reduced {
        return 1.0;
    }
    // Both are at most one line long: exact enough in f32.
    #[allow(clippy::cast_precision_loss)]
    let p = into as f32 / dur as f32;
    coming + (1.0 - coming) * p
}

#[cfg(test)]
mod tests {
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
    fn sung_fading_and_coming_inks() {
        let l = line(
            "one two three",
            &[
                ("one ", 1000, Some(1400)),
                ("two ", 1400, Some(1800)),
                ("three", 1800, Some(1850)),
            ],
        );
        let p = pieces(&l, 1600, 0.55, false);
        assert!((p[0].alpha - SUNG_ALPHA).abs() < 1e-6);
        assert!(
            (p[1].alpha - (0.55 + 0.45 * 0.5)).abs() < 1e-3,
            "{}",
            p[1].alpha
        );
        assert!((p[2].alpha - 0.55).abs() < 1e-6);
        // A 50 ms piece still fades over 100 ms.
        let q = pieces(&l, 1850, 0.55, false);
        assert!(q[2].alpha > 0.55 && q[2].alpha < 1.0, "{}", q[2].alpha);
        // Reduced motion: the sung piece is bright at once.
        let r = pieces(&l, 1401, 0.55, true);
        assert!((r[1].alpha - 1.0).abs() < 1e-6);
    }

    #[test]
    fn missing_end_times_use_the_next_start() {
        let l = line("a b", &[("a ", 0, None), ("b", 1000, None)]);
        let p = pieces(&l, 500, 0.55, false);
        assert!((p[0].alpha - (0.55 + 0.45 * 0.5)).abs() < 1e-3);
        assert!((pieces(&l, 6000, 0.55, false)[1].alpha - SUNG_ALPHA).abs() < 1e-6);
    }
}
