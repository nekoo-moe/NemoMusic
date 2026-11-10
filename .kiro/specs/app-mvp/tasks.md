# App MVP: tasks

## Tasks
Wave 0 (Kiro)
- [x] 0.1 UI reference research -> `docs/ui-reference.md`. _Requirements: 13_
- [x] 0.2 Install `yt-dlp` for development; record the locator rule. _Requirements: 3, 9_
- [x] 0.3 Core contracts in `nemo-core` (Catalog, ByteSource, StreamOpener, PlayerControl, LyricsService, ImageSource). _Requirements: 2, 3_
- [x] 0.4 Record approved crates in `tech.md` and the egress list in the README. _Requirements: 11_

Wave 1 (parallel agents, one crate each)
- [ ] 1.1 `nemo-ytm`: reqwest client, HTTP range `ByteSource`, `HttpGet`, anonymous `Catalog` via `ytmapi-rs`, image fetch with disk cache, `yt-dlp` locator. _Requirements: 2, 3, 9_
- [ ] 1.2 `nemo-lyrics`: LRCLIB client, async provider chain with timeouts, SQLite cache, `LyricsService`. _Requirements: 7_
- [ ] 1.3 `nemo-audio`: streaming decode from `ByteSource`, AAC and Opus, rubato when needed, `Player` with queue, seek, volume stage, position clock, SponsorBlock skip. _Requirements: 3, 4, 5, 6, 10_
- [ ] 1.4 `nemo-ui`: shell, navigation, glass player bar, Now Playing, palette from cover, snapshots, mock backend. _Requirements: 1, 8, 13_

Wave 2
- [ ] 2.1 `nemo-ui`: Search, Album, Artist, Playlist, Queue, Lyrics view, Settings. _Requirements: 2, 7, 13_
- [ ] 2.2 `nemo-app`: tokio runtime, wiring, logging, config, README egress list. _Requirements: 1, 11_
- [ ] 2.3 `nemo-library`: folder scan, tags, SQLite, local playback entry. _Requirements: 3_

Wave 3
- [ ] 3.1 Integration run: end-to-end checks, 10-minute underrun test, snapshot review, polish pass. _Requirements: 5, 12, 13_
- [ ] 3.2 README, run guide, known-issues list for the owner.

Wave 4 (needs the owner)
- [ ] 4.1 Google sign-in (spike and screen). 4.2 WASAPI exclusive. 4.3 Live glass tier on the owner's GPU.

## Definition of Done
See section 4 of `.kiro/steering/workflow.md`.
