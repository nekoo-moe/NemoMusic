## Tasks

Tasks in other specs are cited by spec and the ADR 0004 Decision 4 row they own, until their task numbers are fixed. Tasks 1 to 11 are groundwork that other specs wait for. Tasks 1 to 3 move the credential store and keep sign-in behaviour unchanged: same service name `NemoMusic`, same `session-*` keys, same entry modifiers, same error texts.

- [x] 1. Move the `SecretStore` trait into `nemo-core`
  - Crate and files: `nemo-core` (new `src/secret_store.rs`, `src/lib.rs`, `Cargo.toml` feature `test-util`); `nemo-ytm` (`src/credentials.rs`, `Cargo.toml` dev-dependency on `nemo-core` with `test-util`). Touches nemo-core/src/secret_store.rs and nemo-core/src/lib.rs.
  - Does: Moves `SecretStore` and `StoreError` from `credentials.rs` to `nemo-core` and adds `MAX_SECRET_BYTES`, `is_valid_key` and `MemoryStore` (feature `test-util`), which replaces the test-only `fake` store. `Vault`, both platform stores, the key names and the part limits stay where and as they are.
  - _Requirements: none (groundwork)_
  - Depends on: none
  - Wave: 0
  - Checks: unit tests for `is_valid_key` (accepts `session-manifest`, `session-0`, `ytm/acct-1/part-0`; refuses empty, over 96 bytes, upper case, spaces, dots) and for `MemoryStore` (a value over `MAX_SECRET_BYTES` is refused). The existing `Vault` tests pass unchanged on `MemoryStore`.
  - Steering: `structure.md`: `nemo-core` gets feature `test-util`.

- [x] 2. Add the `nemo-secrets` crate with the platform stores
  - Crate and files: new `nemo-secrets` (`Cargo.toml`, `src/lib.rs`, `src/windows.rs`, `src/linux.rs`); `nemo-ytm` (`src/credentials.rs`, `Cargo.toml`); `nemo-app` (`Cargo.toml`, store setup in its wiring); workspace `Cargo.toml` members.
  - Does: Moves `WindowsStore`, `SecretServiceStore`, `SERVICE` and the fixed error texts from `credentials.rs` into `nemo-secrets`, adds `open_platform_store()`, and has `nemo-app` hand the `Arc<dyn SecretStore>` to `Vault`. `nemo-ytm` drops `keyring-core` and both store crates and keeps only `Vault`.
  - _Requirements: none (groundwork)_
  - Depends on: account-actions#1
  - Wave: 0
  - Checks: the ignored per-platform round-trip tests move with the stores and pass when run by hand on Windows and on Linux. `cargo tree -p nemo-ytm --edges normal` lists no keyring crate; `cargo tree -p nemo-secrets` lists only `nemo-core` among `nemo-*`. Manual, needs the owner or a throwaway account: a session saved before this change still loads after it on Windows and on Linux, and sign-out still removes it.
  - Steering: `tech.md`: replace rows "Credential store crates" and "Credential store" with `keyring-core` =1.0.1, `windows-native-keyring-store` =1.1.0 and `zbus-secret-service-keyring-store` =1.0.1, owned by `nemo-secrets`. `structure.md`: add `nemo-secrets` (depends on `nemo-core` only), `nemo-app` depends on it, `nemo-ytm` loses its keyring dependencies. ADR 0004: mark the `SecretStore` move done.

- [x] 3. Add `ScopedStore` to `nemo-secrets`
  - Crate and files: `nemo-secrets` (`src/scoped.rs`, `src/lib.rs`).
  - Does: Adds `ScopedStore`, which prefixes every key with `<prefix>/` and refuses a key that fails `is_valid_key` after prefixing, so one crate cannot reach another crate's entries. The existing `session-*` keys stay unscoped until account-actions#16 migrates them.
  - _Requirements: none (groundwork)_
  - Depends on: account-actions#2
  - Wave: 0
  - Checks: tests with `MemoryStore`: two scopes do not see each other's keys, `delete` reports whether the key existed, a key that is too long after prefixing is refused.
  - Steering: none.

