## Tasks

Cross-spec ids written `<spec>#?` name the owner's task by its ADR 0004 Decision 4 row; replace `?` with the task number once that spec's `tasks.md` has landed. Task 1 is groundwork other specs depend on (row 17: cast, account-actions), and task 2 is the contract nemo-lyrics and lyrics-plus use (row 18).

- [ ] 1. Add the downloads types to `nemo-core`
  - Crate and files: `nemo-core`: `src/downloads.rs` (new), `src/lib.rs`. Touches nemo-core/src/downloads.rs.
  - Does: Adds `SaveReason`, `CollectionRef`, `FailReason`, `SaveState`, `SaveRequest`, `StorageReport`, `DownloadEvent`, `RemovedReason`, `PauseReason`, `DownloadError` (`thiserror`), `DownloadReceiver` and the traits `DownloadService`, `OfflineStore` and `NetworkMode`, signatures as in the design. `CachedAudio.path` is a `Secret<PathBuf>`. No I/O.
  - _Requirements: none (groundwork)_
  - Depends on: none
  - Wave: 0
  - Checks: unit test that `CachedAudio`'s `Debug` prints no path; `DownloadError` texts hold no URL, file name or video id.
  - Steering: ADR 0004: add `downloads.rs` and `LyricsService::keep_offline` to the expected `nemo-core` additions.

- [ ] 2. Add `LyricsService::keep_offline` with a default
  - Crate and files: `nemo-core`: `src/services.rs`. Touches nemo-core/src/services.rs; order with the lyrics-plus task that adds `line_meta` to the same file.
  - Does: Adds `keep_offline(&self, track_key: &str, keep: bool)` with a no-op default, so existing implementations still build.
  - _Requirements: none (groundwork)_
  - Depends on: none
  - Wave: 0
  - Checks: the existing lyrics tests pass unchanged.

- [ ] 3. Add `AppPaths.offline`
  - Crate and files: `nemo-app`: `src/paths.rs`.
  - Does: Adds `offline` (`<root>/offline`) to `AppPaths`, inside the app's private data folder.
  - _Requirements: none (groundwork)_
  - Depends on: none
  - Wave: 0
  - Checks: the `paths.rs` tests cover the new folder.

- [ ] 4. Create the offline index and file layout
  - Crate and files: `nemo-ytm`: `Cargo.toml` (`rusqlite` =0.41.1 bundled, `getrandom` =0.3.4), `src/offline.rs` (new), `src/offline/store.rs` (new).
  - Does: Opens `offline.db` with migration 1: `song`, `segment`, `queue`, `play_count`, `collection`, `collection_song` and art rows. Files go to `blobs/`, `tmp/` and `art/` with random opaque names, plain files with no encryption. Adds the small `Clock` trait of the module.
  - _Requirements: 21, 22_
  - Depends on: offline-cache#1
  - Wave: 1
  - Checks: reopen and migration tests; no file name contains a video id or a title.
  - Steering: `tech.md`: replace "build no audio cache" with the decided rule (cache in `nemo-ytm`, plain files with opaque names, caps); add rows `getrandom` 0.3.4 (MIT OR Apache-2.0, already locked) and `rusqlite` 0.41.1 also in `nemo-ytm` (account-actions then skips its own `rusqlite` line). `structure.md`: `nemo-ytm` owns `offline/` and `offline.db`.

- [ ] 5. Clean up the cache at start
  - Crate and files: `nemo-ytm`: `src/offline/store.rs`.
  - Does: Deletes `.part` files with no queue row and marks `song` rows whose file is missing as unsaved. A damaged index is deleted with its files and the user is told once.
  - _Requirements: 24_
  - Depends on: offline-cache#4
  - Wave: 1
  - Checks: tests with a stray `.part`, a missing blob and a truncated index.

- [ ] 6. Remove songs and wipe the cache
  - Crate and files: `nemo-ytm`: `src/offline/store.rs`.
  - Does: Removes one song, a collection (keeping songs another collection or a user save still holds) or the whole cache, deleting the files and returning freed bytes. `wipe` closes the connection, removes `<root>/offline` completely (songs, art, index, play counts, queue) and returns the file count.
  - _Requirements: 9, 23_
  - Depends on: offline-cache#4
  - Wave: 1
  - Checks: after `wipe` no file is left under `offline/`; play counts and queue are gone; removal keeps no copy.

