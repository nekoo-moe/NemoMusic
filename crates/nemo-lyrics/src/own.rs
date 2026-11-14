//! The user's own lyrics: written, synced or copied from a provider, kept on this computer
//! (lyrics-plus, criteria 6, 7, 9 and 10).
//!
//! They live in the table `own_lyrics` of the lyrics database, one row per track key, beside
//! the cache but apart from it: [`LyricsCache::clear`] and [`LyricsCache::remove_track`] keep
//! them, as they keep offsets, and saving a copy never changes the provider's cached row. Only
//! [`OwnLyrics::delete`] removes them. Times are never spread over the song: a line without a
//! start is refused. Nothing here sends lyrics anywhere. [`OwnLyricsStore`] is the
//! [`OwnLyrics`] implementation.

use std::sync::Arc;

use nemo_core::lyrics::{Granularity, LyricLine, LyricsDoc};
use nemo_core::lyrics_plus::{OWN_LYRICS, OwnLyrics, OwnLyricsDraft, OwnLyricsError};
use rusqlite::{OptionalExtension, params};

use crate::cache::{LyricsCache, decode_doc, encode_doc, granularity_name};

/// The text of every store error. It names no track and quotes no lyrics.
const STORE: &str = "the lyrics database could not be read or written";

fn store_error<E>(_: E) -> OwnLyricsError {
    OwnLyricsError::Store(STORE.to_owned())
}

/// Check a draft and turn it into lyrics: [`OwnLyricsError::Empty`] with no lines,
/// [`OwnLyricsError::MissingStart`] for the first line with no start, and
/// [`OwnLyricsError::WordOrder`] for the first line whose words go back in time or end before
/// they start. The granularity is `Word` when every line has words, otherwise `Line`.
///
/// # Errors
/// The three errors above.
pub fn draft_to_doc(draft: &OwnLyricsDraft) -> Result<LyricsDoc, OwnLyricsError> {
    if draft.lines.is_empty() {
        return Err(OwnLyricsError::Empty);
    }
    let mut lines = Vec::with_capacity(draft.lines.len());
    for (index, draft_line) in draft.lines.iter().enumerate() {
        let line = index + 1;
        let start = draft_line
            .start_ms
            .ok_or(OwnLyricsError::MissingStart { line })?;
        let words = &draft_line.words;
        let in_order = words
            .iter()
            .zip(words.iter().skip(1))
            .all(|(a, b)| a.start_ms <= b.start_ms)
            && words
                .iter()
                .all(|w| w.end_ms.is_none_or(|end| end >= w.start_ms));
        if !in_order {
            return Err(OwnLyricsError::WordOrder { line });
        }
        lines.push(LyricLine {
            start_ms: Some(start),
            end_ms: None,
            text: draft_line.text.clone(),
            words: draft_line.words.clone(),
        });
    }
    let granularity = if lines.iter().all(|l| !l.words.is_empty()) {
        Granularity::Word
    } else {
        Granularity::Line
    };
    Ok(LyricsDoc {
        lines,
        granularity,
        provider: OWN_LYRICS,
        matched: None,
        instrumental: false,
    })
}

/// [`OwnLyrics`] over the lyrics database of a [`LyricsCache`].
#[derive(Debug, Clone)]
pub struct OwnLyricsStore {
    cache: Arc<LyricsCache>,
}

impl OwnLyricsStore {
    /// Own lyrics in the database of `cache`; [`LyricsCache::open`] made the table.
    #[must_use]
    pub fn new(cache: Arc<LyricsCache>) -> Self {
        Self { cache }
    }

    fn put_own(&self, track_key: &str, doc: &LyricsDoc) -> Result<(), OwnLyricsError> {
        let body = encode_doc(doc).map_err(store_error)?;
        self.cache
            .lock()
            .execute(
                "INSERT INTO own_lyrics (track_key, granularity, body, saved_at)
                 VALUES (?1, ?2, ?3, unixepoch())
                 ON CONFLICT (track_key) DO UPDATE SET
                     granularity = excluded.granularity, body = excluded.body,
                     saved_at = excluded.saved_at",
                params![track_key, granularity_name(doc.granularity), body],
            )
            .map_err(store_error)?;
        Ok(())
    }
}

impl OwnLyrics for OwnLyricsStore {
    fn get(&self, track_key: &str) -> Option<LyricsDoc> {
        let body: Option<Vec<u8>> = self
            .cache
            .lock()
            .query_row(
                "SELECT body FROM own_lyrics WHERE track_key = ?1",
                params![track_key],
                |r| r.get(0),
            )
            .optional()
            .inspect_err(|_| tracing::warn!("own lyrics could not be read"))
            .ok()
            .flatten();
        let doc = decode_doc(&body?, OWN_LYRICS)
            .inspect_err(|_| tracing::warn!("stored own lyrics could not be decoded"))
            .ok()?;
        Some(doc)
    }

    fn save(&self, track_key: &str, draft: &OwnLyricsDraft) -> Result<Granularity, OwnLyricsError> {
        let doc = draft_to_doc(draft)?;
        self.put_own(track_key, &doc)?;
        Ok(doc.granularity)
    }

    fn save_copy(&self, track_key: &str, doc: &LyricsDoc) -> Result<(), OwnLyricsError> {
        let copy = LyricsDoc {
            provider: OWN_LYRICS,
            ..doc.clone()
        };
        self.put_own(track_key, &copy)
    }

    fn delete(&self, track_key: &str) -> Result<(), OwnLyricsError> {
        self.cache
            .lock()
            .execute(
                "DELETE FROM own_lyrics WHERE track_key = ?1",
                params![track_key],
            )
            .map_err(store_error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
