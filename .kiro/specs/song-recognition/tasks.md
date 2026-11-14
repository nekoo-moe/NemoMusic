## Tasks

Cross-spec ids written `<spec>#?` point at the task in that spec that adds the named ADR 0004 Decision 4 row; fill in the number when that spec's `tasks.md` lands.

- [ ] 1. Spike: does the service answer without a location or time zone
  - Crate and files: none merged. A throwaway probe outside the repo; the result goes into `.kiro/specs/song-recognition/design.md` (Risks and Alternatives).
  - Does: Builds one signature of a short clip the owner supplies (with SongRec 0.8.2's own fingerprint command, not its request code) and posts it to `amp.shazam.com` with only `signature.uri`, `signature.samplems`, `signature.timestamp` and `timestamp`, no `geolocation` and no `timezone`, the real time, `User-Agent: NemoMusic/<version>`, no cookies. Also tries path segments other than `en/US/android`.
  - Pass: a match comes back, so the plan stands. Fail (refusal, empty answer or a block of the real User-Agent): stop, raise it to the owner, and hold tasks 2 to 17; the feature stays unavailable and nothing false is sent (criterion 7). If only `en/US/android` works, it stays a constant in task 10 and the owner is told.
  - _Requirements: 7, 25_
  - Depends on: none
  - Wave: 1
  - Checks: needs the owner: the test clip, and consent to the one live request (the host is approved, ADR 0004).
  - Steering: none

- [ ] 2. Add the Recognizer trait and its types to nemo-core
  - Crate and files: `nemo-core`: new `src/recognize.rs`, `src/lib.rs`. Touches nemo-core/src/recognize.rs and nemo-core/src/lib.rs.
  - Does: Adds `InputSource`, `RecognizedSong` (no cover field), `RecognitionState`, `CancelFlag`, `HistoryEntry`, `HistorySource`, `RecognizeError` and the `Recognizer` trait, signatures as in the design (ADR 0004 row 40). No I/O, no runtime.
  - _Requirements: none (groundwork)_
  - Depends on: song-recognition#1 (pass)
  - Wave: 0
  - Checks: `CancelFlag` unit test; `RecognizeError` `Display` holds no URL or fingerprint.
  - Steering: none (ADR 0004 already lists row 40).

- [ ] 3. Create the nemo-recognize crate skeleton
  - Crate and files: new `crates/nemo-recognize/` (`Cargo.toml`, `src/lib.rs`), workspace member entry.
  - Does: Adds the crate with `#![forbid(unsafe_code)]`, `#![warn(missing_docs)]`, `[lints] workspace = true` and its own tokio runtime holder. Depends on `nemo-core` only.
  - _Requirements: none (groundwork)_
  - Depends on: song-recognition#2
  - Wave: 0
  - Checks: `cargo tree -p nemo-recognize --edges normal` shows no other `nemo-*` crate.
  - Steering: `structure.md` adds `nemo-recognize` to the layout, the ownership list and the graph under `nemo-app`.

- [ ] 4. Port the signature format and the Hanning window from SongRec
  - Crate and files: `nemo-recognize`: `src/fingerprint.rs`, `src/fingerprint/hanning.rs`, `src/fingerprint/signature_format.rs`, `NOTICE`.
  - Does: Ports `hanning.rs` and `signature_format.rs` with an SPDX `GPL-3.0-or-later` header, records SongRec, its author and the ported commit in `NOTICE`. Little-endian packing uses `u32::from_le_bytes`; no `byteorder`. Encodes the data URI with `crc32fast` and `base64`.
  - _Requirements: none (groundwork)_
  - Depends on: song-recognition#3
  - Wave: 1
  - Checks: round-trip test of encode and decode; property test (`proptest`) on the binary format parser; a fixture signature decoded from SongRec at the ported commit.
  - Steering: `tech.md` rows for `crc32fast` =1.5.5 and `base64` at the locked version, and the note that the ported code is GPL-3.0+.

- [ ] 5. Port the fingerprint algorithm from SongRec
  - Crate and files: `nemo-recognize`: `src/fingerprint/algorithm.rs`, `tests/golden_signature.rs`, fixture under `crates/nemo-recognize/tests/fixtures/`.
  - Does: Ports `algorithm.rs` (`SignatureGenerator::make_signature_from_buffer`, FFT through `realfft`) and exposes `make_signature(mono_16khz)`. `communication.rs` is not ported.
  - _Requirements: none (groundwork)_
  - Depends on: song-recognition#4
  - Wave: 1
  - Checks: golden test: a fixed synthetic 12 s buffer gives the peak list SongRec's own code gives at the ported commit.
  - Steering: `tech.md` rows for `realfft` =3.5.1 and `rustfft` =6.4.1 (no exception, ADR 0004 Decision 5).

- [ ] 6. Store the recognition history
  - Crate and files: `nemo-recognize`: `src/history.rs`, migrations for `recognition.sqlite3` in the app data folder.
  - Does: One table `history(id, title, artist, recognized_at, source)`, WAL, text only and no cover. Adds an entry, keeps at most 500 and drops the oldest, lists newest first, deletes one and clears all. Nothing deletes it on sign-out or when the feature is turned off.
  - _Requirements: 16, 17, 18, 19_
  - Depends on: song-recognition#3
  - Wave: 1
  - Checks: tests: cap at 500, newest first, delete one, clear all, migration from empty, history kept after a simulated sign-out.
  - Steering: `tech.md` notes `rusqlite` 0.41.1 (as locked) for `nemo-recognize`.

- [ ] 7. Capture a microphone into a ring buffer
  - Crate and files: `nemo-recognize`: `src/capture.rs`, `src/capture/device.rs`, `tests/rt_no_alloc.rs`.
  - Does: A capture thread owns a `cpal` input stream opened only when a run starts and dropped when it ends. The `/// RT-SAFE` callback only converts to f32 mono and pushes into an `rtrb` ring of 12 s at the device rate: no allocation, lock, log or blocking call (`code.md` real-time rules). Lists microphones first and maps "no device" and "access denied" to `NoInputDevice` and `PermissionDenied`.
  - _Requirements: 3, 4, 13, 22_
  - Depends on: song-recognition#3
  - Wave: 1
  - Checks: counting-allocator test: zero allocations in 10 s of simulated callbacks; worst-case callback time in a `criterion` bench. Capture is a separate input stream and never touches the output path.
  - Steering: `tech.md` notes `cpal` and `rtrb` at the versions `nemo-audio` pins, used by `nemo-recognize`.

- [ ] 8. Capture this computer's sound (loopback)
  - Crate and files: `nemo-recognize`: `src/capture/loopback.rs`.
  - Does: Offers `InputSource::ComputerSound` only where the OS can capture the output: on Windows the default output device opened as an input (WASAPI loopback), on Linux its monitor source. Checks whether cpal 0.18.2 does this; if not on Windows, uses `wasapi` 0.27.0 (approved). The same RT rules hold.
  - _Requirements: 14, 15, 22_
  - Depends on: song-recognition#7
  - Wave: 1
  - Checks: counting-allocator test for the loopback callback; manual check on Windows 11 and Linux that playback and the signal-path view are unchanged during capture.
  - Steering: `tech.md` notes `wasapi` 0.27.0 for `nemo-recognize` only if it is used.

- [ ] 9. Turn the captured audio into signatures
  - Crate and files: `nemo-recognize`: `src/worker.rs`.
  - Does: A worker thread drains the ring, resamples to 16 kHz mono with `rubato`, and keeps one `Vec<f32>` of at most 12 s, the only copy of the audio. At 5 s, 9 s and 12 s it builds a signature from the whole window. A cancel, result or error drops the input stream first, then the buffer; nothing goes to disk or logs.
  - _Requirements: 4, 5, 15, 24_
  - Depends on: song-recognition#5, song-recognition#7
  - Wave: 2
  - Checks: fake input source: at most 12 s kept, signatures at 5, 9 and 12 s, stream dropped before the result, no file created in the data or temp folder during a run.
  - Steering: `tech.md` notes `rubato` at the version `nemo-audio` pins, used by `nemo-recognize`.

- [ ] 10. Send the recognition request
  - Crate and files: `nemo-recognize`: `src/request.rs`, `tests/live_service.rs` (`#[ignore]`).
  - Does: `request_body` holds only `signature.uri`, `signature.samplems`, `signature.timestamp` and `timestamp`, all real; the path has fresh `uuid` v4 ids per request; headers `User-Agent: NemoMusic/<version>` and `Content-Type: application/json`; the `reqwest` cookie store stays off. Parses the three fields used with `serde_json` and maps offline, unreachable, 429, other refusals and unknown shapes to `RecognizeError`, with no retry.
  - _Requirements: 6, 7, 11, 12, 25_
  - Depends on: song-recognition#1 (result), song-recognition#4
  - Wave: 2
  - Checks: tests: no `geolocation` or `timezone` key, the User-Agent, no cookie header; a local test server returns a match, no match, 429 and a broken body. The `#[ignore]` live test runs by hand only.
  - Steering: README egress line `amp.shazam.com` (an audio fingerprint, only when the user starts a recognition); `tech.md` notes `uuid` 1.29.2 with `v4` (as locked).

- [ ] 11. Implement ShazamRecognizer
  - Crate and files: `nemo-recognize`: `src/recognizer.rs`.
  - Does: Implements `Recognizer`: `sources`, `state` (Listening with seconds, Matching, Done), `recognize` with at most 3 requests at least 4 s apart, "No match" after the third, a 10 s cool-down after a run, and a history entry with the source on a match.
  - _Requirements: 3, 4, 11, 16, 24_
  - Depends on: song-recognition#6, song-recognition#8, song-recognition#9, song-recognition#10
  - Wave: 2
  - Checks: tests with the fake input and the local test server: 3-request cap, 4 s spacing, 10 s cool-down, cancel closes the stream at once, no request before `recognize` is called.
  - Steering: none

- [ ] 12. Wire recognition into nemo-app
  - Crate and files: `nemo-app`: `src/backend.rs`, `src/ui_bridge.rs`, a new `src/recognize_wiring.rs` if `backend.rs` would pass 500 lines.
  - Does: Builds `ShazamRecognizer` only when the switch is on and `confirmed_version == version`, and hands the UI an `Arc<dyn Recognizer>`; turning it off cancels a run. After a match, runs one `Catalog::search("<title> <artist>")` and takes the first song whose normalized title and first artist are equal for the cover, else a placeholder; "Find in NemoMusic" lists catalog and library matches and plays only what the user picks. Registers no sign-out step.
  - _Requirements: 1, 8, 9, 10, 18, 19, 25, 26_
  - Depends on: song-recognition#11, integrations#? (row 43: `OptInInfo`)
  - Wave: 2
  - Checks: counting fake host: zero requests and no input stream while the switch is off or unconfirmed; no image request to any recognition-service host; cover matching tests.
  - Steering: none

- [ ] 13. Add the recognition entry to settings
  - Crate and files: `nemo-recognize` (`OptInInfo` for recognition), `nemo-ui`: `src/gui/views/settings.rs`.
  - Does: Shows the `OptInRow`: sends an audio fingerprint, its length, the time, the app name and version and the IP address, never the recording, then the matched title and artist as a YouTube Music search; hosts `amp.shazam.com` and `music.youtube.com`; risk: unofficial service, terms may be breached, software patents may apply outside the EU. The switch stays off until confirmed.
  - _Requirements: 1, 2, 23, 26_
  - Depends on: song-recognition#12, integrations#? (`OptInRow` in nemo-ui)
  - Wave: 3
  - Checks: needs the owner to approve the risk wording (open question 1). The `nemo-app` test comparing `OptInInfo.hosts` with the README egress list passes. `design.md` checklist with a screenshot.
  - Steering: none

- [ ] 14. Add the Recognize button and the Listening state
  - Crate and files: `nemo-ui`: new `src/gui/views/recognize.rs`, `src/keys.rs`.
  - Does: A button and a shortcut start a run; a second press of the shortcut or Cancel ends it. Shows "Listening" with seconds and a Cancel button; under reduced motion a static indicator and a text label replace the animation. A source picker lists microphones first, and "This computer's sound" only where offered, with a one-time confirmation that it listens to everything this computer plays until the result.
  - _Requirements: 3, 14, 15, 20, 21_
  - Depends on: song-recognition#12
  - Wave: 3
  - Checks: headless view tests: Listening, reduced motion, loopback confirmation; `design.md` checklist with a screenshot; visible focus on every control.
  - Steering: none

- [ ] 15. Show the result card and Find in NemoMusic
  - Crate and files: `nemo-ui`: `src/gui/views/recognize.rs`, `src/copy.rs`.
  - Does: Shows title, artist and the YouTube Music cover or a placeholder, and "Find in NemoMusic", which lists catalog and library matches and plays only the one picked. Shows "No match" with Try again, "not in the catalog", and short messages for offline, unreachable and rate limited, and for a missing device or denied access with the path to the Windows or Linux setting.
  - _Requirements: 8, 9, 10, 11, 12, 13_
  - Depends on: song-recognition#14
  - Wave: 3
  - Checks: headless view tests per state; `design.md` checklist with a screenshot.
  - Steering: none

- [ ] 16. Add the recognition history view
  - Crate and files: `nemo-ui`: new `src/gui/views/recognize_history.rs`.
  - Does: Lists entries newest first with title, artist, time and source, and offers "Find in NemoMusic", delete one and clear all.
  - _Requirements: 17_
  - Depends on: song-recognition#15
  - Wave: 3
  - Checks: headless view test; keyboard order; `design.md` checklist with a screenshot.
  - Steering: none

- [ ] 17. Manual checks
  - Crate and files: none (results in the PR description).
  - Does: Recognizes a song from a microphone and from this computer's sound on Windows 11 and on Linux, while NemoMusic plays and while it is idle. Checks the denied-microphone path, that the signal-path view stays accurate during capture, and that no file appears in the data folder.
  - _Requirements: 13, 14, 22_
  - Depends on: song-recognition#13, song-recognition#16
  - Wave: 3
  - Checks: needs the owner (microphone, Windows and Linux machines). `cargo test -p nemo-audio --test bit_exact` passes.
  - Steering: none

## Definition of Done

Every task meets [section 4 of `workflow.md`](../../steering/workflow.md#4-definition-of-done). Also for this spec:
- Task 1 passed and its result is in the design before any other task merges.
- No captured audio reaches disk, logs or the network (tests of tasks 9 and 10), and the request body has no location or time zone.
- The capture callbacks pass the counting-allocator test; `nemo-audio` is untouched and `cargo test -p nemo-audio --test bit_exact` passes.