- [ ] 7. Download one song with ranged reads
  - Crate and files: `nemo-ytm`: `src/offline/fetch.rs` (new).
  - Does: Blocking `reqwest` `GET` with one `Range: bytes=<part length>-` stream into `tmp/<opaque>.part` in 1 MiB reads, checking a pause flag between reads. Compares the length to `Content-Length`, syncs and renames to `blobs/`; a 200 answer to a range restarts from zero. The bytes are kept as served, with no re-encoding; URLs, file names and video ids are never logged.
  - _Requirements: 11, 13_
  - Depends on: offline-cache#4
  - Wave: 1
  - Checks: loopback `test_server.rs` serving ranges: resume continues from the `.part` length, pause stops within one chunk, a short body fails, saved bytes equal the served bytes.

- [ ] 8. Pin saved lyrics in the lyrics cache
  - Crate and files: `nemo-lyrics`: `src/cache.rs` (migration adds a `pinned` flag), the `LyricsService` implementation. Touches nemo-lyrics/src/cache.rs; order with lyrics-plus tasks on the same file.
  - Does: Implements `keep_offline`: a pinned entry skips expiry until unpinned.
  - _Requirements: 17_
  - Depends on: offline-cache#2
  - Wave: 1
  - Checks: an expired pinned entry is still served; unpinning lets it expire; migration on an existing cache file.

- [ ] 9. Queue and pace downloads on one worker
  - Crate and files: `nemo-ytm`: `src/offline/worker.rs` (new).
  - Does: One `std` thread, `nemo-downloads`, takes one queued song at a time: resolve with `StreamResolver::resolve_with` and the `StreamRequest` it is given (`prefer_premium` set by `nemo-app`), fetch, then sleep a `Jitter` of 10 to 30 s. Failures back off 60 s doubling to 30 min, honour `Retry-After` on 429, and pause the queue after five in a row; a failed song gets a plain reason and the worker moves on. No parallel connections and no retry without the wait; the user retries one song by hand.
  - _Requirements: 10, 11, 12, 14_
  - Depends on: offline-cache#7, account-actions#? (row 13, `StreamRequest` and `resolve_with`)
  - Wave: 2
  - Checks: fake clock and fake `Jitter`: never two fetches at once, every gap 10 to 30 s, 429 with `Retry-After` waits that long, five failures pause, manual retry runs once.

- [ ] 10. Enforce the size limit and free space
  - Crate and files: `nemo-ytm`: `Cargo.toml` (`fs4` =1.1.0), `src/offline/worker.rs`.
  - Does: Before each download adds the expected size from the first response to `used_bytes`; over the limit (default 2 GB) the song fails `LimitReached`, and free disk under size plus 256 MiB fails `DiskFull`. User saves are never deleted to make room.
  - _Requirements: 8_
  - Depends on: offline-cache#9
  - Wave: 2
  - Checks: tests at, under and over the limit with a fake free-space source. If `fs4` turns out to need a C toolchain, stop and ask the owner.
  - Steering: `tech.md` row `fs4` 1.1.0 (MIT OR Apache-2.0, MSRV 1.75).

- [ ] 11. Save cover, metadata, lyrics and segments with each song
  - Crate and files: `nemo-ytm`: `src/offline/worker.rs`, `src/offline/store.rs`.
  - Does: After the audio, the worker stores the cover through `ImageSource` in `art/`, fetches lyrics with `LyricsService::lookup` and pins them with `keep_offline`, and stores SponsorBlock segments from `SegmentSource`, all in one transaction with the `song` row.
  - _Requirements: 17_
  - Depends on: offline-cache#9, offline-cache#2
  - Wave: 2
  - Checks: with fakes for the three services: a saved song has its cover, segments and a pinned lyrics call; a failed extra does not fail the song.

