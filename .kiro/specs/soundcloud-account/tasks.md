## Tasks

- [x] 1. Sign-in, account state and sign-out (PR 1)
  - Crate and files: `nemo-signin`, `nemo-core/src/source_account.rs`, `nemo-soundcloud` (`account.rs`, `auth.rs`, `signin.rs`, `wipe.rs`, `client.rs`, `http.rs`), `nemo-app` (`soundcloud.rs`, `backend.rs`, `paths.rs`).
  - Does: criteria 1 to 6, 17.
  - _Requirements: 1, 2, 3, 4, 5, 6, 17_
  - Depends on: soundcloud-full#6
  - Checks: the tests named in design.md; `cargo fmt`, clippy and tests of `nemo-signin`, `nemo-core`, `nemo-soundcloud`, `nemo-app`.
  - Steering: `product.md` Scope line for `soundcloud` (labelled).

- [x] 2. Settings row, account card, setup Sources step (PR 2)
  - Crate and files: `nemo-ui` (settings, setup, copy), `nemo-app` (bridge).
  - Does: criteria 7 to 10. The switch is stored by `nemo-app` (`Settings.opt_ins`) and read by the UI through `UiSettings.soundcloud`; turning it on shows `Restart now` because the services start with the app; turning it off stops the client at once (`ScClient::set_enabled`).
  - _Requirements: 7, 8, 9, 10_
  - Depends on: 1
  - Steering: `design.md` setup-flow line and a SoundCloud screens section (labelled).

- [x] 3. Account library reads (PR 3)
  - Crate and files: `nemo-soundcloud` (`library.rs`), `nemo-core` mapping.
  - Does: criteria 11 and 12. Reads of likes, the library (own and liked playlists and albums), followings, the feed and the history, one page at a time; the `Catalog` methods read up to four pages. The Refresh button and the refresh after sign-in are wired with the screens (PR 4), which call these reads.
  - _Requirements: 11, 12_
  - Depends on: 1

- [ ] 4. Source switch and SoundCloud library screen (PR 4)
  - _Requirements: 13, 14, 15_
  - Depends on: 2, 3

- [ ] 5. Playlist, user and track pages (PR 5)
  - _Requirements: 13, 14, 15_
  - Depends on: 4

- [ ] 6. SoundCloud search results with filter chips (PR 6)
  - _Requirements: 13, 14, 15_
  - Depends on: 4

- [ ] 7. Account writes (PR 7)
  - Crate and files: `nemo-soundcloud` (`write.rs`), `nemo-ui`.
  - _Requirements: 16, 17_
  - Depends on: 5, 6; the web app's request bodies read first.

## Definition of Done

Every task meets [section 4 of workflow.md](../../steering/workflow.md#4-definition-of-done).
