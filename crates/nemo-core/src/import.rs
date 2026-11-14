//! Importing a playlist exported from another service (Exportify, Soundiiz CSV files).
//!
//! No I/O here. `nemo-library` implements [`PlaylistImport`]: it parses the file, then matches
//! each row against the catalog with one human-paced search at a time. The user checks the
//! matches before anything is saved.

use std::time::Duration;

use crate::BoxFuture;
use crate::catalog::CatalogError;
use crate::track::Track;

/// One song as the exported file lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedSong {
    /// Song title.
    pub title: String,
    /// Artist names, in file order.
    pub artists: Vec<String>,
    /// Album title, if the file has one.
    pub album: Option<String>,
    /// Length, if the file has one.
    pub duration: Option<Duration>,
    /// ISRC code, if the file has one.
    pub isrc: Option<String>,
    /// Line in the file, counted from 1.
    pub line: u32,
}

/// How sure a match is. Ordered from `None` (no usable match) to `High`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Confidence {
    /// No usable match.
    None,
    /// A weak match; the user should check it.
    Low,
    /// A likely match.
    Medium,
    /// A sure match.
    High,
}

/// A row of the file that was not imported, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedRow {
    /// Line in the file, counted from 1.
    pub line: u32,
    /// Plain reason, such as "no title".
    pub reason: String,
}

/// A catalog track that may be the imported song.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchCandidate {
    /// The catalog track.
    pub track: Track,
    /// Match score, 0 to 100.
    pub score: u8,
    /// How sure the match is.
    pub confidence: Confidence,
}

/// Where a row stands in the import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RowState {
    /// Not searched yet, or its search failed.
    #[default]
    Pending,
    /// Searched; the candidates are listed.
    Matched,
    /// The user left it out.
    Skipped,
}

/// One song of the import with its candidates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRow {
    /// The song as the file lists it.
    pub song: ImportedSong,
    /// The candidates found, best first.
    pub candidates: Vec<MatchCandidate>,
    /// Index into `candidates` of the one to import. Never pre-selected for a `None` match.
    pub chosen: Option<usize>,
    /// Where the row stands.
    pub state: RowState,
}

/// One import in progress: the parsed rows, the skipped lines and how far matching got.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSession {
    /// The playlist name, from the file name.
    pub name: String,
    /// The rows to match, in file order.
    pub rows: Vec<ImportRow>,
    /// Lines that were not imported.
    pub skipped: Vec<SkippedRow>,
    /// Matching continues from this row.
    cursor: usize,
}

impl ImportSession {
    /// A new session that has matched nothing yet.
    #[must_use]
    pub const fn new(name: String, rows: Vec<ImportRow>, skipped: Vec<SkippedRow>) -> Self {
        Self {
            name,
            rows,
            skipped,
            cursor: 0,
        }
    }

    /// The index of the next [`RowState::Pending`] row matching should search, or `None` when
    /// none is left.
    #[must_use]
    pub fn next_pending(&self) -> Option<usize> {
        self.rows
            .iter()
            .enumerate()
            .skip(self.cursor)
            .find(|(_, r)| r.state == RowState::Pending)
            .map(|(i, _)| i)
    }

    /// Continue matching after `row`, which was searched. A row whose search failed stays
    /// pending and is not passed, so it is tried again.
    pub fn pass(&mut self, row: usize) {
        self.cursor = self.cursor.max(row.saturating_add(1));
    }
}

/// Reading or matching an import failed. Texts hold no song titles or file text.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportError {
    /// The file is not UTF-8 text.
    #[error("the file is not valid UTF-8 text")]
    NotText,
    /// The file is empty.
    #[error("the file is empty")]
    Empty,
    /// The file is over the size limit.
    #[error("the file is larger than {limit} bytes")]
    TooLarge {
        /// The limit in bytes.
        limit: u64,
    },
    /// No column holds the song titles.
    #[error("no title column found")]
    NoTitleColumn,
    /// The file is not a supported export format.
    #[error("this file format is not supported")]
    Unsupported,
    /// A catalog search failed; the row stays pending.
    #[error("search failed: {0}")]
    Search(CatalogError),
}

/// Parses exported playlists and matches them against the catalog.
pub trait PlaylistImport: Send + Sync {
    /// Read an exported file into a session. Nothing is searched yet.
    ///
    /// # Errors
    /// An [`ImportError`] when the file is not text, empty, too large, has no title column or
    /// is not a supported format. Nothing is imported then.
    fn parse(&self, file_name: &str, bytes: &[u8]) -> Result<ImportSession, ImportError>;

    /// Match the next pending row with one human-paced search. `Ok(false)` when none is left.
    ///
    /// # Errors
    /// [`ImportError::Search`] when the search fails; the row stays pending.
    fn match_next<'a>(
        &'a self,
        session: &'a mut ImportSession,
    ) -> BoxFuture<'a, Result<bool, ImportError>>;

    /// Search one row again with text the user typed, replacing its candidates.
    ///
    /// # Errors
    /// [`ImportError::Search`] when the search fails; the row keeps its candidates.
    fn research<'a>(
        &'a self,
        session: &'a mut ImportSession,
        row: usize,
        text: &'a str,
    ) -> BoxFuture<'a, Result<(), ImportError>>;

    /// The chosen tracks of matched rows, in file order.
    fn accepted(&self, session: &ImportSession) -> Vec<Track>;

    /// The rows without a chosen track, as plain text the user can copy.
    fn unmatched_text(&self, session: &ImportSession) -> String;
}

#[cfg(test)]
mod tests {
    use super::{Confidence, ImportRow, ImportSession, ImportedSong, RowState};

    #[test]
    fn confidence_orders_none_low_medium_high() {
        assert!(Confidence::None < Confidence::Low);
        assert!(Confidence::Low < Confidence::Medium);
        assert!(Confidence::Medium < Confidence::High);
        let mut all = [
            Confidence::High,
            Confidence::None,
            Confidence::Medium,
            Confidence::Low,
        ];
        all.sort();
        assert_eq!(
            all,
            [
                Confidence::None,
                Confidence::Low,
                Confidence::Medium,
                Confidence::High
            ]
        );
    }

    fn row(state: RowState) -> ImportRow {
        ImportRow {
            song: ImportedSong {
                title: "Song".to_owned(),
                artists: Vec::new(),
                album: None,
                duration: None,
                isrc: None,
                line: 2,
            },
            candidates: Vec::new(),
            chosen: None,
            state,
        }
    }

    #[test]
    fn the_cursor_skips_passed_and_finished_rows() {
        let rows = vec![
            row(RowState::Pending),
            row(RowState::Skipped),
            row(RowState::Pending),
        ];
        let mut s = ImportSession::new("List".to_owned(), rows, Vec::new());
        assert_eq!(s.next_pending(), Some(0));
        // A failed search does not pass the row.
        assert_eq!(s.next_pending(), Some(0));
        s.rows[0].state = RowState::Matched;
        s.pass(0);
        assert_eq!(s.next_pending(), Some(2));
        s.pass(2);
        assert_eq!(s.next_pending(), None);
    }
}