- [x] 4. Add `SignOutWipe`, `WipeScope` and `WipeError`
  - Crate and files: `nemo-core` (`src/account.rs`). Touches nemo-core/src/account.rs.
  - Does: Adds the sign-out step trait with its scope and error types (ADR 0004 row 26), and a list of failed step names on `SignOutReport` so sign-out can report them. `AccountId` lands here too, because `WipeScope::Account` needs it; task 6 adds the rest of row 25.
  - _Requirements: none (groundwork)_
  - Depends on: none
  - Wave: 0
  - Checks: unit test that a fake step's name and count reach the report.
  - Steering: none.

- [x] 5. Run the registered wipe steps on sign-out
  - Crate and files: `nemo-ytm` (`src/account.rs`); `nemo-app` (wiring).
  - Does: `GoogleAccount` gets `register_wipe(Arc<dyn SignOutWipe>)`. `sign_out` runs every registered step with `WipeScope::All`, keeps going after a failure, names the failed steps in the report, and deletes the `Vault` entries last. `nemo-app` gets the one place where other specs register their steps.
  - _Requirements: 15_
  - Depends on: account-actions#4
  - Wave: 0
  - Checks: three fake steps with the middle one failing: all three run, the report names the failed one, the `Vault` is wiped after the steps. Sign-out with no steps behaves as today.
  - Steering: none.

- [x] 6. Add the account list types and `AccountSwitcher`
  - Crate and files: `nemo-core` (`src/account.rs`). Touches nemo-core/src/account.rs, after account-actions#4.
  - Does: Adds `MAX_ACCOUNTS`, `AccountId`, `PremiumStatus`, `AccountEntry`, `AccountList` with `generation`, the `AccountSwitcher` trait and `AccountError::{LimitReached, UnknownAccount}` (ADR 0004 row 25). `NoAccount` returns an empty list.
  - _Requirements: none (groundwork)_
  - Depends on: account-actions#4
  - Wave: 0
  - Checks: unit tests for the defaults and for error texts that hold no e-mail, name or id.
  - Steering: none.

- [x] 7. Add the rating and playlist-edit types to the catalog
  - Crate and files: `nemo-core` (`src/catalog.rs`); struct literals of `PlaylistDetail` in `nemo-ytm` and `nemo-app`. Touches nemo-core/src/catalog.rs.
  - Does: Adds `LikeStatus`, `Catalog::set_rating` with `rate_song` keeping its signature, `CatalogError::{Rejected, RateLimited}`, `SetVideoId`, `Privacy`, `PlaylistEditInfo`, `PlaylistDetail.edit` (always `None` for now) and the `PlaylistEditor` trait including `create` (ADR 0004 row 27).
  - _Requirements: none (groundwork)_
  - Depends on: none
  - Wave: 0
  - Checks: a `Catalog` that implements neither rating method returns an error instead of looping between the two defaults.
  - Steering: none.

- [x] 8. Add `StreamRequest`, `AudioTrackInfo` and `resolve_with`
  - Crate and files: `nemo-core` (`src/stream.rs`). Touches nemo-core/src/stream.rs.
  - Does: Adds `StreamRequest` (audio language, `prefer_premium`, `prefer_codec`), `AudioTrackInfo` and `StreamResolver::resolve_with`, which defaults to `resolve` (ADR 0004 row 13). The `ResolvedStream` fields `audio_language` and `audio_tracks` are not added here: ADR 0004 lands all four new `ResolvedStream` fields and `Track.source_url` in one task (soundcloud, rows 9 and 10), which depends on this one.
  - _Requirements: none (groundwork)_
  - Depends on: none
  - Wave: 0
  - Checks: the default `resolve_with` returns what `resolve` returns.
  - Steering: none.

