//! What the catalog learned about tracks, kept in memory so a play needs no lookup of the track.
//!
//! Every track the catalog maps leaves a [`TrackRecord`] under its page address. The stream
//! resolver reads it when that page is played. Nothing is written to disk; at most
//! [`CAPACITY`] records are kept, the oldest first out.

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, PoisonError};

use nemo_core::secret::Secret;

use crate::model::{ScTrack, ScTranscoding};

/// Records kept.
pub const CAPACITY: usize = 200;

/// The streams of one track and what the stream lookup needs.
#[derive(Debug, Clone)]
pub struct TrackRecord {
    /// The numeric track id.
    pub id: u64,
    /// The streams the track offers.
    pub transcodings: Vec<ScTranscoding>,
    /// Sent with the stream lookup. Never logged.
    pub authorization: Option<Secret<String>>,
    /// The track is served as a preview only (`policy` is `SNIP`).
    pub preview: bool,
}

impl TrackRecord {
    /// The record of `track`, or `None` for a track that lists no stream (a stub).
    #[must_use]
    pub fn of(track: &ScTrack) -> Option<Self> {
        let media = track.media.as_ref()?;
        if media.transcodings.is_empty() {
            return None;
        }
        Some(Self {
            id: track.id,
            transcodings: media.transcodings.clone(),
            authorization: track.track_authorization.clone().map(Secret::new),
            preview: track.policy.as_deref() == Some("SNIP"),
        })
    }
}

#[derive(Default)]
struct Inner {
    by_page: HashMap<String, TrackRecord>,
    order: VecDeque<String>,
}

/// A small in-memory map from a track's page address to its record.
#[derive(Default)]
pub struct TrackRecords {
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for TrackRecords {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackRecords").finish_non_exhaustive()
    }
}

impl TrackRecords {
    /// Remember `record` for `page`, replacing an older one and dropping the oldest at capacity.
    pub fn insert(&self, page: &str, record: TrackRecord) {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if inner.by_page.insert(page.to_owned(), record).is_some() {
            inner.order.retain(|p| p != page);
        }
        inner.order.push_back(page.to_owned());
        while inner.order.len() > CAPACITY {
            if let Some(old) = inner.order.pop_front() {
                inner.by_page.remove(&old);
            }
        }
    }

    /// The record of `page`, if it is still kept.
    #[must_use]
    pub fn get(&self, page: &str) -> Option<TrackRecord> {
        let inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        inner.by_page.get(page).cloned()
    }

    /// How many records are kept.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .by_page
            .len()
    }

    /// Whether no record is kept.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: u64) -> TrackRecord {
        TrackRecord {
            id,
            transcodings: Vec::new(),
            authorization: None,
            preview: false,
        }
    }

    #[test]
    fn the_oldest_record_goes_first_and_a_reinsert_renews() {
        let records = TrackRecords::default();
        for n in 0..CAPACITY as u64 {
            records.insert(&format!("p{n}"), record(n));
        }
        records.insert("p0", record(1000));
        records.insert("new", record(2000));
        assert_eq!(records.len(), CAPACITY);
        assert_eq!(
            records.get("p0").map(|r| r.id),
            Some(1000),
            "renewed, so it stayed"
        );
        assert!(records.get("p1").is_none(), "the oldest went");
        assert_eq!(records.get("new").map(|r| r.id), Some(2000));
    }

    #[test]
    fn a_stub_has_no_record_and_a_snipped_track_is_a_preview() {
        let stub: ScTrack = serde_json::from_str(r#"{"id":1,"kind":"track"}"#).expect("stub");
        assert!(TrackRecord::of(&stub).is_none());
        let json = r#"{"id":2,"policy":"SNIP","track_authorization":"a",
            "media":{"transcodings":[{"url":"u","preset":"mp3_0_0","snipped":true,
            "format":{"protocol":"progressive"}}]}}"#;
        let track: ScTrack = serde_json::from_str(json).expect("track");
        let record = TrackRecord::of(&track).expect("record");
        assert!(record.preview);
        assert!(
            !format!("{record:?}").contains("\"a\""),
            "the authorization is redacted"
        );
    }
}
