## Tasks

Cross-spec ids written `<spec>#?` name the task in that spec that adds the ADR 0004 Decision 4 row given in brackets; the number is filled in once that spec's `tasks.md` is committed.

- [ ] 1. Add `import.rs` with the import types and the `PlaylistImport` trait
  - Crate and files: `nemo-core` (new nemo-core/src/import.rs, touches nemo-core/src/lib.rs).
  - Does: `ImportedSong`, `Confidence`, `SkippedRow`, `MatchCandidate`, `RowState`, `ImportRow`, `ImportSession`, `ImportError` and the trait (`parse`, `match_next`, `research`, `accepted`, `unmatched_text`). Signatures only, no I/O.
  - _Requirements: none (groundwork)_
  - Depends on: none
  - Wave: 0
  - Checks: a test that `Confidence` orders `None < Low < Medium < High`.
  - Steering: none

- [ ] 2. Spike: ISRC search and the real export formats
  - Crate and files: none shipped; anonymised fixtures in `crates/nemo-library/tests/fixtures/import/` (no personal data); result written into the Risks section of `.kiro/specs/playlist-import/design.md`.
  - Does: search YouTube Music with the ISRC of 20 songs as query text; check the Exportify duration header ("Duration (ms)" or "Track Duration (ms)") and artist separator, the Soundiiz `duration` unit, and a real TuneMyMusic export.
  - _Requirements: 24, 25_
  - Depends on: none
  - Wave: 1
  - Checks: pass = at least 10 of 20 ISRC searches return the right song, so `ISRC_FIRST` stays true; fail sets it false in task 5 and the code stays. A verified TuneMyMusic export adds its columns in task 3; otherwise task 3 answers `Unsupported` for it. Needs the owner to supply the sample playlists and real exports (requirements Open Question 1).
  - Steering: none

- [ ] 3. Read Exportify and Soundiiz CSV files in nemo-library
  - Crate and files: `nemo-library` (new `import.rs`, `import/columns.rs`), dependency `csv` =1.4.0.
  - Does: strip a UTF-8 BOM, `ReaderBuilder` with `flexible(true)` and `has_headers(true)`, columns found by header name; Exportify and Soundiiz columns, any missing column except title allowed, ragged and title-less rows listed in `skipped` with line and reason; `NotText`, `Empty`, `TooLarge` (5 MiB), `NoTitleColumn`, `Unsupported`. Playlist name from the file name. The bytes are dropped after parsing.
  - _Requirements: 1, 2, 3, 4, 5, 6, 24_
  - Depends on: playlist-import#1, playlist-import#2
  - Wave: 1
  - Checks: tests for quoted commas, doubled quotes, line breaks in a field, BOM, CRLF, ragged row, empty and binary files, the 5 MiB limit, headers in any order, Soundiiz duration in s, ms and `m:ss`; proptest that parsing never panics on random bytes.
  - Steering: `tech.md` row for `csv` =1.4.0 (Unlicense OR MIT, used as MIT; MSRV 1.73; released 2025-10-17; pure Rust; `nemo-library` only). `structure.md`: `nemo-library` gains the `import` module; no new `nemo-*` edge.