- [x] 9. Resolve through a `StreamRequest` in `nemo-audio`
  - Crate and files: `nemo-audio` (`src/prepare.rs`, `src/player.rs`); `nemo-app` (passes the default request).
  - Does: `PrepareDeps` gains the current `StreamRequest` and `prepare` calls `resolve_with` instead of `resolve`. The default request (automatic language, no Premium, no codec hint) keeps every resolver as it is today. The request is read on the prepare thread, never in the audio callback.
  - _Requirements: none (groundwork)_
  - Depends on: account-actions#8
  - Wave: 0
  - Checks: `cargo test -p nemo-audio --test bit_exact`; the counting-allocator test `rt_no_alloc` still passes; a fake resolver records the request it was given.
  - Steering: none.

- [x] 10. Add `PlayerCommand::SwitchAudioTrack`
  - Crate and files: `nemo-core` (`src/player.rs`); `nemo-audio` (`src/control_cmd.rs`). Touches nemo-core/src/player.rs; only one task at a time edits it (ADR 0004).
  - Does: Adds `SwitchAudioTrack(Option<String>)` (ADR 0004 row 6). `control_cmd.rs` prepares the current track again with that language in its `StreamRequest` and reopens it at the current position. For a local track the command does nothing.
  - _Requirements: 27_
  - Depends on: account-actions#9
  - Wave: 0
  - Checks: `cargo test -p nemo-audio --test bit_exact`; `rt_no_alloc` still passes (the command runs on the control thread, never in the callback); a fake resolver shows the reopen position equals the position before the switch.
  - Steering: none.

- [x] 11. Add `ReleaseFeed` and `ReleaseNotice`
  - Crate and files: `nemo-core` (new `src/releases.rs`, `src/lib.rs`). Touches nemo-core/src/releases.rs and nemo-core/src/lib.rs.
  - Does: Adds `ReleaseNotice` and the `ReleaseFeed` trait with `list`, `mark_seen` and `take_unannounced` (ADR 0004 row 28).
  - _Requirements: none (groundwork)_
  - Depends on: none
  - Wave: 0
  - Checks: none beyond the usual.
  - Steering: none.

- [ ] 12. Spike: several accounts, one session each
  - Crate and files: throwaway branch `spike/nemo-ytm-multi-account`; only the result merges, into the Risks and Alternatives section of `.kiro/specs/account-actions/design.md`.
  - Does: Signs two throwaway Google accounts in through `nemo-signin` with fresh, empty webview folders and checks with `ytmapi-rs` 0.3.4 (one cookie each, no `X-Goog-AuthUser`) that each library differs and that both still work after 24 hours and after one signs out (Open Question 1). Human-paced requests only. Pass: criteria 11, 12 and 15 hold with one cookie per account, and tasks 16 and 21 to 23 go ahead as written. Fail: fallback 1 (one multi-login cookie, an `X-Goog-AuthUser` index per account, signed requests with `sha2`) rewrites tasks 16, 21 and 22; if that fails too, fallback 2 (one account at a time, switching signs out and opens the Google chooser) drops criterion 36 to one account and removes task 22.
  - _Requirements: 11, 12, 15, 36_
  - Depends on: none
  - Wave: 1
  - Checks: needs the owner: two throwaway Google accounts (ADR 0004, Actions for the owner). The result is written into the design's Risks section.
  - Steering: none.

- [ ] 13. Spike: the Premium session for `yt-dlp`
  - Crate and files: throwaway branch `spike/nemo-ytm-premium`; only the result merges, into the Risks and Alternatives section of `.kiro/specs/account-actions/design.md`.
  - Does: With one Premium account, runs `yt-dlp` with the session in a short-lived cookie file on Windows and through `--cookies /dev/stdin` on Linux. Records which field marks a Premium-only format (Open Question 3), the best AAC and Opus bitrates, whether the Windows file can be user-only and deleted on close while `yt-dlp` still reads it, and that no argument or environment variable carries the cookie. Pass: a marker exists and both hand-offs work, so tasks 24 and 25 go ahead as written. Fail on the marker: `PremiumStatus` is learned from the bitrate alone (task 25 changes). Fail on the Windows file: Premium stays Linux-only and the Windows switch is disabled with a reason (tasks 24, 25 and 35 change).
  - _Requirements: 18, 21, 22, 38_
  - Depends on: none
  - Wave: 1
  - Checks: needs the owner: a throwaway Premium account, or the owner's own with consent. The result is written into the design's Risks section.
  - Steering: none.

