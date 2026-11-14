//! Private serde mirror of the cached lyrics body and its encode/decode functions.

use nemo_core::lyrics::{Granularity, LyricLine, LyricWord, LyricsDoc, MatchedMeta, ProviderId};
use serde::{Deserialize, Serialize};

use super::CacheError;

// Private serde mirror of the nemo-core types, which carry no serde. The provider is not stored:
// it is the row's `provider` column and is restored from the id the caller asks with.

#[derive(Serialize, Deserialize)]
enum GranularityRec {
    Unsynced,
    Line,
    Word,
    Syllable,
}

#[derive(Serialize, Deserialize)]
struct WordRec {
    start_ms: u64,
    end_ms: Option<u64>,
    text: String,
}

#[derive(Serialize, Deserialize)]
struct LineRec {
    start_ms: Option<u64>,
    end_ms: Option<u64>,
    text: String,
    words: Vec<WordRec>,
}

#[derive(Serialize, Deserialize)]
struct MatchedRec {
    title: String,
    artists: Vec<String>,
    album: Option<String>,
    duration_ms: Option<u64>,
}

#[derive(Serialize, Deserialize)]
struct DocRec {
    lines: Vec<LineRec>,
    granularity: GranularityRec,
    matched: Option<MatchedRec>,
    instrumental: bool,
}

impl From<Granularity> for GranularityRec {
    fn from(g: Granularity) -> Self {
        match g {
            Granularity::Unsynced => Self::Unsynced,
            Granularity::Line => Self::Line,
            Granularity::Word => Self::Word,
            Granularity::Syllable => Self::Syllable,
        }
    }
}

impl From<GranularityRec> for Granularity {
    fn from(g: GranularityRec) -> Self {
        match g {
            GranularityRec::Unsynced => Self::Unsynced,
            GranularityRec::Line => Self::Line,
            GranularityRec::Word => Self::Word,
            GranularityRec::Syllable => Self::Syllable,
        }
    }
}

pub(super) fn granularity_name(g: Granularity) -> &'static str {
    match g {
        Granularity::Unsynced => "unsynced",
        Granularity::Line => "line",
        Granularity::Word => "word",
        Granularity::Syllable => "syllable",
    }
}

pub(super) fn encode_doc(doc: &LyricsDoc) -> Result<Vec<u8>, CacheError> {
    let rec = DocRec {
        lines: doc
            .lines
            .iter()
            .map(|l| LineRec {
                start_ms: l.start_ms,
                end_ms: l.end_ms,
                text: l.text.clone(),
                words: l
                    .words
                    .iter()
                    .map(|w| WordRec {
                        start_ms: w.start_ms,
                        end_ms: w.end_ms,
                        text: w.text.clone(),
                    })
                    .collect(),
            })
            .collect(),
        granularity: doc.granularity.into(),
        matched: doc.matched.as_ref().map(|m| MatchedRec {
            title: m.title.clone(),
            artists: m.artists.clone(),
            album: m.album.clone(),
            duration_ms: m.duration_ms,
        }),
        instrumental: doc.instrumental,
    };
    serde_json::to_vec(&rec).map_err(|e| CacheError::Body(e.to_string()))
}

pub(super) fn decode_doc(body: &[u8], provider: ProviderId) -> Result<LyricsDoc, CacheError> {
    let rec: DocRec = serde_json::from_slice(body).map_err(|e| CacheError::Body(e.to_string()))?;
    Ok(LyricsDoc {
        lines: rec
            .lines
            .into_iter()
            .map(|l| LyricLine {
                start_ms: l.start_ms,
                end_ms: l.end_ms,
                text: l.text,
                words: l
                    .words
                    .into_iter()
                    .map(|w| LyricWord {
                        start_ms: w.start_ms,
                        end_ms: w.end_ms,
                        text: w.text,
                    })
                    .collect(),
            })
            .collect(),
        granularity: rec.granularity.into(),
        provider,
        matched: rec.matched.map(|m| MatchedMeta {
            title: m.title,
            artists: m.artists,
            album: m.album,
            duration_ms: m.duration_ms,
        }),
        instrumental: rec.instrumental,
    })
}
