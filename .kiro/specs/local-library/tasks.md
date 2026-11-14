## Tasks

Cross-spec ids written `<spec>#?` name the owner's task by its ADR 0004 Decision 4 row; replace `?` with the task number once that spec's `tasks.md` has landed. Tasks 1, 2, 3, 6 and 7 are groundwork other specs depend on (rows 14, 15, 16).

- [x] 1. Add the local playback types to `nemo-core`
  - Crate and files: `nemo-core`: `src/library.rs` (new), `src/lib.rs`. Touches nemo-core/src/library.rs.
  - Does: Adds `TrackSpan`, `ReplayGainMode`, `ReplayGainValues`, `LocalPlayback`, `LocalTrackLookup`, `Loudness`, `LoudnessMeter` and `LoudnessError`, signatures as in the design, with rustdoc. `LocalTrackLookup` is documented as "prepare thread only, never the audio callback". No I/O.
  - _Requirements: none (groundwork)_
  - Depends on: none
  - Wave: 0
  - Checks: unit tests for `Default` values (no span, no gain). Used by nemo-audio `prepare` (row 16).
  - Steering: ADR 0004: add `library.rs` to the expected `nemo-core` additions and record that folder watching was not built (design, "Departure").

- [ ] 2. Add the library service types to `nemo-core`
  - Crate and files: `nemo-core`: `src/library.rs`. Touches nemo-core/src/library.rs.
  - Does: Adds `LocalTrackDetails`, `TrackStatus`, `UnplayableReason`, `LocalPlaylistId`, `PlaylistEntry` (path or streamed `Track`), `LocalPlaylist`, `M3uImport`, `TagEdit`, `TagDiff`, `ScanEvent` (including `Rebuilt`), `ScanSummary`, `ScanReceiver`, `LibraryError` (`thiserror`: `Db`, `Io`, `Tag`, `Cue`, `M3u`, `Corrupt`, `Busy`) and the `LocalLibraryService` trait with rename, reorder, delete and entry add and remove. Error texts carry no path or tag.
  - _Requirements: none (groundwork)_
  - Depends on: local-library#1
  - Wave: 0
  - Checks: unit test that `LibraryError` display strings contain no path. Used by playlist-import and listening-stats (`create_playlist`, `add_tracks`).

- [x] 3. Make `FileSource` the shared file reader
  - Crate and files: `nemo-audio`: `src/source.rs`, `tests/file_source.rs` (new), fixtures in `tests/fixtures/`.
  - Does: Documents `FileSource` as the one `ByteSource` for local and cached files (row 15), with typed open errors that hold no path. Adds tests that `StreamDecoder::open(FileSource, None)` probes an AAC-in-MP4 file and an existing FLAC file with no container hint, and that seek and length match the file.
  - _Requirements: none (groundwork)_
  - Depends on: none
  - Wave: 0
  - Checks: `cargo test -p nemo-audio --test bit_exact` (audio path). Fixture under 200 KB with its licence in `fixtures/README.md`. Used by offline-cache. If soundcloud's `ByteSource::is_seekable` (row 10) has landed, `FileSource` returns true; otherwise that soundcloud task adds it.

- [x] 4. Spike: choose the Opus decoder
  - Crate and files: `nemo-audio`: a throwaway test on a spike branch (`tests/opus_spike.rs`); only the result lands, in `.kiro/specs/local-library/design.md` (Risks).
  - Does: Must prove three things. (a) The pure-Rust `opus-decoder` 0.1.1 decodes every RFC 8251 test vector and passes `opus_compare`, wrapped as a `symphonia` decoder. (b) `symphonia-format-ogg` 0.6.1 demuxes Ogg Opus and the `mkv` reader demuxes WebM Opus, with pre-skip applied. (c) It builds on the workspace `rust-version` 1.97 with an allowed licence.
  - _Requirements: 9_
  - Depends on: none
  - Wave: 1
  - Checks: pass: (a), (b) and (c) hold, so task 6 uses `opus-decoder` and needs no C toolchain. Fail on (a) or (c): task 6 uses `symphonia-adapter-libopus` =0.3.0 (CMake and a C compiler, owner accepted 2026-11-14), and task 6 adds the libopus licence notice and the CMake build note. Fail on (b) for Ogg: stop and ask the owner before task 6, because criterion 9 names Ogg Opus. Test vectors are downloaded to scratch, not committed. No owner needed unless (b) fails.