- [ ] 14. Spike: check the `ytmapi-rs` edit queries
  - Crate and files: throwaway branch `spike/nemo-ytm-edit-queries`; the result merges into the Risks and Alternatives section of `.kiro/specs/account-actions/design.md`, and the recorded bodies merge as fixtures under `crates/nemo-ytm/tests/fixtures/`.
  - Does: Checks `RateSongQuery`, `EditPlaylistQuery`, `AddPlaylistItemsQuery`, `RemovePlaylistItemsQuery`, `DeletePlaylistQuery`, and `CreatePlaylistQuery` if the crate has it, field by field against a throwaway account. Records which actions need a raw innertube call; `move_track` is raw in any case. Pass or fail does not drop a task: a query that works is used typed in tasks 18 to 20, one that fails becomes a raw call in `raw.rs` style.
  - _Requirements: 1, 5, 7_
  - Depends on: none
  - Wave: 1
  - Checks: needs the owner's throwaway account. Fixtures hold no cookie, token or account id.
  - Steering: none.

- [ ] 15. Spike: the cheapest release check
  - Crate and files: throwaway branch `spike/nemo-ytm-release-check`; only the result merges, into the Risks and Alternatives section of `.kiro/specs/account-actions/design.md`.
  - Does: With a throwaway account that follows a known set of artists, compares the new-releases page matched against the subscription list (two calls) with one browse per followed artist, and records whether the page is personal enough and the calls per check (Open Question 2). Pass: the two-call plan finds the followed artists' releases and task 28 uses it. Fail: task 28 uses one browse per artist, one every 90 seconds, oldest first, at most 40 per check.
  - _Requirements: 30_
  - Depends on: none
  - Wave: 1
  - Checks: needs the owner's throwaway account. The result is written into the design's Risks section.
  - Steering: none.

- [ ] 16. Store one session per account and migrate the old keys
  - Crate and files: `nemo-ytm` (`src/credentials.rs`, new `src/account_index.rs`); `nemo-app` (passes a `ScopedStore` with prefix `ytm`).
  - Does: `Vault` keys entries per account (`ytm/index`, `ytm/acct-<n>/manifest`, `ytm/acct-<n>/part-<i>`). At start-up, when `ytm/index` is missing and `session-manifest` exists, it copies the parts to account 1, writes the index last, then deletes the old keys; a crash before the index write runs the copy again. A user signed in before the upgrade stays signed in.
  - _Requirements: 11, 15_
  - Depends on: account-actions#3, account-actions#6, account-actions#12
  - Wave: 1
  - Checks: migration tests with a `MemoryStore` that fails at each step; the index never lists a missing account. Manual upgrade with an existing sign-in on Windows and on Linux.
  - Steering: ADR 0001: record the multi-account result of task 12 and the keyring layout.

- [ ] 17. Add the release notice store
  - Crate and files: `nemo-ytm` (new `src/releases.rs` with its migrations, `Cargo.toml`: `rusqlite` =0.41.1 only if offline-cache has not added it).
  - Does: A SQLite file in the app data folder holds `ReleaseNotice` rows and `last_check` per `AccountId`, with migrations, and implements `ReleaseFeed` over them. It deletes rows for a given `WipeScope`.
  - _Requirements: 31, 33_
  - Depends on: account-actions#6, account-actions#11, account-actions#15
  - Wave: 1
  - Checks: round trip; `mark_seen`; `take_unannounced` returns each notice once; wiping one account keeps the other's rows; migration from an empty file.
  - Steering: none.

