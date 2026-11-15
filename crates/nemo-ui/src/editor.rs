//! The lyrics editor model (lyrics-plus): pure state for timing pasted lyrics by tapping.
//!
//! The UI thread owns an [`EditorModel`]. Each tap passes `PlayerSnapshot.position` read at
//! the key event; the model stores it as the start of the current line and moves on. The user
//! can step back, re-tap a line, nudge a start by [`NUDGE`], and run an optional word pass over
//! the same text. No time is ever made up: a line without a tap has no start, and
//! [`EditorModel::draft`] names the first such line instead of spreading times (criterion 6).
//!
//! The preview plays the song with the unsaved draft: [`EditorModel::preview_doc`] gives the
//! lyrics the view will show after saving, and [`EditorModel::preview_line`] finds the active
//! line with the lyrics view's own finder and the track offset (criteria 5 and 29).

use std::time::Duration;

use nemo_core::lyrics::{Granularity, LyricLine, LyricWord, LyricsDoc};
use nemo_core::lyrics_plus::{DraftLine, OWN_LYRICS, OwnLyricsDraft, OwnLyricsError};

use crate::lyrics_view;

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

    /// The words of a line as saved: all of them when every word was tapped, else none.
    fn saved_words(line: &Line) -> Vec<LyricWord> {
        line.words
            .iter()
            .map(|(text, start)| {
                start.map(|start_ms| LyricWord {
                    start_ms,
                    end_ms: None,
                    text: text.clone(),
                })
            })
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default()
    }

    /// The unsaved draft as the lyrics the view will show once it is saved: the same lines,
    /// starts and words as [`EditorModel::draft`], `Word` granularity when every line has
    /// words and `Line` otherwise, and the own-lyrics provider. A line not tapped yet keeps no
    /// start, so the highlight stops before it; nothing is spread.
    #[must_use]
    pub fn preview_doc(&self) -> LyricsDoc {
        let lines: Vec<LyricLine> = self
            .lines
            .iter()
            .map(|l| LyricLine {
                start_ms: l.start_ms,
                end_ms: None,
                text: l.text.clone(),
                words: Self::saved_words(l),
            })
            .collect();
        let granularity = if !lines.is_empty() && lines.iter().all(|l| !l.words.is_empty()) {
            Granularity::Word
        } else {
            Granularity::Line
        };
        LyricsDoc {
            lines,
            granularity,
            provider: OWN_LYRICS,
            matched: None,
            instrumental: false,
        }
    }

    /// The line the preview highlights at the playback `position` with the track's
    /// `offset_ms` (`lyrics_offset_ms`), found by [`lyrics_view::active_line_at`] exactly as
    /// the lyrics view finds it. Builds [`EditorModel::preview_doc`]; a view that draws every
    /// frame keeps that doc and calls [`lyrics_view::active_line_at`] itself.
    #[must_use]
    pub fn preview_line(&self, position: Duration, offset_ms: i64) -> Option<usize> {
        lyrics_view::active_line_at(&self.preview_doc(), position, offset_ms)
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
            .map(|l| DraftLine {
                text: l.text.clone(),
                start_ms: l.start_ms,
                words: Self::saved_words(l),
            })
            .collect();
        Ok(OwnLyricsDraft { lines })
    }
}

#[cfg(test)]
mod tests;