- [x] 5. Decode ALAC, Vorbis and AIFF
  - Crate and files: `nemo-audio`: `Cargo.toml` (`symphonia` features `alac`, `vorbis`, `ogg`, `aiff`), `src/decoder.rs` (`codec_from_id`), `tests/golden/`; `nemo-library`: `Cargo.toml` (same features, so the scan's probe agrees with the player). Touches nemo-audio/src/decoder.rs.
  - Does: Turns on the four `symphonia` features and maps the new codec ids. Adds ALAC and AIFF fixtures made from the existing WAV fixture and a short Ogg Vorbis fixture, with SHA-256 checksums.
  - _Requirements: 9, 12_
  - Depends on: none
  - Wave: 1
  - Checks: `cargo test -p nemo-audio --test bit_exact`; golden tests: ALAC and AIFF decode to samples identical to the WAV fixture; Vorbis matches a reference decode within a stated tolerance (lossy, not bit-exact).
  - Steering: `tech.md` Decode row: add features `alac`, `vorbis`, `ogg`, `aiff`.
  - Done: `nemo-library` has no `symphonia` dependency and no probe yet, so its `Cargo.toml` was left alone; task 12, which adds the probe, adds `symphonia` with the same features. `scan.rs` `AUDIO_EXTENSIONS` does not list `aiff` or `aif` yet (task 12).

- [x] 6. Add the Opus decoder through one codec registry
  - Crate and files: `nemo-audio`: `Cargo.toml`, `src/codecs.rs` (new, `build_codecs()`), `src/decoder.rs`, `src/stream_decoder.rs`, `tests/golden/`; `nemo-library`: treat the Opus codec id as playable in the scan probe. Touches nemo-audio/src/decoder.rs.
  - Does: Registers the decoder the spike chose next to the default `symphonia` decoders behind `build_codecs()`, so `StreamDecoder` keeps one code path and either decoder can be swapped. Maps `Codec::Opus`. Adds Ogg Opus and WebM Opus fixtures.
  - Spike result (task 4, design Risks): `opus-decoder` failed the mono half of the RFC 8251 vectors, so this task uses the fallback `symphonia-adapter-libopus` =0.3.0. The adapter drops the OpusHead pre-skip itself; the fixtures check the decoded length for Ogg and WebM.
  - _Requirements: 9_
  - Depends on: local-library#4, local-library#5
  - Wave: 1
  - Checks: `cargo test -p nemo-audio --test bit_exact`; Opus fixtures match a reference decode within a stated tolerance; a failed decoder build plays nothing and logs without paths. Not yet in `supported_stream_codecs()` (task 7).
  - Steering: `tech.md` Opus decode row: the chosen crate (and, for the fallback, `opusic-sys` 0.7.7 BSD-3-Clause and the CMake need), replacing the `opus` or `audiopus` wording, `audiopus` recorded as rejected. README: the libopus licence notice, only if the fallback is chosen.
  - Done: `nemo-library` has no probe yet, so nothing there changed; task 12, which adds the probe, treats the Opus codec id as playable (`.opus` and `.ogg` are already in `AUDIO_EXTENSIONS`, `.webm` is not). `stream_decoder.rs` and `decode_file` both build their decoder through `codecs.rs`. A lossy codec now states no bit depth even when the container does (FFmpeg's WebM muxer writes 16 for Opus). The README also gets the CMake build note.

- [ ] 7. Let streams play as Opus
  - Crate and files: `nemo-audio`: `src/decoder.rs` (`supported_stream_codecs()`). Touches nemo-audio/src/decoder.rs.
  - Does: Adds `Codec::Opus` to `supported_stream_codecs()` (owner approved 2026-11-14), so the YouTube and SoundCloud rankers can pick Opus. Separate PR so the change of stream choice is easy to review and revert.
  - _Requirements: none (groundwork)_
  - Depends on: local-library#6
  - Wave: 1
  - Checks: `cargo test -p nemo-audio --test bit_exact`; a test that a WebM Opus fixture served by the loopback test server plays to the end. Waited on by soundcloud and account-actions rankers and youtube-extras (row 14).

- [ ] 8. Create the `library.db` store with migrations
  - Crate and files: `nemo-library`: `Cargo.toml` (`rusqlite` =0.41.1, bundled), `src/store.rs` (new), `src/lib.rs`.
  - Does: Opens `library.db` in WAL mode and runs migration 1: folders, tracks (path, size, modified time, tags, status and reason, CUE span, gain values), albums (with the `gapless` flag), playlists, playlist entries and loudness. Adds the queries the later tasks use; one write connection, readers open their own.
  - _Requirements: 5, 18_
  - Depends on: local-library#2
  - Wave: 1
  - Checks: migration, reopen and WAL tests on a temp folder.
  - Steering: `tech.md` Library DB row: `rusqlite` 0.41.1 from "proposed" to "decided". `structure.md`: `nemo-library` owns `library.db`.

- [ ] 9. Rebuild a damaged library file
  - Crate and files: `nemo-library`: `src/store.rs`.
  - Does: Runs `PRAGMA quick_check` at open. On failure renames the file and its `-wal` and `-shm` to `library.db.broken-<unix time>`, creates a new file, asks for a rescan and emits one `ScanEvent::Rebuilt` whose text says the kept copy holds the old playlists.
  - _Requirements: 6_
  - Depends on: local-library#8
  - Wave: 1
  - Checks: a truncated file triggers the rebuild and keeps the copy.

- [ ] 10. Load the library from the store at start
  - Crate and files: `nemo-library`: `src/library.rs`, `src/catalog.rs`.
  - Does: Loads every row into an `Arc<RwLock<Snapshot>>` on a worker thread and makes `LocalCatalog` read the snapshot instead of rescanning.
  - _Requirements: 5_
  - Depends on: local-library#8
  - Wave: 1
  - Checks: 50 000 rows (`MAX_FILES`) load within the 1 s budget (release-mode test, ignored in debug).

- [ ] 11. Scan folders incrementally with one writer
  - Crate and files: `nemo-library`: `src/scan.rs`, `src/store.rs`.
  - Does: One scan thread walks the folders (no symlinks, depth 16) and sends `ScanEvent` values to one writer thread, which batches transactions and swaps rows into the snapshot so tracks show as found. A file is re-read only when size or modified time changed; missing files go after the walk; removing a folder removes its tracks and never touches a file.
  - _Requirements: 1, 3, 4, 28_
  - Depends on: local-library#10
  - Wave: 2
  - Checks: temp-folder tests for add, change, remove and folder removal (files on disk unchanged); a log-capture test that no path or tag is written at info level or above. No live watching.

- [ ] 12. Read all tag fields and mark unplayable files
  - Crate and files: `nemo-library`: `src/tags.rs`, `src/scan.rs`, `Cargo.toml` (`symphonia` =0.6.1 with the `nemo-audio` features, moved here from task 5).
  - Does: Reads title, artist, album, album artist, track and disc number, year, genre, duration and embedded cover, with the file name as the title fallback. Probes and decodes the first packet; a file that fails is stored as `Unplayable` with its reason, so "playable" means the player's decoders accept it.
  - _Requirements: 7, 8, 9_
  - Depends on: local-library#11, local-library#6
  - Wave: 2
  - Checks: fixtures with full tags, no tags and a damaged file; the playable set equals the formats of criterion 9.

- [ ] 13. Read ReplayGain, R128 and gapless tags
  - Crate and files: `nemo-library`: `src/replaygain.rs` (new), `src/scan.rs`.
  - Does: Reads ReplayGain track and album gain and peak and R128 gains (converted to the ReplayGain reference) into `ReplayGainValues`. Sets the album `gapless` flag from the iTunes `pgap` tag.
  - _Requirements: 11, 15_
  - Depends on: local-library#12
  - Wave: 2
  - Checks: `proptest` for the gain parsers and the R128 conversion.

- [ ] 14. Parse CUE sheets into tracks
  - Crate and files: `nemo-library`: `src/cue.rs` (new), `src/scan.rs`.
  - Does: Hand-written parser for a CUE file of the same name and an embedded CUE tag: UTF-8 with BOM, Windows-1252 fallback, `INDEX 00` and `01`. Each CUE track is stored as a track with title, artist and a `TrackSpan` of the one audio file; a missing audio file or bad index lists the audio file as one track and records the problem.
  - _Requirements: 8, 22_
  - Depends on: local-library#12
  - Wave: 2
  - Checks: `proptest` on the parser; fixture CUE files for both encodings.

- [ ] 15. Store and edit local playlists
  - Crate and files: `nemo-library`: `src/playlists.rs` (new), `src/store.rs`.
  - Does: Create, rename, reorder and delete playlists, and add or remove entries, each saved in one transaction at once. Entries may be a local path or a streamed `Track` (playlist-import, listening-stats). An entry whose file is gone stays and is flagged missing.
  - _Requirements: 18, 21_
  - Depends on: local-library#8
  - Wave: 2
  - Checks: reopen keeps every change; missing-file entries survive a rescan.

- [ ] 16. Import and export M3U and M3U8
  - Crate and files: `nemo-library`: `src/m3u.rs` (new).
  - Does: Import reads relative and absolute paths (UTF-8 for M3U8), skips lines it cannot resolve and returns `M3uImport` with the skipped count. Export writes UTF-8 M3U8 to the chosen path and reports how many streamed entries were left out.
  - _Requirements: 19, 20_
  - Depends on: local-library#15
  - Wave: 2
  - Checks: `proptest` round trip; fixtures with relative, absolute and broken lines.

- [ ] 17. Edit tags without risking the file
  - Crate and files: `nemo-library`: `src/tag_edit.rs` (new), `src/store.rs`.
  - Does: `preview` returns old and new values as `TagDiff`. `apply` copies to a temp file beside the original, writes with `lofty`, re-reads, replaces the original and updates the row; any failure removes the temp file and leaves the original unchanged with a plain `LibraryError::Tag` reason.
  - _Requirements: 23, 24_
  - Depends on: local-library#12
  - Wave: 2
  - Checks: a read-only file and a held file leave the bytes identical (hash before and after); cover and unknown frames survive an edit.

- [ ] 18. Implement `LocalLibraryService` and `LocalTrackLookup`
  - Crate and files: `nemo-library`: `src/service.rs` (new), `src/lib.rs`.
  - Does: Implements the trait over the store, scan, playlists, M3U and tag edits, and `LocalTrackLookup::playback` on its own read connection (span and gain for the prepare thread).
  - _Requirements: 5, 10, 18_
  - Depends on: local-library#13, local-library#14, local-library#16, local-library#17
  - Wave: 2
  - Checks: service tests over a temp library; `playback` returns the span of a CUE track and `None` for an unknown track.

- [ ] 19. Play CUE spans in the player
  - Crate and files: `nemo-audio`: `src/prepare.rs` (`PrepareDeps.local`), `src/feeder.rs`, `tests/fixtures/` (FLAC with a 3-track CUE). Touches nemo-audio/src/prepare.rs; land before offline-cache#13 and soundcloud's `prepare` branch, one at a time.
  - Does: The prepare thread asks `LocalTrackLookup` for span and gain (never the callback). The feeder seeks to `span.start` with `SeekMode::Accurate` and ends the job at `span.end` by counting frames. Nothing new runs in the `cpal` callback.
  - _Requirements: 10, 22_
  - Depends on: local-library#1
  - Wave: 2
  - Checks: `cargo test -p nemo-audio --test bit_exact`; `rt_no_alloc` (counting allocator) with a local-file run; the three CUE tracks played in a row equal the whole file frame for frame.

- [ ] 20. Join CUE tracks and gapless albums without a fade
  - Crate and files: `nemo-library`: `src/gapless.rs` (new); `nemo-app`: inject the policy.
  - Does: Implements `GaplessPolicy::join_without_fade`: true for consecutive tracks of one CUE file and for consecutive tracks of an album with the `gapless` flag.
  - _Requirements: 11, 22_
  - Depends on: local-library#18, audio-effects#? (row 4, `GaplessPolicy`)
  - Wave: 2
  - Checks: unit tests; `cargo test -p nemo-audio --test bit_exact`; with crossfade on, CUE tracks still join frame for frame.

- [ ] 21. Apply ReplayGain as an off-by-default stage
  - Crate and files: `nemo-audio`: `src/feeder.rs`, `src/prepare.rs`, `src/replay_gain.rs` (new). Touches nemo-audio/src/prepare.rs.
  - Does: When the setting is on and the song has a value, prepare picks track or album gain plus pre-gain, lowers it so peak x gain stays at or below 1.0, and the feeder applies one scalar per block after the song's `Chain` and before the transition mixer, registered as `ProcessingStage::ReplayGain { db }`. Off, or no tag, means the stage is not built: no arithmetic, unity gain, bit-perfect path kept. A change applies from the next song.
  - _Requirements: 15, 16, 17_
  - Depends on: local-library#19, audio-effects#? (row 3, `ProcessingStage::ReplayGain`)
  - Wave: 2
  - Checks: `cargo test -p nemo-audio --test bit_exact` with ReplayGain off; `rt_no_alloc` run with the gain stage on (callback allocates nothing; feeder preallocates); `proptest` for dB-to-scalar and the peak clamp; `is_bit_perfect` false while the stage is in the path. Alters audio only when enabled, as the approved spec says (`workflow.md` section 8).

- [ ] 22. Measure loudness with `ebur128`
  - Crate and files: `nemo-audio`: `Cargo.toml` (`ebur128` =0.1.11), `src/loudness.rs` (new, `AudioMeter`).
  - Does: `AudioMeter` implements `LoudnessMeter`: decodes a file with `StreamDecoder`, feeds `ebur128` and returns integrated loudness and true peak. Checks the cancel flag between blocks.
  - _Requirements: 16_
  - Depends on: local-library#1, local-library#6
  - Wave: 2
  - Checks: a tone generated in the test gives the expected integrated loudness within 0.1 LU (no external fixture needed); cancel stops within one block.
  - Steering: `tech.md` row `ebur128` 0.1.11 (MIT). `structure.md`: `nemo-audio` implements `LoudnessMeter`, `nemo-app` wires it.

- [ ] 23. Run the opt-in loudness scan in the background
  - Crate and files: `nemo-library`: `src/replaygain.rs`, `src/service.rs` (`set_loudness_scan`).
  - Does: While its switch is on (off by default), a low-priority thread measures one untagged file at a time through the injected `LoudnessMeter` and writes the result to the `loudness` table only. Turning it off stops the scan between blocks.
  - _Requirements: 16_
  - Depends on: local-library#18
  - Wave: 2
  - Checks: with a fake meter: file bytes unchanged after the scan (hash); stop is honoured; nothing runs while the switch is off.

- [ ] 24. Wire the library into the app
  - Crate and files: `nemo-app`: `src/backend.rs`, `src/main.rs`.
  - Does: Opens `library.db` in `AppPaths.library` on a worker thread, builds `LocalLibraryService`, passes `LocalTrackLookup` into `PrepareDeps`, passes `AudioMeter` to the loudness scan and queues a rescan after the snapshot shows. Registers no sign-out step.
  - _Requirements: 5, 10, 28, 29_
  - Depends on: local-library#18, local-library#19, local-library#22, local-library#23, account-actions#? (row 26, sign-out registry)
  - Wave: 2
  - Checks: a test that sign-out leaves `library.db` in place; a check that no new host is contacted (no network client in the library path).

- [ ] 25. Keep the lossless badge honest for local formats
  - Crate and files: `nemo-core`: `src/audio.rs`; `nemo-ui`: signal-path view. Touches nemo-core/src/audio.rs.
  - Does: The true-lossless badge shows only for local FLAC, ALAC and WAV on a bit-perfect path, with the real rate and depth; otherwise codec and format with the reason in the signal-path view. Opus, Vorbis, MP3 and AAC never read "lossless"; facts not read show "Unknown". AIFF follows criterion 12 and gets no badge unless the owner adds it.
  - _Requirements: 12, 13, 14_
  - Depends on: local-library#5, local-library#6
  - Wave: 2
  - Checks: unit test that `is_bit_perfect` is false for `OutputMode::Shared` (all `cpal` reports on Linux); table test per codec.

- [ ] 26. Hand opened files to the running app
  - Crate and files: `nemo-app`: `src/single_instance.rs` (new), `src/main.rs`.
  - Does: A second launch with file paths sends them to the running app over a named pipe `\\.\pipe\NemoMusic-<user SID>` limited to the current user (Windows) or a Unix socket in `$XDG_RUNTIME_DIR` with mode 0600 (Linux), then exits; with no running app it starts and plays. No token and no file. The file plays without joining the library unless it lies under a watched folder.
  - _Requirements: 25_
  - Depends on: local-library#24
  - Wave: 2
  - Checks: loopback test of the hand-off; manual on Windows and Linux Mint. `code.md` forbids `unsafe` outside `nemo-audio` `ffi`: use `tokio::net::windows::named_pipe` and std `UnixListener`, or a small crate (licence and `rust-version` checked); if limiting the pipe to the current user needs `unsafe`, stop and ask the owner.
  - Steering: `tech.md` row for the crate, only if one is added.

- [ ] 27. Add the folder list with the system dialog
  - Crate and files: `nemo-ui`: `Cargo.toml` (`rfd` =0.18.0), `src/gui/views/settings.rs`.
  - Does: Settings lists watched folders with add and remove. Add opens the `rfd` folder dialog and falls back to a typed path when the dialog fails (no desktop portal on Linux). Removing a folder says no file is deleted.
  - _Requirements: 1, 2, 4, 27_
  - Depends on: local-library#24
  - Wave: 3
  - Checks: `design.md` checklist with a screenshot; keyboard reach and visible focus; manual on Linux without a portal.
  - Steering: `tech.md` row `rfd` 0.18.0 (MIT, MSRV 1.92), unless the listening-stats task that also needs `rfd` landed it first.

- [ ] 28. Show the library with scan progress and track status
  - Crate and files: `nemo-ui`: `src/gui/views/library.rs`.
  - Does: Tracks appear while the scan runs without blocking the screen; unplayable tracks show their reason on request; missing facts read "Unknown". Scan-list and cover animations follow reduced motion.
  - _Requirements: 1, 8, 14, 27, 30_
  - Depends on: local-library#24
  - Wave: 3
  - Checks: `design.md` checklist with a screenshot; keyboard and focus; reduced-motion snapshot.

- [ ] 29. Add local playlist screens
  - Crate and files: `nemo-ui`: `src/gui/views/local_playlist.rs` (new), `src/gui/views/library.rs`.
  - Does: Create, rename, reorder (keyboard too) and delete local playlists. Missing entries show as missing and playback moves to the next entry.
  - _Requirements: 18, 21, 27_
  - Depends on: local-library#24
  - Wave: 3
  - Checks: `design.md` checklist with a screenshot; a test that a missing entry is skipped in the queue.

- [ ] 30. Add M3U import and export to the playlist screens
  - Crate and files: `nemo-ui`: `src/gui/views/local_playlist.rs`.
  - Does: Import opens an `rfd` file dialog and reports added and skipped lines; export asks where to save the M3U8 and reports streamed entries left out.
  - _Requirements: 19, 20, 27_
  - Depends on: local-library#27, local-library#29
  - Wave: 3
  - Checks: `design.md` checklist with a screenshot; keyboard and focus.

- [ ] 31. Add the tag editor with a confirm step
  - Crate and files: `nemo-ui`: `src/gui/views/tag_editor.rs` (new).
  - Does: Shows old and new values from `preview_tag_edit`, writes only after the user confirms, and shows a plain failure that says the file is unchanged.
  - _Requirements: 23, 24, 27_
  - Depends on: local-library#24
  - Wave: 3
  - Checks: `design.md` checklist with a screenshot; keyboard and focus; neither button pre-selected.

- [ ] 32. Add ReplayGain settings and show the gain stage
  - Crate and files: `nemo-ui`: `src/gui/views/settings.rs`, signal-path view; `nemo-app`: save the settings.
  - Does: ReplayGain switch (off by default), track or album mode, pre-gain, and the separate loudness-scan switch with progress and stop. The signal path shows the ReplayGain stage while it is in the path.
  - _Requirements: 13, 15, 16, 27_
  - Depends on: local-library#21, local-library#23, local-library#24
  - Wave: 3
  - Checks: `design.md` checklist with a screenshot; a test that a new install has both switches off; `cargo test -p nemo-audio --test bit_exact` still passes with defaults.

- [ ] 33. Offer play, queue or playlist for dropped files
  - Crate and files: `nemo-ui`: `src/gui/views/shell.rs` (drop target), `src/gui/views/overlays.rs`.
  - Does: Dropping files or a folder offers "Play now", "Add to queue" or "Add to playlist"; a folder expands to its supported files. Dropped files join the library only when they lie under a watched folder.
  - _Requirements: 25, 26, 27_
  - Depends on: local-library#24, local-library#29
  - Wave: 3
  - Checks: `design.md` checklist with a screenshot; keyboard reach of the offer.

- [ ] 34. Update the README for the local library
  - Crate and files: `README.md`.
  - Does: Says the library, covers and playlists stay on this computer and no new host is contacted, that tag editing writes to the user's files after a confirm, and that the Linux folder dialog uses the desktop portal with a typed-path fallback.
  - _Requirements: 28_
  - Depends on: local-library#27
  - Wave: 3
  - Checks: README egress list unchanged.

- [ ] 35. Run the manual checks on Windows and Linux Mint
  - Crate and files: none (results in the PR description).
  - Does: Checks open-with, command-line paths, drop, second-launch hand-off, the folder dialog without a portal, keyboard reach and focus of every library action, media keys, seek, queue and signal path for local tracks, logs at info and above without paths or tags, and that sign-out keeps the library.
  - _Requirements: 10, 25, 26, 27, 28, 29_
  - Depends on: local-library#26, local-library#28, local-library#29, local-library#30, local-library#31, local-library#32, local-library#33
  - Wave: 3
  - Checks: needs the owner's Linux Mint machine (or the owner) for the Linux half.

## Definition of Done

See [section 4 of `.kiro/steering/workflow.md`](../../steering/workflow.md#4-definition-of-done). For this spec also:
- Every task touching `nemo-audio` runs `cargo test -p nemo-audio --test bit_exact`, which passes with ReplayGain off; the new golden ALAC and AIFF fixtures decode identical to the WAV fixture.
- `cargo test -p nemo-audio --test rt_no_alloc` covers a local-file run and a run with the ReplayGain stage on.
- No path or tag is logged at info level or above; no new network host.