- [ ] 12. Implement `DownloadService`
  - Crate and files: `nemo-ytm`: `src/offline/service.rs` (new), `src/offline.rs` (`OfflineCache`).
  - Does: `OfflineCache` implements `DownloadService` and `OfflineStore` over the store and worker: save (songs of an album or playlist become queue rows), a cheap per-frame `state`, pause, resume, retry, removal, `clear_all` with freed bytes, `storage` (used, free, limit, per collection, smart, art), `set_limit`, `set_smart` and `subscribe`.
  - _Requirements: 1, 7, 9, 11_
  - Depends on: offline-cache#5, offline-cache#6, offline-cache#10, offline-cache#11
  - Wave: 2
  - Checks: service tests over a temp folder; `state` answers without SQL on the hot path; events arrive in order.

- [ ] 13. Play saved songs from the cache
  - Crate and files: `nemo-audio`: `src/prepare.rs` (`PrepareDeps.offline`), `tests/bit_exact.rs`, `tests/rt_no_alloc.rs`. Touches nemo-audio/src/prepare.rs; lands after local-library#19 and local-library#21, one at a time.
  - Does: For a YouTube track, `prepare()` asks `OfflineStore::open` before the resolver; a hit opens `FileSource` and `StreamDecoder::open(source, None)` with no lookup and no audio request. The badge uses the probed codec and `bytes * 8 / duration`, "Unknown" when either is missing, and `quality_class` stays Lossy (never Lossless, HD or Hi-Res). A failed open or probe calls `mark_unsaved`; online it falls back to the stream, offline it shows an error. Only this code calls `Secret::expose` on the path.
  - _Requirements: 3, 13, 15, 16, 24_
  - Depends on: offline-cache#1, local-library#3, local-library#6
  - Wave: 2
  - Checks: `cargo test -p nemo-audio --test bit_exact`: a cached copy of the fixture decodes to the same samples as the streamed copy, and the default result is unchanged with the cache present but unused; `rt_no_alloc` (counting allocator) run playing from a cached file; badge tests; a recording fake resolver sees zero calls for a saved song. No new work in the audio callback.

- [ ] 14. Answer segments and covers from the cache
  - Crate and files: `nemo-ytm`: `src/offline/sources.rs` (new: `OfflineSegmentSource`, `OfflineImages`).
  - Does: Wrappers answer from the index for saved songs and from the online source otherwise, so SponsorBlock skips and covers work offline.
  - _Requirements: 17, 18_
  - Depends on: offline-cache#11
  - Wave: 2
  - Checks: with the online source failing, a saved song still returns its segments and cover, and the player skips a stored segment.

- [ ] 15. Add offline mode with the offline catalog and request gates
  - Crate and files: `nemo-ytm`: `src/offline/catalog.rs` (new), `src/offline/gate.rs` (new).
  - Does: The offline `Catalog` lists only saved songs, albums and playlists. Gates around the resolver, stream opener, segment source and image source refuse at once while `NetworkMode::is_offline`. Mode switches on by itself after two network failures within 30 s on user actions, with one notice, and only the user switches it off; no connectivity probe is sent.
  - _Requirements: 4, 5_
  - Depends on: offline-cache#4, offline-cache#1
  - Wave: 2
  - Checks: a recording fake host sees zero requests while saved songs browse and play in offline mode; two failures switch once; a third does not repeat the notice.

- [ ] 16. Gate the other services that call YouTube hosts
  - Crate and files: `nemo-app`: `src/backend.rs` (gate wrappers around `CommentSource`, `VideoRatings`, `SubtitleSource`, the `UrlResolver`, the `Catalog` extensions and the release check).
  - Does: Wraps every other service that reaches a YouTube host in the offline gate. Services for other hosts are not gated.
  - _Requirements: 4_
  - Depends on: offline-cache#15, youtube-extras#? (rows 20, 21, 22), soundcloud#? (row 11, `UrlResolver`), account-actions#? (row 28, release check)
  - Wave: 2
  - Checks: the fake-host test of task 15 extended to these services: zero requests in offline mode.

