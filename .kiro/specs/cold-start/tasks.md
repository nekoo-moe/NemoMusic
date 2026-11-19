# Cold start: tasks

## Tasks

- [x] 1. Measure a fresh process: `cold_start` and `cold_net` examples, four new stage lines, BEFORE table in the PR. _Requirements: 1, 2_
- [x] 2. The play path never waits for or fails because of the installer: a resolve waits for a running install (at most 90 s); the stream opener's HTTP client is built once, before the first click; one `yt-dlp --version` warm-up after the first frame. _Requirements: 3, 4_
- [ ] 3. Persisted resolved-URL cache (private file, mode 0600, expiry, one check before reuse, wiped on sign-out and with the cache wipe, never logged). _Requirements: 5_
- [ ] 4. Start-up warm-up of the last played song and the first Home items (at most 3), precache of two songs. _Requirements: 6_
- [ ] 5. Pointer press and hover hint from the UI. _Requirements: 7_
- [ ] 6. A resting or failed native path starts yt-dlp at once. _Requirements: 8_
- [ ] 7. Only if the numbers show yt-dlp is still the slow path: in-process native extractor with `rquickjs` and a local cipher solver (own PR; `tech.md` row and README host added in that PR). _Requirements: 10_

## Definition of Done

See section 4 of `.kiro/steering/workflow.md`. Per PR: `cargo fmt --all -- --check`, `cargo clippy -p nemo-ytm -p nemo-app --all-targets -- -D warnings` (and `nemo-ui` when touched), `cargo test` for the crates touched.
