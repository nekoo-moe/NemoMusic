//! The lyrics editor model (lyrics-plus): pure state for timing pasted lyrics by tapping.
//!
//! The UI thread owns an [`EditorModel`]. Each tap passes `PlayerSnapshot.position` read at
//! the key event; the model stores it as the start of the current line and moves on. The user
//! can step back, re-tap a line, nudge a start by [`NUDGE`], and run an optional word pass over
//! the same text. No time is ever made up: a line without a tap has no start, and
//! [`EditorModel::draft`] names the first such line instead of spreading times (criterion 6).

use std::time::Duration;

use nemo_core::lyrics::{LyricWord, LyricsDoc};
use nemo_core::lyrics_plus::{DraftLine, OwnLyricsDraft, OwnLyricsError};

/// One nudge step.
pub const NUDGE: Duration = Duration::from_millis(50);

/// Which pass the taps time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pass {
    /// Line starts.
    Lines,
    /// Word starts, line by line, after every line has a start.
    Words,
}

/// One line being timed: its text, its start and the starts of its words.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Line {
    text: String,
    start_ms: Option<u64>,
    /// The words with their starts; empty until the word pass reaches the line.
    words: Vec<(String, Option<u64>)>,
}

/// The editor state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorModel {
    lines: Vec<Line>,
    pass: Pass,
    /// The line the next tap times.
    line: usize,
    /// The word the next tap times, in the word pass.
    word: usize,
}

fn ms(position: Duration) -> u64 {
    u64::try_from(position.as_millis()).unwrap_or(u64::MAX)
}

/// The words of a line, each keeping the spaces after it, so they join back to the line.
fn split_words(text: &str) -> Vec<(String, Option<u64>)> {
    text.split_inclusive(' ')
        .filter(|w| !w.trim().is_empty())
        .map(|w| (w.to_owned(), None))
        .collect()
}