- [ ] 18. Like and dislike through `set_rating`
  - Crate and files: `nemo-ytm` (new `src/ratings.rs`, `src/session.rs`); `nemo-app` (`src/merged.rs`).
  - Does: `SessionCatalog` implements `set_rating` with the call task 14 chose, and `rate_song` goes through it. Liking a disliked song sends one `Liked` call that replaces the dislike. Offline returns `Network` before sending, a refusal is `Rejected` with a fixed text and no response body, HTTP 429 is `RateLimited`; there is no retry loop and no queue, and nothing skips the playing track.
  - _Requirements: 1, 2, 3, 4, 9, 10_
  - Depends on: account-actions#7, account-actions#14
  - Wave: 2
  - Checks: requests against `test_server.rs` with the recorded bodies; no request while offline; a refusal leaves the previous state.
  - Steering: none.

- [ ] 19. Edit playlist details on YouTube Music
  - Crate and files: `nemo-ytm` (new `src/playlist_edit.rs`, playlist page parsing in `src/pages.rs`).
  - Does: Implements `PlaylistEditor::{rename, set_description, set_privacy, delete}` and fills `PlaylistDetail.edit` only for playlists the signed-in user owns. Errors follow task 18.
  - _Requirements: 5, 8, 9, 10_
  - Depends on: account-actions#7, account-actions#14
  - Wave: 2
  - Checks: recorded fixtures; a playlist owned by someone else has `edit: None`.
  - Steering: none.

- [ ] 20. Create playlists and add, remove and move songs
  - Crate and files: `nemo-ytm` (`src/playlist_edit.rs`, `src/raw.rs`).
  - Does: Implements `create` (the first videos in one call, used by playlist-import), `add_tracks`, `remove_track` and `move_track` (raw call). The local order changes only after YouTube Music accepts.
  - _Requirements: 7, 9, 10_
  - Depends on: account-actions#19
  - Wave: 2
  - Checks: recorded fixtures; move to the top, to the end and with `before: None`.
  - Steering: none.

- [ ] 21. Switch between signed-in accounts
  - Crate and files: `nemo-ytm` (`src/account.rs`, `src/session.rs`).
  - Does: `GoogleAccount` keeps one `SessionCatalog` per account and implements `AccountSwitcher::{list, switch_to}`. A switch loads that account's cookie, swaps the `SessionSlot`, bumps `generation` and tells the player nothing; answers started under an older generation are dropped. An account whose session no longer works stays listed as "sign in again" and the active account does not change. Logs hold no e-mail, name or id (`Secret<T>`).
  - _Requirements: 12, 13, 14_
  - Depends on: account-actions#6, account-actions#16
  - Wave: 2
  - Checks: generation test (an old answer is dropped); a fake player receives no command on a switch; captured logs hold no account data.
  - Steering: none.

- [ ] 22. Add a second account
  - Crate and files: `nemo-ytm` (`src/account.rs`); `nemo-signin` (an argument for the webview data folder).
  - Does: `add_account` runs `nemo-signin` with a fresh, empty webview folder (`accounts/pending/`), checks the cookie, saves it under the next free `AccountId` and deletes the folder. Other accounts stay signed in. A sixth account returns `LimitReached`.
  - _Requirements: 11, 36_
  - Depends on: account-actions#21
  - Wave: 2
  - Checks: fake helper tests: the folder is gone after success, cancel and failure; the limit holds at `MAX_ACCOUNTS`. Manual with the owner's throwaway accounts.
  - Steering: none.

- [ ] 23. Sign out of one account or all
  - Crate and files: `nemo-ytm` (`src/account.rs`).
  - Does: `sign_out_account(id)` runs every registered `SignOutWipe` with `WipeScope::Account(id)`, then removes that account's release rows and, last, its `Vault` entries. `AccountService::sign_out` does the same for all accounts with `WipeScope::All`. When the active account leaves, the next one becomes active, or none.
  - _Requirements: 15, 33_
  - Depends on: account-actions#5, account-actions#17, account-actions#22
  - Wave: 2
  - Checks: with two accounts, signing out one keeps the other's keys, rows and session; signing out all leaves no `ytm/` key.
  - Steering: none.

