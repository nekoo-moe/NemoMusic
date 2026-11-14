## Overview

The user picks an exported file. NemoMusic reads it, searches YouTube Music for each song one at a time, scores the candidates, shows a review screen and saves the result as a local playlist or, when the account-actions design allows it, as a YouTube Music playlist. Files only (owner, 2026-11-14): Exportify CSV and Soundiiz CSV. TuneMyMusic only after its format is checked against a real export; no link import, no Spotify sign-in.

No new crate. The parser, matcher and session live in a module `import` of `nemo-library` (it already owns local playlists). The matcher reaches YouTube Music only through the `Catalog` trait in `nemo-core` (`catalog.rs`, `Catalog::search`), which `nemo-app` injects. `nemo-library` and `nemo-ytm` stay free of each other. The UI reaches the module through a new `nemo-core` trait, `PlaylistImport`.

The CSV reader is hand-written (RFC 4180: quoted fields, doubled quotes, line breaks inside quotes, a UTF-8 BOM). The `csv` crate 1.4.0 was released 2025-10-17, which is more than the 12 months `tech.md` allows, and it is not in `Cargo.lock`. The reader is about 100 lines and takes no dependency. Text matching uses `unicode-normalization` 0.1.26, already in `Cargo.lock` for lyrics.

ISRC check (criterion 25): the YouTube Music search request has no ISRC field (the `search` body is a query text, optional `params` and a continuation token), and the result items carry no ISRC. A `yt-dlp` issue reports that an ISRC as query text returns nothing through `yt-dlp` but works in a browser, and a community CSV importer sends it as text. So the ISRC is used only as search text, tried first, and a hit counts only if title and duration also agree. It is never proof. The implementing task runs a spike with 20 songs; if fewer than half of the ISRC searches return the right song, the constant `ISRC_FIRST` is set to false and the code stays.

## Affected Crates

- `nemo-core`: new file `import.rs` with the types and the `PlaylistImport` trait (below). No I/O.
- `nemo-library`: module `import` (`csv.rs`, `columns.rs`, `normalize.rs`, `score.rs`, `session.rs`). No database: the session lives in memory only.
- `nemo-ui`: the import screens (pick file, progress, review, save). Pacing timer and file dialogs live here.
- `nemo-app`: builds `ImportEngine` with the catalog and injects it as `Arc<dyn PlaylistImport>`; wires saving to the local playlist store and the account playlist editor.
- `nemo-ytm`: no change. Saving to the account uses the playlist editing that `account-actions` adds.

## Data Flow

1. File: the UI reads the chosen file on the UI side with a size cap of 5 MiB and passes the bytes to `PlaylistImport::parse`. The file is never sent anywhere (criterion 1) and the bytes are dropped after parsing.
2. Match: the UI task calls `PlaylistImport::match_next(&mut session)` once, waits the pause (1.5 s, longer after a 429 or backoff), then calls it again. Each call makes one `Catalog::search` request, or two when an ISRC is tried first and fails. Cancel means the UI stops calling; results found so far stay in the session (criterion 9). No thread is owned by the library, so nothing blocks the audio path and no audio buffer is touched.
3. Review: the UI reads `ImportSession::rows()`; "search again" calls `PlaylistImport::research(row, text)`, "pick another" sets the chosen candidate (each row keeps up to 5). A preview plays the candidate `Track` through the normal player (it has a `video_id`).
4. Save: `PlaylistImport::accepted(&session)` returns the tracks in file order, minus skipped rows. `nemo-app` saves them through the local playlist trait (from `local-library`) or the account playlist editor (from `account-actions`).
5. End: closing the review screen, sign-out or app exit drops the session (criteria 20, 21). The only thing kept is the saved playlist.

Score of a candidate: `0.5 x title + 0.3 x artist + 0.2 x duration`. Title and artist similarity is the share of normalized word tokens in common (NFKC, lower case, text in brackets and "feat." parts removed, "remastered" and similar tags removed). Duration is 1.0 within 3 s, falling to 0 at 15 s, and 0.5 when either side is unknown. Confidence: high at 0.85 or more and a duration within 5 s (or unknown), medium at 0.65, low at 0.45, none below. These thresholds are a first guess to be tuned with the owner's sample playlists (Open Question 1).

## Interfaces

