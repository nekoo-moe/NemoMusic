//! Parser for `LyricsPlus` `/v2/lyrics/get` answers and their 404 bodies (see the parent
//! module for the facts it relies on).

use std::collections::BTreeMap;

use nemo_core::lyrics::{
    DetailedDoc, Granularity, LyricLine, LyricWord, LyricsCredit, LyricsDoc, LyricsError,
    MatchedMeta,
};
use nemo_core::lyrics_plus::LineMeta;
use serde::Deserialize;

use super::{CREDIT_URL, LYRICSPLUS_ID};
use crate::http::split_artists;

/// Longest upstream name kept for the credit line.
pub(super) const MAX_NAME_CHARS: usize = 40;

#[derive(Debug, Deserialize)]
struct Answer {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    metadata: Metadata,
    #[serde(default)]
    lyrics: Vec<Line>,
    #[serde(rename = "processingTime")]
    processing: Option<Processing>,
}

#[derive(Debug, Default, Deserialize)]
struct Metadata {
    source: Option<String>,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    curator: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Processing {
    winner_source: Option<String>,
    selected_song_metadata: Option<Picked>,
    #[serde(default)]
    sources_status: BTreeMap<String, SourceStatus>,
}

#[derive(Debug, Deserialize)]
struct Picked {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct SourceStatus {
    #[serde(default)]
    status: String,
}

#[derive(Debug, Deserialize)]
struct Line {
    #[serde(default)]
    time: i64,
    #[serde(default)]
    duration: i64,
    #[serde(default)]
    text: String,
    #[serde(default)]
    syllabus: Vec<Piece>,
    #[serde(default)]
    element: Element,
    transliteration: Option<Transliteration>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Piece {
    #[serde(default)]
    time: i64,
    #[serde(default)]
    duration: i64,
    #[serde(default)]
    text: String,
    #[serde(default)]
    is_background: bool,
    #[serde(default)]
    synthetic: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Element {
    singer: Option<String>,
    #[serde(default)]
    is_background: bool,
}

#[derive(Debug, Deserialize)]
struct Transliteration {
    #[serde(default)]
    text: String,
}

#[derive(Debug, Default, Deserialize)]
struct MissBody {
    #[serde(rename = "processingTime")]
    processing: Option<Processing>,
}

/// A time from the answer in ms; negative values count as 0.
fn ms(v: i64) -> u64 {
    u64::try_from(v).unwrap_or(0)
}

/// One output line with its side data and whether it splits a word into pieces.
struct Built {
    line: LyricLine,
    meta: LineMeta,
    split: bool,
}

/// Builds lines, numbering singers in the order they first appear.
#[derive(Default)]
struct Builder {
    singers: Vec<String>,
    out: Vec<Built>,
}

impl Builder {
    fn singer(&mut self, alias: Option<&str>) -> Option<u8> {
        let alias = alias.map(str::trim).filter(|a| !a.is_empty())?;
        let at = self
            .singers
            .iter()
            .position(|s| s == alias)
            .unwrap_or_else(|| {
                self.singers.push(alias.to_owned());
                self.singers.len() - 1
            });
        u8::try_from(at).ok()
    }

    /// A line with line timing only (or none, when `timed` is false).
    fn plain(&mut self, line: &Line, timed: bool, meta: LineMeta) {
        let start = ms(line.time);
        self.out.push(Built {
            line: LyricLine {
                start_ms: timed.then_some(start),
                end_ms: timed.then_some(start.saturating_add(ms(line.duration))),
                text: line.text.trim().to_owned(),
                words: Vec::new(),
            },
            meta,
            split: false,
        });
    }

    /// A line built from its timed pieces. Nothing is pushed when `pieces` is empty.
    fn pieces(&mut self, pieces: &[&Piece], start: Option<u64>, meta: LineMeta) {
        let words: Vec<LyricWord> = pieces
            .iter()
            .filter(|p| !p.text.is_empty())
            .map(|p| LyricWord {
                start_ms: ms(p.time),
                end_ms: Some(ms(p.time).saturating_add(ms(p.duration))),
                text: p.text.clone(),
            })
            .collect();
        let (Some(first), Some(end)) = (words.first(), words.iter().filter_map(|w| w.end_ms).max())
        else {
            return;
        };
        // A piece with no trailing whitespace followed by another piece splits a word.
        let split = words
            .iter()
            .rev()
            .skip(1)
            .any(|w| !w.text.ends_with(char::is_whitespace));
        let text: String = words.iter().map(|w| w.text.as_str()).collect();
        self.out.push(Built {
            line: LyricLine {
                start_ms: Some(start.unwrap_or(first.start_ms).min(first.start_ms)),
                end_ms: Some(end),
                text: text.trim().to_owned(),
                words,
            },
            meta,
            split,
        });
    }

    fn line(&mut self, line: &Line, kind: Kind) -> bool {
        let singer = self.singer(line.element.singer.as_deref());
        let roman = line
            .transliteration
            .as_ref()
            .map(|t| t.text.trim().to_owned())
            .filter(|t| !t.is_empty());
        let meta = |background: bool, roman: Option<String>| LineMeta {
            singer,
            background,
            roman,
        };
        let lead_bg = line.element.is_background;
        if kind != Kind::Timed || line.syllabus.is_empty() {
            self.plain(line, kind != Kind::Unsynced, meta(lead_bg, roman));
            return false;
        }
        if line.syllabus.iter().any(|p| p.synthetic) {
            self.plain(line, true, meta(lead_bg, roman));
            return true;
        }
        let (bg, main): (Vec<&Piece>, Vec<&Piece>) = line
            .syllabus
            .iter()
            .partition(|p| p.is_background || lead_bg);
        let before = self.out.len();
        self.pieces(&main, Some(ms(line.time)), meta(false, roman.clone()));
        // The romanization belongs to the main line; a background-only line keeps it.
        let bg_roman = (self.out.len() == before).then_some(roman).flatten();
        self.pieces(&bg, None, meta(true, bg_roman));
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// `Word` or `syllable`: pieces are read.
    Timed,
    /// `Line`: pieces, if any, are ignored (a label never adds timing).
    Line,
    /// `None`: no timing at all.
    Unsynced,
}

fn kind(label: &str) -> Kind {
    let label = label.trim();
    if label.eq_ignore_ascii_case("word") || label.eq_ignore_ascii_case("syllable") {
        Kind::Timed
    } else if label.eq_ignore_ascii_case("none") {
        Kind::Unsynced
    } else {
        Kind::Line
    }
}

/// A readable name for the upstream source the server reports, for the credit line.
#[must_use]
pub fn upstream_name(raw: &str) -> Option<String> {
    let s = raw.trim();
    let lower = s.to_ascii_lowercase();
    let known = match lower.as_str() {
        "" => return None,
        "apple" | "apple music" => "Apple Music",
        "deezer" => "Deezer",
        "qq" | "qq music" => "QQ Music",
        "spotify" => "Spotify",
        "musixmatch" => "Musixmatch",
        "lyricsplus" | "lyrics+" => "LyricsPlus community",
        _ if lower == "qapple" || (lower.contains("apple") && lower.contains("qq")) => {
            "Apple Music with QQ Music timing"
        }
        _ => {
            let clean: String = s
                .chars()
                .filter(|c| !c.is_control())
                .take(MAX_NAME_CHARS)
                .collect();
            return Some(clean);
        }
    };
    Some(known.to_owned())
}

/// The credit: `LyricsPlus`, the upstream source, and the curator of a user-submitted lyric.
#[must_use]
pub fn credit(source: Option<&str>, curator: Option<&str>) -> LyricsCredit {
    let from = source.and_then(upstream_name);
    LyricsCredit {
        label: from.as_ref().map_or_else(
            || "Lyrics via LyricsPlus".to_owned(),
            |s| format!("Lyrics via LyricsPlus, from {s}"),
        ),
        url: Some(CREDIT_URL.to_owned()),
        by: curator
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .map(|c| c.chars().take(MAX_NAME_CHARS).collect()),
        by_url: None,
        note: from.map(|s| format!("source: {s}")),
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is checked to be a positive count of seconds under a day first"
)]
fn secs_to_ms(secs: Option<f64>) -> Option<u64> {
    let s = secs?;
    (s.is_finite() && s > 0.0 && s < 86_400.0).then(|| (s * 1000.0).round() as u64)
}

fn matched(a: &Answer) -> Option<MatchedMeta> {
    let picked = a
        .processing
        .as_ref()
        .and_then(|p| p.selected_song_metadata.as_ref());
    let title = picked
        .and_then(|p| p.title.clone())
        .or_else(|| a.metadata.title.clone())
        .filter(|t| !t.trim().is_empty())?;
    let artist = picked
        .and_then(|p| p.artist.clone())
        .or_else(|| a.metadata.artist.clone());
    Some(MatchedMeta {
        title,
        artists: artist.as_deref().map(split_artists).unwrap_or_default(),
        album: picked
            .and_then(|p| p.album.clone())
            .or_else(|| a.metadata.album.clone())
            .filter(|al| !al.trim().is_empty()),
        duration_ms: secs_to_ms(picked.and_then(|p| p.duration)),
    })
}

/// Parse a `/v2/lyrics/get` answer. `Ok(None)` when it holds no lyrics.
///
/// # Errors
/// [`LyricsError::Parse`] if the body is not an answer.
pub fn parse_response(body: &str) -> Result<Option<DetailedDoc>, LyricsError> {
    let a: Answer = serde_json::from_str(body).map_err(|e| LyricsError::Parse(e.to_string()))?;
    let kind = kind(&a.kind);
    let mut b = Builder::default();
    let mut dropped = 0usize;
    for line in &a.lyrics {
        dropped += usize::from(b.line(line, kind));
    }
    let mut built: Vec<Built> = b
        .out
        .into_iter()
        .filter(|x| !x.line.text.is_empty())
        .collect();
    if built.is_empty() {
        return Ok(None);
    }
    if dropped > 0 {
        tracing::debug!(
            lines = dropped,
            "LyricsPlus lines with spread timing kept as lines"
        );
    }
    // Stable, so a background line stays after its main line when they start together.
    if kind != Kind::Unsynced {
        built.sort_by_key(|x| x.line.start_ms);
    }
    let granularity = if built.iter().any(|x| x.split) {
        Granularity::Syllable
    } else if built.iter().any(|x| !x.line.words.is_empty()) {
        Granularity::Word
    } else if kind == Kind::Unsynced {
        Granularity::Unsynced
    } else {
        Granularity::Line
    };
    let mut meta: Vec<LineMeta> = built.iter().map(|x| x.meta.clone()).collect();
    if meta.iter().all(|m| *m == LineMeta::default()) {
        meta.clear();
    }
    let source = a
        .metadata
        .source
        .clone()
        .or_else(|| a.processing.as_ref().and_then(|p| p.winner_source.clone()));
    let credit = credit(source.as_deref(), a.metadata.curator.as_deref());
    let doc = LyricsDoc {
        matched: matched(&a),
        lines: built.into_iter().map(|x| x.line).collect(),
        granularity,
        provider: LYRICSPLUS_ID,
        instrumental: false,
    };
    Ok(Some((doc, Some(credit), meta)))
}

/// Whether a 404 body says the miss may be temporary: some source was skipped, timed out or
/// still waiting. A miss where every source answered `BAD` (looked and found nothing), or a
/// body that names no sources, is a real miss.
#[must_use]
pub fn miss_is_temporary(body: &str) -> bool {
    serde_json::from_str::<MissBody>(body)
        .ok()
        .and_then(|m| m.processing)
        .is_some_and(|p| p.sources_status.values().any(|s| s.status != "BAD"))
}
