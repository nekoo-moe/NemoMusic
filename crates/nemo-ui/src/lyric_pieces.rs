//! The timed pieces of word- and syllable-timed lyrics (`lyrics-rebuild` spec). Pure: it splits a
//! line into its pieces, gives each its start and end, and (in [`motion`]) says what the piece
//! looks like at a playback position: the soft sweep, the glow, the wobble and the letter wave.
//! Drawing lives in `gui::lyric_list`.

use nemo_core::lyrics::LyricLine;

pub mod motion;

/// The shortest length of one piece, ms.
pub const MIN_PIECE_MS: u64 = 100;
/// Length of the last piece when the provider gives no end time, ms.
const LAST_PIECE_MS: u64 = 600;
/// Alpha of an unsung word: the same token as an unlit line (owner, 2026-11-16 and 2026-11-18).
pub const UNSUNG_ALPHA: f32 = crate::lyrics_view::metrics::INACTIVE_ALPHA;

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

/// Start and end of one timed piece, ms of song time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// When the piece starts.
    pub start_ms: i64,
    /// When it ends (at least [`MIN_PIECE_MS`] after the start).
    pub end_ms: i64,
}

impl Timing {
    /// Length in ms.
    #[must_use]
    pub fn len_ms(self) -> i64 {
        self.end_ms - self.start_ms
    }
}

/// The timing of every piece of `line`. A missing end is the next start; the last piece ends
/// with the line, or [`LAST_PIECE_MS`] after its start.
#[must_use]
pub fn timings(line: &LyricLine) -> Vec<Timing> {
    let n = line.words.len();
    line.words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let next = line.words.get(i + 1).map(|n| n.start_ms);
            let fallback_end = if i + 1 == n {
                line.end_ms.unwrap_or(w.start_ms + LAST_PIECE_MS)
            } else {
                next.unwrap_or(w.start_ms + LAST_PIECE_MS)
            };
            let end = w.end_ms.unwrap_or(fallback_end);
            let start = i64::try_from(w.start_ms).unwrap_or(i64::MAX / 2);
            let len = i64::try_from(end.saturating_sub(w.start_ms).max(MIN_PIECE_MS))
                .unwrap_or(i64::MAX / 2);
            Timing {
                start_ms: start,
                end_ms: start.saturating_add(len),
            }
        })
        .collect()
}

/// `true` when the first letter with a direction in `text` reads right to left (Hebrew, Arabic
/// and their relatives), so the sweep runs from the right.
#[must_use]
pub fn is_rtl(text: &str) -> bool {
    text.chars()
        .find(|c| c.is_alphabetic())
        .is_some_and(|c| matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF))
}

#[cfg(test)]
mod tests;