- [ ] 24. Hand the session to `yt-dlp` without exposing it
  - Crate and files: `nemo-ytm` (new `src/premium.rs`, `src/ytdlp.rs`).
  - Does: Adds `SessionSource` and the hand-off task 13 proved: on Windows a `run/<random>.txt` file, created new with user-only access, passed as `--cookies <file>` and removed by a drop guard and a start-up sweep; on Linux `--cookies /dev/stdin`. `--ignore-config` stays. If the file cannot be created, resolution falls back to anonymous.
  - _Requirements: 21_
  - Depends on: account-actions#13, account-actions#21
  - Wave: 2
  - Checks: a fake `yt-dlp` script (extending the tests near `ytdlp.rs:598`) proves no argument or environment variable holds the cookie and the file is gone after success, failure and timeout; sweep test; no path or cookie in logs.
  - Steering: none.

- [ ] 25. Resolve Premium audio with the active session
  - Crate and files: `nemo-ytm` (`src/ytdlp.rs`, `src/premium.rs`, `src/account.rs`).
  - Does: `YtDlpResolver::resolve_with` passes the active session only when `prefer_premium` is true, and resolves as today otherwise. The first stream resolved with the session sets the account's `PremiumStatus` from the marker task 13 found, with no extra request. Without Premium it plays as before and claims nothing; a refusal is `StreamError::NotAvailable` and the setting stays on.
  - _Requirements: 18, 19, 20, 38_
  - Depends on: account-actions#8, account-actions#9, account-actions#24
  - Wave: 2
  - Checks: recorded `yt-dlp` answers with and without the marker; a counting fake shows no extra request; with `prefer_premium` false the fake `yt-dlp` gets no `--cookies`.
  - Steering: none.

- [ ] 26. Rank YouTube audio formats by what can play
  - Crate and files: `nemo-ytm` (`src/premium.rs`, `src/ytdlp.rs`).
  - Does: Adds `rank_audio`: the higher bitrate wins, Opus wins a tie, and only codecs in `supported_stream_codecs()` take part, so Premium plays AAC until the Opus decoder is in that list. The resolved stream carries the codec and bitrate actually chosen, or none so the badge says "Unknown". No YouTube stream is labelled lossless, "HD" or "Hi-Res".
  - _Requirements: 22, 23_
  - Depends on: account-actions#25, local-library (ADR 0004 row 14: Opus decoder and `supported_stream_codecs()`)
  - Wave: 2
  - Checks: ranking tests with and without `Codec::Opus` in the supported list; no label of a YouTube stream contains "lossless", "HD" or "Hi-Res".
  - Steering: `tech.md` Opus row: Premium plays AAC until the decoder lands.

- [x] 27. Choose the audio track by preferred language
  - Crate and files: `nemo-ytm` (`src/ytdlp.rs`).
  - Does: `resolve_with` keeps the formats whose `language` matches `StreamRequest::audio_language`; with no match, or "Automatic", it plays the default (original) track. It fills `ResolvedStream.audio_language` and `audio_tracks`, so the UI can say which language plays and offer a switch.
  - _Requirements: 24, 25, 26, 27_
  - Depends on: account-actions#8, soundcloud (ADR 0004 rows 9 and 10: the one task that adds the new `ResolvedStream` fields)
  - Wave: 2
  - Checks: a recorded answer of a video with dubbed tracks; tests for a match, no match and "Automatic".
  - Steering: none.

- [ ] 28. Check for new releases at a calm pace
  - Crate and files: `nemo-ytm` (`src/releases.rs`, `src/account.rs`).
  - Does: `ReleaseWatcher`, a tokio task owned by `GoogleAccount`, checks only the active account with the plan task 15 chose, at most once per `MIN_CHECK_INTERVAL` (6 hours) and only while the app runs, and writes new notices to the store. It does not start while the setting is off or unconfirmed, or when nobody is signed in.
  - _Requirements: 30, 31, 35_
  - Depends on: account-actions#15, account-actions#17, account-actions#21
  - Wave: 2
  - Checks: `tokio::time::pause` tests: no second check within 6 hours, no burst of one call per artist. A counting fake host shows zero requests while the switch is off (opt-in rule 4). `music.youtube.com` is already on the README egress list.
  - Steering: none.