- [ ] 17. Count plays and keep the most-played songs
  - Crate and files: `nemo-ytm`: `src/offline/smart.rs` (new), `src/offline.rs` (`PlayObserver` impl).
  - Does: `OfflineCache` as a `PlayObserver` hands each counted `TrackEnded` to the store writer (`on_event` must not block) and `record_play` stores it in `play_count` on this computer only. After a play and at start, songs with at least 3 plays that are not saved are queued as `Smart`; before a smart save, the least played smart songs are evicted until at most 50 songs and 25% of the limit hold. `User` songs are never evicted. Nothing runs while the smart switch is off (opt-in rule 1).
  - _Requirements: 20, 21_
  - Depends on: offline-cache#12, integrations#? (row 30, `PlayObserver` and `PlayRule`)
  - Wave: 2
  - Checks: `proptest` with random counts and sizes: smart songs never pass 50 or 25% of the limit and no `User` song is evicted; zero fetches with the switch off.

- [ ] 18. Check saved songs for takedowns
  - Crate and files: `nemo-ytm`: `src/offline/verify.rs` (new).
  - Does: Re-resolves one saved song at a time, at most one every 10 to 30 s, each song about every 30 days, only online and idle. `NotAvailable` twice in a row deletes the file and index row and sends `Removed { reason: RemovedByYoutube }` naming the song.
  - _Requirements: 25_
  - Depends on: offline-cache#12
  - Wave: 2
  - Checks: two `NotAvailable` answers delete the song, one does not; pacing with a fake clock. Needs an owner answer: the opt-in row lists takedown checks under smart downloads, so say in the PR whether checks for user saves run with smart downloads off; until answered they run only while that opt-in is confirmed.

- [ ] 19. Wipe the cache on sign-out
  - Crate and files: `nemo-ytm`: `src/offline.rs` (`SignOutWipe` impl); `nemo-app`: register the step with `GoogleAccount`.
  - Does: `OfflineCache` implements `SignOutWipe` named "offline" (the `WipeScope` is ignored) by calling `OfflineStore::wipe` and returns the file count for `SignOutReport.files_removed`, so the sign-out screen shows it. Signing out of any account wipes the whole cache.
  - _Requirements: 23_
  - Depends on: offline-cache#6, account-actions#? (row 26, `SignOutWipe`)
  - Wave: 2
  - Checks: after sign-out no file is left under `offline/` and the report counts the files.

- [ ] 20. Wire the offline cache into the app
  - Crate and files: `nemo-app`: `src/backend.rs`, `src/main.rs`.
  - Does: Builds `OfflineCache` in `AppPaths.offline`, the worker, the gates and the offline `Catalog`; sets `PrepareDeps.offline`; sets `prefer_premium` from the account-actions Premium switch; registers the `PlayObserver`. Smart downloads start only when the switch is on and `confirmed_version == version` of its `OptInInfo`.
  - _Requirements: 3, 14, 19_
  - Depends on: offline-cache#12, offline-cache#13, offline-cache#14, offline-cache#15, offline-cache#17, offline-cache#18, offline-cache#19, offline-cache#3, account-actions#? (Premium switch, row 13), integrations#? (row 43, `OptInInfo`)
  - Wave: 2
  - Checks: a counting fake host sees zero smart fetches while the switch is off or unconfirmed; the README-versus-`OptInInfo` hosts test in `nemo-app` passes.
  - Steering: `structure.md`: `nemo-app` wires `OfflineStore`, `DownloadService` and `NetworkMode`. README egress: the "When" of `www.youtube.com` and `*.googlevideo.com` adds "when songs are saved offline or checked (smart downloads: only when smart downloads are on)"; no new host.

- [ ] 21. Add "Save offline" and the saved mark
  - Crate and files: `nemo-ui`: song rows and the album, playlist, queue and search views (`src/gui/views/detail.rs`, `queue.rs`, `search.rs`, `library.rs`).
  - Does: "Save offline" on a song, album or playlist; per-song state (queued, saving, saved, failed) and a saved mark on every row and page where the song appears. No export, copy, "show in folder", drag-out or path is shown.
  - _Requirements: 1, 2, 22, 26_
  - Depends on: offline-cache#20
  - Wave: 3
  - Checks: `design.md` checklist with a screenshot; keyboard reach and visible focus; a UI test that no saved-song row offers drag-out.