- [ ] 4. Normalize text and score candidates
  - Crate and files: `nemo-library` (`import/normalize.rs`, `import/score.rs`), dependency `unicode-normalization` 0.1.26 (already locked).
  - Does: NFKC, lower case, bracketed text, "feat." parts and remaster tags removed; score `0.5 x title + 0.3 x artist + 0.2 x duration`; confidence high at 0.85 with duration within 5 s or unknown, medium at 0.65, low at 0.45, none below (starting values, tuned with the owner's samples).
  - _Requirements: 8_
  - Depends on: playlist-import#1
  - Wave: 1
  - Checks: table tests: "Song (feat. X)" vs "Song", remaster tags, wrong artist, duration 20 s off, unknown duration.
  - Steering: `tech.md` notes `unicode-normalization` is now also a `nemo-library` dependency.

- [ ] 5. Implement `ImportEngine` and the match session
  - Crate and files: `nemo-library` (`import/session.rs`).
  - Does: `match_next` makes one `Catalog::search` (two when the ISRC is tried first and fails; an ISRC hit counts only if title and duration agree); up to 5 candidates per row; `research`; `accepted` keeps file order minus skipped rows; `unmatched_text`. Rows are sorted low and none first and a `None` match is never pre-selected. A failed search keeps the row `Pending`. Owns no thread and logs counts only, never titles.
  - _Requirements: 7, 8, 9, 11, 12, 14, 19, 25_
  - Depends on: playlist-import#3, playlist-import#4
  - Wave: 1
  - Checks: a counting fake `Catalog` (one request per `match_next`, two on an ISRC miss); cancel keeps results; skip; research; file order kept.
  - Steering: none

- [ ] 6. Wire the import engine and saving to a local playlist in nemo-app
  - Crate and files: `nemo-app` (wiring).
  - Does: builds `ImportEngine` with the catalog and injects `Arc<dyn PlaylistImport>`; saves accepted tracks through `LocalLibraryService::create_playlist` and `add_tracks`; sends nothing to YouTube for a local save; returns saved and not-saved counts.
  - _Requirements: 14, 17_
  - Depends on: playlist-import#5, local-library#? (row 16: `LocalLibraryService` playlists with entries that have no file path)
  - Wave: 2
  - Checks: an integration test with a fake library store: file order, skipped rows absent, zero catalog calls during the save.
  - Steering: none

- [ ] 7. Save an import as a YouTube Music playlist
  - Crate and files: `nemo-app` (wiring).
  - Does: when the account playlist editor is built and the user is signed in, `PlaylistEditor::create` with the confirmed name and `Privacy::Private`, then `add_tracks` in batches of 50 with a 1.5 s pause; a part-way failure returns the count added so the UI can offer a local copy and a retry. Otherwise only the local save is offered.
  - _Requirements: 15, 16, 18, 26_
  - Depends on: playlist-import#6, account-actions#? (row 27: `PlaylistEditor::create`)
  - Wave: 2
  - Checks: a fake `PlaylistEditor` failing on batch 2 gives the right count; signed out or no editor offers local only.
  - Steering: none

- [ ] 8. Add the file pick and matching progress screens
  - Crate and files: `nemo-ui` (new import views; pacing timer).
  - Does: "Import playlist" opens a file dialog, reads the file on this computer with the 5 MiB cap and calls `parse`; shows name, song count and skipped rows with reasons; runs `match_next` with a 1.5 s pause, doubled after a 429 or backoff up to 60 s; progress, cancel that keeps results, and a time notice above 500 songs.
  - _Requirements: 1, 5, 6, 7, 9, 22_
  - Depends on: playlist-import#6
  - Wave: 3
  - Checks: `design.md` checklist with screenshot; keyboard-only reach with visible focus; progress has no animation under reduced motion.
  - Steering: uses the file dialog crate in `tech.md`; if `rfd` has no row yet when this task lands, this task adds it.

- [ ] 9. Add the review screen
  - Crate and files: `nemo-ui` (import review view).
  - Does: each source song beside its match with title, artist, duration and confidence, low and none first, none not pre-selected; search again, pick another candidate or skip; a preview plays the candidate through the normal player.
  - _Requirements: 10, 11, 12, 13, 22_
  - Depends on: playlist-import#8
  - Wave: 3
  - Checks: `design.md` checklist with screenshot; keyboard check.
  - Steering: none

- [ ] 10. Add the save step and drop the session afterwards
  - Crate and files: `nemo-ui` (import save view, UI state).
  - Does: local or YouTube Music choice with the name to confirm, the reason when only local is offered, saved and not-saved counts, local copy and retry after a part-way failure, saving the unmatched list as a text file the user names. The session is dropped when the review closes, on sign-out or when `AccountList.generation` changes; nothing else is kept.
  - _Requirements: 14, 15, 16, 17, 18, 19, 20, 21, 22, 26_
  - Depends on: playlist-import#7, playlist-import#9, account-actions#? (row 25: `AccountList.generation`)
  - Wave: 3
  - Checks: `design.md` checklist with screenshot; a UI-state test that sign-out and closing drop the session.
  - Steering: none

- [ ] 11. Document the import and run the manual checks
  - Crate and files: `README.md`.
  - Does: states that file import reads the file on this computer and contacts only `music.youtube.com`, already listed; the owner imports a real Exportify and a real Soundiiz export end to end and confirms no copy of the file or match history stays after the review closes.
  - _Requirements: 20, 23_
  - Depends on: playlist-import#10
  - Wave: 3
  - Checks: needs the owner (real exports, threshold tuning with the sample playlists).
  - Steering: README egress section gains the file-import line; no new host.

## Definition of Done

Every task meets [section 4 of workflow.md](../../steering/workflow.md#4-definition-of-done). For this spec also:
- The file never leaves this computer: tests show `parse` makes no request and only `Catalog::search` and the playlist save reach the network.
- Audio code is untouched; `cargo test -p nemo-audio --test bit_exact` keeps passing.