- [ ] 29. Wire the account settings and services in `nemo-app`
  - Crate and files: `nemo-core` (`src/services.rs`: `UiSettings`, `SettingChange`); `nemo-app` (config, wiring). Touches nemo-core/src/services.rs.
  - Does: Adds `premium_audio`, `audio_language`, `release_notices` and `os_release_notices` (off or "Automatic" by default), builds the `StreamRequest` from them, and injects `AccountSwitcher`, `PlaylistEditor` and `ReleaseFeed`. The Premium hand-off and the release watcher are built only when their switch is on and the current `OptInInfo` version was confirmed.
  - _Requirements: 17, 20, 24, 28_
  - Depends on: account-actions#23, account-actions#25, account-actions#27, account-actions#28, integrations (ADR 0004 row 43: `OptInInfo`)
  - Wave: 2
  - Checks: a counting fake host shows zero session hand-offs and zero release checks while a switch is off or unconfirmed; settings round trip; `cargo test -p nemo-audio --test bit_exact` (the request reaches `nemo-audio`).
  - Steering: none.

- [ ] 30. Send one grouped OS notification per release check
  - Crate and files: `nemo-app` (wiring).
  - Does: After each check `nemo-app` calls `ReleaseFeed::take_unannounced` and, when `os_release_notices` is on, sends one grouped `OsNotifier::notify`. It sends none while the window is focused (ADR 0004, third round); the in-app list still updates.
  - _Requirements: 32_
  - Depends on: account-actions#28, account-actions#29, integrations (ADR 0004 row 29: `OsNotifier`)
  - Wave: 2
  - Checks: fake notifier: one call for three notices, none for zero, none while focused.
  - Steering: none.

- [ ] 31. Add the dislike button
  - Crate and files: `nemo-ui` (Now Playing and track row views under `src/gui/views/`).
  - Does: A dislike control beside like; like and dislike are never both shown, and a like replaces a dislike. The state changes only after YouTube Music accepts; a failure keeps the old state and shows one plain line with a retry, and offline disables the control. A dislike never skips the playing track.
  - _Requirements: 1, 2, 3, 4, 9, 10_
  - Depends on: account-actions#18, account-actions#29
  - Wave: 3
  - Checks: headless UI tests; `design.md` checklist with a screenshot.
  - Steering: none.

- [ ] 32. Add the playlist edit menu
  - Crate and files: `nemo-ui` (playlist view under `src/gui/views/`).
  - Does: Rename, description, privacy (private, unlisted, public) and delete, shown only for owned playlists. Delete asks first and names the playlist. Failures follow task 31.
  - _Requirements: 5, 6, 8, 9, 10_
  - Depends on: account-actions#19, account-actions#29
  - Wave: 3
  - Checks: headless tests, including the confirmation that names the playlist and hidden actions on someone else's playlist; `design.md` checklist with a screenshot.
  - Steering: none.

- [ ] 33. Add, remove and move songs from the playlist view
  - Crate and files: `nemo-ui` (playlist view, track context menu).
  - Does: Add to playlist, remove and move, shown only for owned playlists; the new order shows after YouTube Music accepts.
  - _Requirements: 7, 8, 9, 10_
  - Depends on: account-actions#20, account-actions#32
  - Wave: 3
  - Checks: headless tests; keyboard reorder; `design.md` checklist with a screenshot.
  - Steering: none.

- [ ] 34. Add the account switcher
  - Crate and files: `nemo-ui` (account switcher view, sidebar entry).
  - Does: Lists accounts by display name with the active one marked, adds an account, switches, signs out of one or all, says so when 5 accounts are signed in, and shows "sign in again" for an expired session. Library, likes, history, home feed and playlists clear when `generation` changes. Works by keyboard alone; reduced motion removes the switch and list animations.
  - _Requirements: 11, 12, 13, 14, 15, 16, 34, 36_
  - Depends on: account-actions#22, account-actions#23, account-actions#29
  - Wave: 3
  - Checks: headless keyboard-only and reduced-motion tests; `design.md` checklist with a screenshot.
  - Steering: none.