```rust
// nemo-core/src/import.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedSong {
    pub title: String, pub artists: Vec<String>, pub album: Option<String>,
    pub duration: Option<Duration>, pub isrc: Option<String>, pub line: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Confidence { None, Low, Medium, High }
pub struct SkippedRow { pub line: u32, pub reason: String }
pub struct MatchCandidate { pub track: Track, pub score: u8 /* 0..=100 */, pub confidence: Confidence }
pub enum RowState { Pending, Matched, Skipped }
pub struct ImportRow {
    pub song: ImportedSong, pub candidates: Vec<MatchCandidate>,
    pub chosen: Option<usize>, pub state: RowState,
}
pub struct ImportSession {
    pub name: String, pub rows: Vec<ImportRow>, pub skipped: Vec<SkippedRow>, /* private cursor */
}
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportError {
    #[error("the file is not valid UTF-8 text")] NotText,
    #[error("the file is empty")] Empty,
    #[error("the file is larger than {limit} bytes")] TooLarge { limit: u64 },
    #[error("no title column found")] NoTitleColumn,
    #[error("this file format is not supported")] Unsupported,
    #[error("search failed: {0}")] Search(CatalogError),
}
pub trait PlaylistImport: Send + Sync {
    fn parse(&self, file_name: &str, bytes: &[u8]) -> Result<ImportSession, ImportError>;
    /// Match the next pending row with one human-paced search. `Ok(false)` when none is left.
    fn match_next<'a>(&'a self, session: &'a mut ImportSession) -> BoxFuture<'a, Result<bool, ImportError>>;
    fn research<'a>(&'a self, session: &'a mut ImportSession, row: usize, text: &'a str)
        -> BoxFuture<'a, Result<(), ImportError>>;
    fn accepted(&self, session: &ImportSession) -> Vec<Track>;
    fn unmatched_text(&self, session: &ImportSession) -> String;
}

// nemo-library/src/import/mod.rs
pub struct ImportEngine { /* Arc<dyn Catalog> */ }
impl ImportEngine { pub fn new(catalog: Arc<dyn Catalog>) -> Self; }
pub const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
pub const ISRC_FIRST: bool = true;
```

Needed from other designs (names are guesses): a local playlist store trait in `nemo-core` (`local-library`: create a playlist from tracks); a playlist editor trait (`account-actions`: create a playlist and add videos, reporting how many were added when it fails part way, criterion 18).

## Error Handling

Parse errors map to the plain texts of criterion 5; nothing is imported. A row with no title is listed in `skipped` with its line number and "no title". A failed search keeps the row `Pending` and returns `ImportError::Search`; on `CatalogError::Network` the UI pauses and offers retry, on 429-like errors it doubles the pause up to 60 s. A save to the account that fails part way returns the count added; the UI offers a local copy and a retry (criterion 18). Song titles and file text never go to a log; only counts do.

## Testing Strategy

- CSV: quoted commas, doubled quotes, line breaks in a field, BOM, CRLF, a ragged row, empty file, binary file, 5 MiB limit.
- Columns: Exportify and Soundiiz headers by name in any order, missing optional columns, missing title; Soundiiz `duration` in seconds, milliseconds or `m:ss`.
- Score: table tests for "Song (feat. X)" vs "Song", remaster tags, wrong artist, duration 20 s off, unknown duration; sorting low and none first; no pre-selected `None`.
- Session: a fake `Catalog` counts requests (one per `match_next`, two with an ISRC miss), cancel keeps results, skip, research, `accepted` keeps file order.
- A property test: `parse` never panics on random bytes.
- Audio is untouched, so `cargo test -p nemo-audio --test bit_exact` only has to keep passing. No real-time path changes.
- Manual: a real Exportify export and a Soundiiz export (fixtures must hold no personal data); the ISRC spike.

## Risks and Alternatives

- The export formats are partly unverified: the Exportify header for duration may read "Duration (ms)" or "Track Duration (ms)", and its artist separator is unverified. The reader accepts both duration headers and splits artists on `;`. Soundiiz duration units are unverified.
- Wrong matches on remixes, live versions and common titles. Mitigation: low and none first, no pre-selection of none, review before saving.
- Volume: 1,000 songs at one request per 1.5 s is about 25 minutes. The design sets no hard limit, shows a notice above 500 songs with the time estimate, and cancel keeps results. The owner may prefer a hard limit.
- Alternative rejected: the `csv` crate (older than 12 months). Alternative rejected: a new `nemo-import` crate (ADR 0004 prefers a module when the code is small and uses only `nemo-core` traits). Alternative rejected: matching inside `nemo-ytm` (it would own a playlist concept that belongs with local playlists).
- Departure from ADR 0004: none. `PlaylistImport` is a new `nemo-core` trait the ADR does not list.

## Steering and README changes

- `tech.md`: no new crate row. Note that `csv` 1.4.0 was rejected for age (released 2025-10-17) and that `unicode-normalization` 0.1.26 is reused by `nemo-library`.
- `structure.md`: `nemo-library` gains the `import` module and depends on no new crate.
- README egress list: no new host. State that file import contacts only `music.youtube.com`.
- ADR 0004: add `PlaylistImport` and its types to Decision 2's expected additions.

<!-- Sources:
crates/nemo-core/src/catalog.rs (Catalog::search, SearchResults), crates/nemo-core/src/track.rs, .kiro/steering/{tech,structure}.md
.kiro/specs/playlist-import/requirements.md, docs/adr/0004-feature-wave-crates.md
https://github.com/yt-dlp/yt-dlp/issues/15389 (ISRC query: no result in yt-dlp, works in a browser)
https://github.com/shkz-x/spotify-To-youtube (CSV importer that searches with title, artist, album and ISRC)
https://github.com/watsonbox/exportify, https://support.soundiiz.com/hc/en-us/articles/38063068900754
https://crates.io/crates/csv (1.4.0, Unlicense OR MIT, 2025-10-17, rust-version 1.73)
-->
