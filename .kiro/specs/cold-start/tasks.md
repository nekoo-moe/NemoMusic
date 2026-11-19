# Cold start: tasks

## Tasks

- [x] 1. Measure a fresh process: `cold_start` and `cold_net` examples, four new stage lines, BEFORE table in the PR. _Requirements: 1, 2_
- [x] 2. The play path never waits for or fails because of the installer: a resolve waits for a running install (at most 90 s); the stream opener's HTTP client is built once, before the first click; one `yt-dlp --version` warm-up after the first frame. _Requirements: 3, 4_
- [x] 3. Persisted resolved-URL cache (private file, mode 0600, expiry, one check before reuse, wiped on sign-out and with the cache wipe, never logged). _Requirements: 5_
- [x] 4. Start-up warm-up of the last played song and the first Home items (at most 3), precache of two songs. _Requirements: 6_
- [ ] 5. Pointer press and hover hint from the UI. _Requirements: 7_ NOT DONE (2026-11-19): `button` captures the press, so only hover can be seen from a `mouse_area`; the hint needs a cancellable resolve (`warm` cannot be cancelled), a `UiHooks` field, a message and a delay timer in `nemo-ui`, and touches the card views as well as `kit::track_row`. The gain is at most the dwell time (about 150 ms) on a click; the persisted cache, the start-up hints and the two-song precache (tasks 3 and 4) cover the cases that cost seconds.
- [ ] 6. A resting or failed native path starts yt-dlp at once. _Requirements: 8_ NOT DONE: it already does. A resting path goes straight to yt-dlp (`use_primary`), and a refused or failed native answer starts yt-dlp at once (the 1.5 s race only matters for a slow native request). A refusal costs 53-134 ms here (measured); no rest sooner than 5 refusals, because a refusal is per video (fast-start).
- [x] 7. In-process resolver tier: `rquickjs` and the EJS scripts, player store, `tv` and `tv_samsung` clients, tier before `yt-dlp`; offline validation with the EJS vectors (PR `feat/nemo-ytm-js-extractor`). Not validated end to end on the Codespace IP: googlevideo refuses (403) the addresses of these clients here for `yt-dlp` too. _Requirements: 11 to 15_

## Definition of Done

See section 4 of `.kiro/steering/workflow.md`. Per PR: `cargo fmt --all -- --check`, `cargo clippy -p nemo-ytm -p nemo-app --all-targets -- -D warnings` (and `nemo-ui` when touched), `cargo test` for the crates touched.