- [ ] 35. Add the Premium audio setting
  - Crate and files: `nemo-ui` (settings view); `docs/adr/0001-youtube-music-auth.md`; `README.md`.
  - Does: One `OptInRow` "Use Premium audio quality", off by default, saying that the session is passed to `yt-dlp`, that this may restrict the account, and that it works only with Premium. Until Premium is known it says "active when your plan has it". A refusal shows a banner suggesting turning it off; a cookie-file failure shows "Premium audio could not start".
  - _Requirements: 17, 19, 37_
  - Depends on: account-actions#25, account-actions#26, account-actions#29, integrations (ADR 0004 row 43: `OptInRow`)
  - Wave: 3
  - Checks: headless tests; the `nemo-app` test that compares `OptInInfo.hosts` with the README egress list; `design.md` checklist with a screenshot.
  - Steering: ADR 0001, in this same change: the default stays anonymous, this setting is the one switch, and the session goes through a temporary cookie file (Linux: a pipe). README egress: `www.youtube.com` and `*.googlevideo.com` also say "only when Premium audio is on" for the session.

- [ ] 36. Add the audio language setting and the track switch
  - Crate and files: `nemo-ui` (settings view, Now Playing).
  - Does: "Preferred audio language" in settings, default "Automatic". When a video has several audio tracks, Now Playing shows which language plays and lets the user switch during playback through `SwitchAudioTrack`.
  - _Requirements: 24, 26, 27_
  - Depends on: account-actions#10, account-actions#27, account-actions#29
  - Wave: 3
  - Checks: headless tests, keyboard only; `design.md` checklist with a screenshot.
  - Steering: none.

- [ ] 37. Add the release notice settings and list
  - Crate and files: `nemo-ui` (settings view, new release list view); `README.md`.
  - Does: Two `OptInRow`s: "Notify me about new releases from followed artists" (sends the followed artists' channel IDs to `music.youtube.com`, checks at most every 6 hours) and "OS notifications for new releases" (nothing leaves the computer; says who can read the text). An in-app list of new releases with no badge count, popup or sound, and no animation under reduced motion.
  - _Requirements: 28, 29, 31, 32, 34_
  - Depends on: account-actions#29, account-actions#30, integrations (ADR 0004 row 43: `OptInRow`)
  - Wave: 3
  - Checks: headless tests, including reduced motion; the `nemo-app` egress comparison test; `design.md` checklist with a screenshot.
  - Steering: README egress: `music.youtube.com` also says "only when release notices are on" for the check.

- [ ] 38. Run the account manual checks
  - Crate and files: none (results in the PR description).
  - Does: With two throwaway accounts and one Premium account, on Windows and on Linux: add, switch while a song plays, sign out of one and of all, dislike, edit a playlist, Premium on and off, a dubbed video, one release check. Confirms the README egress list holds `music.youtube.com`, `www.youtube.com` and `accounts.google.com` and no new host.
  - _Requirements: 11, 12, 13, 15, 18, 22, 35_
  - Depends on: account-actions#31, account-actions#32, account-actions#33, account-actions#34, account-actions#35, account-actions#36, account-actions#37
  - Wave: 3
  - Checks: needs the owner (throwaway and Premium accounts, a Linux machine); `cargo test -p nemo-audio --test bit_exact`.
  - Steering: none.

## Definition of Done

Every task meets [section 4 of workflow.md](../../steering/workflow.md#4-definition-of-done). For this spec also:

- [ ] `cargo test -p nemo-audio --test bit_exact` and `rt_no_alloc` pass in every task that touches `nemo-audio` (tasks 9, 10, 29) and in task 38.
- [ ] No cookie, token, e-mail, display name or account id in logs, errors, fixtures or command lines (`Secret<T>`, criterion 14 and 21).
- [ ] Each opt-in switch (Premium, release notices) has a counting fake-host test proving zero requests while it is off or unconfirmed.
- [ ] Each spike wrote its pass or fail result into the Risks and Alternatives section of `design.md`, and the tasks it changes were updated before they start.
- [ ] Sign-in on an existing install still works after tasks 1, 2 and 16 (manual, Windows and Linux).
