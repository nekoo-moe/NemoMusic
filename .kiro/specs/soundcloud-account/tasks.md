## Tasks

- [x] 1. Sign-in, account state and sign-out (PR 1)
  - Crate and files: `nemo-signin`, `nemo-core/src/source_account.rs`, `nemo-soundcloud` (`account.rs`, `auth.rs`, `signin.rs`, `wipe.rs`, `client.rs`, `http.rs`), `nemo-app` (`soundcloud.rs`, `backend.rs`, `paths.rs`).
  - Does: criteria 1 to 6, 17.
  - _Requirements: 1, 2, 3, 4, 5, 6, 17_
  - Depends on: soundcloud-full#6
  - Checks: the tests named in design.md; `cargo fmt`, clippy and tests of `nemo-signin`, `nemo-core`, `nemo-soundcloud`, `nemo-app`.
  - Steering: `product.md` Scope line for `soundcloud` (labelled).

- [x] 1a. Sign-in fix: the window never closed on the owner's Linux build (2026-11-20)
  - Crate and files: `nemo-signin` (`soundcloud.rs`, `token.rs`, `hook.js`, `lib.rs`), `nemo-soundcloud` (`signin.rs`, `account.rs`), `nemo-core` (`source_account.rs`), `nemo-ui` (account card, setup card, copy).
  - Does: criteria 6a to 6c. Verified here against the real soundcloud.com in WebKitGTK (GTK Broadway) without an account: cookie manager, `document.cookie`, probes, the 25 s hint, the `check` line and the finish on a client-side `oauth_token` cookie. NOT verified: a real signed-in account, a http-only cookie, the page hook against the real site. The owner runs `NEMO_SIGNIN_DEBUG=1 ./nemo-app`, presses Sign in and sends back the app log.
  - Depends on: 1

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

- [x] 4. Source switch and SoundCloud library screen (PR 4)
  - _Requirements: 13, 14, 15_
  - Depends on: 2, 3

- [ ] 5. Playlist, user and track pages (PR 5)
  - _Requirements: 13, 14, 15_
  - Depends on: 4

- [x] 6. SoundCloud search results with filter chips (PR 5 of the 2026-11-19 stack; `All` and a track page are left)
  - _Requirements: 13, 14, 15_
  - Depends on: 4

- [ ] 7. Account writes (PR 7)
  - Crate and files: `nemo-soundcloud` (`write.rs`), `nemo-ui`.
  - _Requirements: 16, 17_
  - Depends on: 5, 6; the web app's request bodies read first.

## Definition of Done

Every task meets [section 4 of workflow.md](../../steering/workflow.md#4-definition-of-done).