impl EditorModel {
    /// A model for pasted plain lyrics: one line per non-blank text line, trimmed, untimed.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let lines = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|l| Line {
                text: l.to_owned(),
                start_ms: None,
                words: Vec::new(),
            })
            .collect();
        Self {
            lines,
            pass: Pass::Lines,
            line: 0,
            word: 0,
        }
    }

    /// A model over existing lyrics, keeping their line starts, to re-time or fix them.
    #[must_use]
    pub fn from_doc(doc: &LyricsDoc) -> Self {
        let lines = doc
            .lines
            .iter()
            .map(|l| Line {
                text: l.text.clone(),
                start_ms: l.start_ms,
                words: Vec::new(),
            })
            .collect();
        Self {
            lines,
            pass: Pass::Lines,
            line: 0,
            word: 0,
        }
    }

    /// The current pass.
    #[must_use]
    pub fn pass(&self) -> Pass {
        self.pass
    }

    /// The line the next tap times; equal to the line count when every line is done.
    #[must_use]
    pub fn current_line(&self) -> usize {
        self.line
    }

    /// The word the next tap times in the word pass.
    #[must_use]
    pub fn current_word(&self) -> usize {
        self.word
    }

    /// How many lines there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether there is no line.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// The text and start of line `index`.
    #[must_use]
    pub fn line(&self, index: usize) -> Option<(&str, Option<u64>)> {
        self.lines.get(index).map(|l| (l.text.as_str(), l.start_ms))
    }

    /// The words of line `index` with their starts (empty before the word pass reaches it).
    #[must_use]
    pub fn words(&self, index: usize) -> Vec<(&str, Option<u64>)> {
        self.lines.get(index).map_or_else(Vec::new, |l| {
            l.words.iter().map(|(w, s)| (w.as_str(), *s)).collect()
        })
    }

    /// Tap: the playback `position` starts the current line (or word) and the next one
    /// becomes current. A tap after the last line or word does nothing.
    pub fn tap(&mut self, position: Duration) {
        let at = ms(position);
        match self.pass {
            Pass::Lines => {
                if let Some(line) = self.lines.get_mut(self.line) {
                    line.start_ms = Some(at);
                    self.line += 1;
                }
            }
            Pass::Words => self.tap_word(at),
        }
    }

    fn tap_word(&mut self, at: u64) {
        let Some(line) = self.lines.get_mut(self.line) else {
            return;
        };
        if let Some(word) = line.words.get_mut(self.word) {
            word.1 = Some(at);
            self.word += 1;
        }
        if self.word >= line.words.len() {
            self.line += 1;
            self.word = 0;
        }
    }

    /// Step back one line (one word in the word pass), so the next tap times it again. The
    /// stored start stays until that tap.
    pub fn step_back(&mut self) {
        match self.pass {
            Pass::Lines => self.line = self.line.saturating_sub(1),
            Pass::Words => {
                if self.word > 0 {
                    self.word -= 1;
                } else if self.line > 0 {
                    self.line -= 1;
                    let words = self.lines.get(self.line).map_or(0, |l| l.words.len());
                    self.word = words.saturating_sub(1);
                }
            }
        }
    }

    /// Make line `index` current, so the next tap re-times it. Out of range does nothing.
    pub fn select_line(&mut self, index: usize) {
        if index < self.lines.len() {
            self.line = index;
            self.word = 0;
        }
    }

    /// Re-tap line `index`: set its start to `position` and make the line after it current.
    pub fn retap(&mut self, index: usize, position: Duration) {
        if self.pass == Pass::Lines && index < self.lines.len() {
            self.line = index;
            self.tap(position);
        }
    }

    /// Move the start of line `index` by `steps` of [`NUDGE`] (negative is earlier), not
    /// below 0. A line with no start stays without one.
    pub fn nudge(&mut self, index: usize, steps: i32) {
        let Some(start) = self.lines.get_mut(index).and_then(|l| l.start_ms.as_mut()) else {
            return;
        };
        let delta = u64::from(steps.unsigned_abs()) * ms(NUDGE);
        *start = if steps < 0 {
            start.saturating_sub(delta)
        } else {
            start.saturating_add(delta)
        };
    }

    /// Start the optional word pass over the same text, from the first line.
    ///
    /// # Errors
    /// [`OwnLyricsError::Empty`] with no lines, and [`OwnLyricsError::MissingStart`] naming
    /// the first line (from 1) that has no start yet.
    pub fn start_word_pass(&mut self) -> Result<(), OwnLyricsError> {
        self.first_missing()?;
        for line in &mut self.lines {
            line.words = split_words(&line.text);
        }
        self.pass = Pass::Words;
        self.line = 0;
        self.word = 0;
        Ok(())
    }

    fn first_missing(&self) -> Result<(), OwnLyricsError> {
        if self.lines.is_empty() {
            return Err(OwnLyricsError::Empty);
        }
        match self.lines.iter().position(|l| l.start_ms.is_none()) {
            Some(index) => Err(OwnLyricsError::MissingStart { line: index + 1 }),
            None => Ok(()),
        }
    }

    /// The draft to save. A line keeps its words only when every word of it was tapped.
    ///
    /// # Errors
    /// [`OwnLyricsError::Empty`] with no lines, and [`OwnLyricsError::MissingStart`] naming
    /// the first line (from 1) that has no start. Nothing is spread.
    pub fn draft(&self) -> Result<OwnLyricsDraft, OwnLyricsError> {
        self.first_missing()?;
        let lines = self
            .lines
            .iter()
            .map(|l| {
                let words: Option<Vec<LyricWord>> = l
                    .words
                    .iter()
                    .map(|(text, start)| {
                        start.map(|start_ms| LyricWord {
                            start_ms,
                            end_ms: None,
                            text: text.clone(),
                        })
                    })
                    .collect();
                DraftLine {
                    text: l.text.clone(),
                    start_ms: l.start_ms,
                    words: words.unwrap_or_default(),
                }
            })
            .collect();
        Ok(OwnLyricsDraft { lines })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "first line\n\n  second line  \nthird\nfourth";

    fn at(ms: u64) -> Duration {
        Duration::from_millis(ms)
    }

    fn starts(model: &EditorModel) -> Vec<Option<u64>> {
        (0..model.len())
            .map(|i| model.line(i).and_then(|(_, s)| s))
            .collect()
    }

    #[test]
    fn pasted_text_becomes_untimed_lines() {
        let model = EditorModel::from_text(TEXT);
        assert_eq!(model.len(), 4);
        assert_eq!(model.line(1), Some(("second line", None)));
        assert_eq!(model.pass(), Pass::Lines);
        assert!(EditorModel::from_text(" \n ").is_empty());
    }

    #[test]
    fn a_tap_starts_the_current_line_and_moves_on() {
        let mut model = EditorModel::from_text(TEXT);
        model.tap(at(1_200));
        model.tap(at(3_450));
        assert_eq!(starts(&model), vec![Some(1_200), Some(3_450), None, None]);
        assert_eq!(model.current_line(), 2);
        model.tap(at(5_000));
        model.tap(at(7_000));
        model.tap(at(9_000));
        assert_eq!(
            model.current_line(),
            4,
            "a tap after the last line does nothing"
        );
        assert_eq!(starts(&model)[3], Some(7_000));
    }

    #[test]
    fn step_back_and_retap_time_a_line_again() {
        let mut model = EditorModel::from_text(TEXT);
        model.tap(at(1_000));
        model.tap(at(2_000));
        model.step_back();
        assert_eq!(model.current_line(), 1);
        assert_eq!(starts(&model)[1], Some(2_000), "kept until the next tap");
        model.tap(at(2_400));
        assert_eq!(starts(&model)[1], Some(2_400));
        model.retap(0, at(900));
        assert_eq!(starts(&model)[0], Some(900));
        assert_eq!(model.current_line(), 1);
        model.step_back();
        model.step_back();
        assert_eq!(model.current_line(), 0, "never before the first line");
    }

    #[test]
    fn nudge_moves_a_start_by_50_ms_and_not_below_zero() {
        let mut model = EditorModel::from_text(TEXT);
        model.tap(at(100));
        model.nudge(0, 1);
        assert_eq!(starts(&model)[0], Some(150));
        model.nudge(0, -2);
        assert_eq!(starts(&model)[0], Some(50));
        model.nudge(0, -5);
        assert_eq!(starts(&model)[0], Some(0));
        model.nudge(1, 3);
        assert_eq!(starts(&model)[1], None, "an untimed line gets no start");
    }

    #[test]
    fn saving_names_the_first_line_with_no_start_and_spreads_nothing() {
        let mut model = EditorModel::from_text(TEXT);
        model.tap(at(1_000));
        model.tap(at(2_000));
        assert_eq!(model.draft(), Err(OwnLyricsError::MissingStart { line: 3 }));
        assert_eq!(starts(&model), vec![Some(1_000), Some(2_000), None, None]);
        assert_eq!(
            EditorModel::from_text("").draft(),
            Err(OwnLyricsError::Empty)
        );
        model.tap(at(3_000));
        model.tap(at(4_000));
        let draft = model.draft().expect("complete");
        assert_eq!(draft.lines[2].start_ms, Some(3_000));
        assert!(draft.lines.iter().all(|l| l.words.is_empty()));
    }

    #[test]
    fn the_word_pass_times_words_line_by_line() {
        let mut model = EditorModel::from_text("a b\nc");
        assert_eq!(
            model.start_word_pass(),
            Err(OwnLyricsError::MissingStart { line: 1 })
        );
        model.tap(at(0));
        model.tap(at(1_000));
        model.start_word_pass().expect("lines are timed");
        assert_eq!(model.pass(), Pass::Words);
        model.tap(at(10));
        model.step_back();
        model.tap(at(20));
        model.tap(at(400));
        assert_eq!(model.current_line(), 1);
        assert_eq!(model.words(0), vec![("a ", Some(20)), ("b", Some(400))]);
        let half = model.draft().expect("draft");
        assert_eq!(half.lines[0].words.len(), 2);
        assert!(
            half.lines[1].words.is_empty(),
            "an untapped word keeps the line line-timed"
        );
        model.tap(at(1_050));
        let full = model.draft().expect("draft");
        assert_eq!(full.lines[1].words[0].start_ms, 1_050);
        model.step_back();
        assert_eq!((model.current_line(), model.current_word()), (1, 0));
    }

    #[test]
    fn existing_lyrics_keep_their_starts() {
        let doc = LyricsDoc {
            lines: vec![nemo_core::lyrics::LyricLine {
                start_ms: Some(500),
                end_ms: None,
                text: "kept".to_owned(),
                words: Vec::new(),
            }],
            granularity: nemo_core::lyrics::Granularity::Line,
            provider: nemo_core::lyrics::ProviderId::new("LRCLIB"),
            matched: None,
            instrumental: false,
        };
        let model = EditorModel::from_doc(&doc);
        assert_eq!(model.line(0), Some(("kept", Some(500))));
    }
}
