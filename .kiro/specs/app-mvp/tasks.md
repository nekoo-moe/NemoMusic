# App MVP: tasks

## Tasks
Wave 0 (Kiro)
- [x] 0.1 UI reference research -> `docs/ui-reference.md`. _Requirements: 13_
- [x] 0.2 Install `yt-dlp` for development; record the locator rule. _Requirements: 3, 9_
- [x] 0.3 Core contracts in `nemo-core` (Catalog, ByteSource, StreamOpener, PlayerControl, LyricsService, ImageSource). _Requirements: 2, 3_
- [x] 0.4 Record approved crates in `tech.md` and the egress list in the README. _Requirements: 11_

Wave 1 (parallel agents, one crate each)
- [x] 1.1 `nemo-ytm`: reqwest client, HTTP range `ByteSource`, `HttpGet`, anonymous `Catalog` via `ytmapi-rs`, image fetch with disk cache, `yt-dlp` locator. _Requirements: 2, 3, 9_
- [x] 1.2 `nemo-lyrics`: LRCLIB client, async provider chain with timeouts, SQLite cache, `LyricsService`. _Requirements: 7_
- [x] 1.3 `nemo-audio`: streaming decode from `ByteSource`, AAC and Opus, rubato when needed, `Player` with queue, seek, volume stage, position clock, SponsorBlock skip. _Requirements: 3, 4, 5, 6, 10_ (Opus is not decoded: symphonia 0.6.1 ships no Opus decoder, so YouTube plays the AAC stream, the fallback in `design.md`. See `decoder::supported_stream_codecs`.)
- [x] 1.4 `nemo-ui`: shell, navigation, glass player bar, Now Playing, palette from cover, snapshots, mock backend. _Requirements: 1, 8, 13_

Wave 2
- [x] 2.1 `nemo-ui`: Search, Album, Artist, Playlist, Queue, Lyrics view, Settings. _Requirements: 2, 7, 13_
- [x] 2.2 `nemo-app`: tokio runtime, wiring, logging, config, README egress list. _Requirements: 1, 11_
- [ ] 2.3 `nemo-library`: folder scan, tags, SQLite, local playback entry. _Requirements: 3_ (Done: scan, tags, in-memory search, local playback. Not done: SQLite storage; `rusqlite` for the library is still "proposed - owner to confirm" in `tech.md`.)

Wave 3
- [ ] 3.1 Integration run: end-to-end checks, 10-minute underrun test, snapshot review, polish pass. _Requirements: 5, 12, 13_ (Done: the end-to-end audio example `crates/nemo-app/examples/smoke_play.rs`, snapshot reviews, polish passes. No record of the 10-minute underrun test.)
- [ ] 3.2 README, run guide, known-issues list for the owner. (Done: README with the Windows and Linux run guide. Not done: a known-issues list; UI gaps are listed in `docs/ui-redesign.md` section 7.)

Wave 4 (needs the owner)
- [x] 4.1 Google sign-in (spike and screen). _ADR 0001_
- [ ] 4.2 WASAPI exclusive.
- [ ] 4.3 Live glass tier on the owner's GPU. (Only Tier Low, the pre-blurred cover, is built.)

Owner-requested waves (2026-11-10 and 11), added after the plan above
- [x] 5.1 Full Material 3 Expressive redesign with motion, Google Sans Flex and the sign-in sheet (`docs/ui-redesign.md`). _Requirements: 1, 8, 13_
- [x] 5.2 Signed-in library, liked songs, history, explore, moods and podcasts. _ADR 0001_
- [x] 5.3 Repeat, like, real text fitting and focus rings (`docs/ui-redesign.md` section 6a). _Requirements: 6, 13_
- [x] 5.4 Song-colour background, motion system, daily Home, themes and lyrics source switcher (`wave-ui-brief.md`). _Requirements: 7, 8, 13_
- [x] 5.5 Playback starts in about 0.5 s; playlist pages open again (`perf-notes.md`). _Requirements: 3_
- [x] 5.6 Lyrics quality: syllable timing, wrapped lines, line glide and the song canvas (`wave-lyrics-brief.md`). _Requirements: 7, 8_

## Definition of Done
See section 4 of `.kiro/steering/workflow.md`.