- [ ] 22. Add the storage view with the limit and removal
  - Crate and files: `nemo-ui`: `src/gui/views/offline_storage.rs` (new), `settings.rs`.
  - Does: Shows used size, free disk, and size per album, playlist, smart downloads and artwork; sets the limit; removes a song, album, playlist or everything and shows the freed space.
  - _Requirements: 7, 8, 9, 26_
  - Depends on: offline-cache#20
  - Wave: 3
  - Checks: `design.md` checklist with a screenshot; keyboard and focus.

- [ ] 23. Add the download queue controls
  - Crate and files: `nemo-ui`: `src/gui/views/offline_storage.rs`.
  - Does: Pause and resume the queue, retry a failed song, and show plain failure reasons and the paused-after-failures notice. Progress animation follows reduced motion.
  - _Requirements: 11, 12, 26_
  - Depends on: offline-cache#22
  - Wave: 3
  - Checks: `design.md` checklist with a screenshot; reduced-motion snapshot.

- [ ] 24. Add the offline switch and shortcut
  - Crate and files: `nemo-ui`: `src/gui/views/settings.rs`, shell shortcut handling; `nemo-app`: save the setting.
  - Does: Offline mode is off by default and switched from Settings and a shortcut. The one notice shows when the app switched by itself; browse then shows only saved songs.
  - _Requirements: 4, 5, 6, 26_
  - Depends on: offline-cache#20
  - Wave: 3
  - Checks: `design.md` checklist with a screenshot; a test that a new install starts online.

- [ ] 25. Add the smart-downloads opt-in row
  - Crate and files: `nemo-ytm`: `OptInInfo` for smart downloads; `nemo-ui`: `OptInRow` in the offline settings.
  - Does: Uses the shared opt-in pattern: off by default; the panel shows what is sent (background fetches of songs played often, and takedown checks), the hosts `www.youtube.com` and `*.googlevideo.com`, the cap (50 songs, 25% of the limit) and the risk text (the account can be restricted; songs YouTube removes are deleted), with "Turn on" and "Cancel", neither pre-selected. Turning off is one click and offers "Delete now" for smart songs.
  - _Requirements: 19, 20, 26_
  - Depends on: offline-cache#20, integrations#? (row 43, `OptInRow`)
  - Wave: 3
  - Checks: `design.md` checklist with a screenshot; keyboard order and 200% text scale.

- [ ] 26. Write the offline-cache risk text in the README
  - Crate and files: `README.md`.
  - Does: Names the risk (background fetches may get the account restricted, songs YouTube removes are deleted, sign-out deletes the whole cache) and states that no new host is contacted.
  - _Requirements: 27_
  - Depends on: offline-cache#25
  - Wave: 3
  - Checks: the egress list matches `OptInInfo.hosts` (test of task 20).

- [ ] 27. Run the manual checks
  - Crate and files: none (results in the PR description).
  - Does: With the network unplugged: a saved album plays and a request log shows no YouTube request; offline mode switches on once by itself; sign-out shows the wipe and leaves no file; keyboard reach, focus and reduced motion on every cache screen. With a Premium account and the Premium switch on, a saved song is the Premium stream.
  - _Requirements: 3, 4, 5, 14, 23, 26_
  - Depends on: offline-cache#21, offline-cache#22, offline-cache#23, offline-cache#24, offline-cache#25
  - Wave: 3
  - Checks: the Premium check needs the owner's Premium test account.

## Definition of Done

See [section 4 of `.kiro/steering/workflow.md`](../../steering/workflow.md#4-definition-of-done). For this spec also:
- Task 13 (and any later change to the cached play path) runs `cargo test -p nemo-audio --test bit_exact` and `cargo test -p nemo-audio --test rt_no_alloc`.
- No URL, file name, path or video id is logged; the cache path never reaches the UI.
- A cached song never reads "Lossless", "HD" or "Hi-Res".
