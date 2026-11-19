## Tasks

Waves: 1 = native client and playback (agent H, PRs 1 to 3 of 2026-11-19); 2 to 4 = next agent. Each task is one PR or less. Tasks marked "next agent" are not started. Task ids are cited as `soundcloud-full#n`.

- [x] 1. Spec, ADR 0005, steering and README rows
  - Crate and files: `.kiro/specs/soundcloud-full/`, `docs/adr/0005-soundcloud-account.md`, `structure.md`, `tech.md`, `README.md`, notes in `.kiro/specs/soundcloud/`.
  - Does: records the owner's request of 2026-11-19, the protocol facts of the probe, the native-client design, the risk text and the host rows.
  - _Requirements: 1, 2, 3, 32_
  - Depends on: none
  - Wave: 1
  - Checks: `cargo fmt --all -- --check`; the `nemo-app` test that compares `HOSTS` with the README egress list is added with task 6 (the hosts of this PR are listed in README but not yet contacted).
  - Steering: as listed, each change labelled `(owner, 2026-11-19; soundcloud-full)`.

- [x] 2. HTTP layer and client id in `nemo-soundcloud`
  - Crate and files: `nemo-soundcloud` (`http.rs`, `client.rs`, `client_id.rs`, `limiter.rs`, `test_server.rs`), `Cargo.toml` (reqwest =0.13.5, tokio =1.54.1, serde, serde_json, thiserror, tracing).
  - Does: `ScHttp` over `reqwest` (HTTPS only, three redirects, timeouts, body cap, no URL in errors), the 250 ms limiter, 429 backoff, two retries on reads, client id discovery and one refresh on 401 or 403, `next_href` host check.
  - _Requirements: 2, 4, 6_
  - Depends on: soundcloud-full#1
  - Wave: 1
  - Checks: loopback tests (redirect to http refused, 429 with `Retry-After`, 5xx retry, body cap, id regex, refresh once, never twice); `cargo tree -p nemo-soundcloud --edges normal` shows only `nemo-core` among `nemo-*`.
  - Steering: none

- [x] 3. Typed responses, mapping and catalog reads
  - Crate and files: `nemo-soundcloud` (`model.rs`, `map.rs`, `catalog.rs`, `tests/fixtures/`).
  - Does: serde structs (read item by item, so one odd item is skipped, not the page), mapping to `Track` (`Origin::SoundCloud`, `source_url`), `PlaylistSummary`/`PlaylistDetail`, `ArtistSummary`/`ArtistDetail`; `search`, `search_kind`, `/resolve`, track, playlist (with `/tracks?ids=` batches), user tracks and playlists, related tracks. `HOSTS` replaced by the probed list.
  - _Requirements: 21, 22, 23, 6_
  - Depends on: soundcloud-full#2
  - Wave: 1
  - Checks: sanitised fixture tests (mapping, attribution fields kept, stub playlist filled, previews and encrypted tracks flagged), a fixture lint (no `client_id`, token, signature or foreign host).
  - Steering: `tech.md` hosts line if the probe shows another host.
  - Note: PR 2 reads the first page of every list; paging by `next_href` and the `Catalog` paging call come with task 9. `tests/live.rs` is the hand-run check against the real service (about 13 requests).

- [ ] 4. Stream lookup and ranking
  - Crate and files: `nemo-soundcloud` (`stream.rs`, `resolver.rs`).
  - Does: `ScStreamResolver` (`UrlResolver`): transcodings to `FormatCandidate`s, `rank_formats`, the stream call, `ResolvedStream` with measured `StreamInfo`, preview flag and expiry, in-memory cache, `warm`.
  - _Requirements: 3, 13, 15, 16, 18, 19, 20_
  - Depends on: soundcloud-full#3
  - Wave: 1
  - Checks: fake-HTTP tests (AAC wins over MP3 and over `abr`, encrypted-only is `NotAvailable`, preview marked, cached until expiry, 403 triggers one new lookup).
  - Steering: none

- [ ] 5. HLS reader and opener
  - Crate and files: `nemo-soundcloud` (`hls.rs`, `hls/playlist.rs`, `opener.rs`), `nemo-core` none.
  - Does: `HlsSource` (playlist parse, init and first segment in parallel, fetch thread, `sync_channel(3)`), `ScStreamOpener`; forward-only (`is_seekable() == false`).
  - _Requirements: 5, 13, 14, 17_
  - Depends on: soundcloud-full#4
  - Wave: 1
  - Checks: loopback tests (init plus three segments read as one stream, key line refused, segment 429 retried, three failures end the stream, channel bound holds); `symphonia` decodes the recorded AAC fMP4 fixture (init plus two segments) to PCM; counting-allocator test.
  - Steering: none

- [ ] 6. Playback wiring and the latency example
  - Crate and files: `nemo-audio` (`prepare.rs`, `PrepareDeps.url_resolver`), `nemo-app` (wiring, routing opener, the README host comparison test), `nemo-soundcloud/examples/latency.rs`.
  - Does: a SoundCloud track plays through the existing player; the next track is warmed; the badge shows the real codec and bitrate; the `#[ignore]` example prints stage times.
  - _Requirements: 3, 13, 14, 15, 17, 20_
  - Depends on: soundcloud-full#5
  - Wave: 1
  - Checks: fake-resolver tests in `prepare` (badge fields, failure skips to next item, mixed queue); `cargo test -p nemo-audio --test bit_exact`; counting fake host proves zero SoundCloud requests while the switch is off; the example is run once and its numbers go in the PR.
  - Steering: none

- [ ] 7. Opt-in version 2 and the SoundCloud settings entry (next agent)
  - Crate and files: `nemo-soundcloud` (`opt_in.rs`), `nemo-ui` (settings through `OptInRow`).
  - Does: `OPT_IN` version 2 with the new text and hosts; the settings entry shows the switch, what is sent, the hosts and the risk text.
  - _Requirements: 1, 32_
  - Depends on: soundcloud-full#6
  - Wave: 2
  - Checks: `design.md` checklist with screenshot; confirm panel shows before the first request.
  - Steering: `design.md` (setup flow line "Public tracks only. No sign-in needed." and a SoundCloud screens section), each labelled.

- [ ] 8. SoundCloud sign-in and sign-out (next agent)
  - Crate and files: `nemo-signin` (`--site soundcloud`), `nemo-core` (`SourceAccount`, `Backend.soundcloud_account`), `nemo-soundcloud` (`account.rs`), `nemo-app`.
  - Does: criteria 7 to 12; token only in `SecretStore` key `soundcloud/oauth-token`; sign-out wipe registered with the app's sign-out.
  - _Requirements: 7, 8, 9, 10, 11, 12_
  - Depends on: soundcloud-full#7, account-actions (sign-out wipe registry)
  - Wave: 2
  - Checks: fake store tests; the cookie filter test (only the `oauth_token` of `soundcloud.com` is returned); a test that the token never appears in `Debug`, errors or logs; needs the owner for a real sign-in. Owner Open Question 1 decides the paste fallback.
  - Steering: `tech.md` row text if the webview needs another host; `structure.md` `nemo-signin` line (already labelled in task 1).

- [ ] 9. Account library reads and the UI (next agent)
  - Crate and files: `nemo-soundcloud` (`library.rs`), `nemo-ui` (source switch, SoundCloud pages on the playlist and album components, skeleton loading).
  - Does: liked tracks, own and liked playlists, followings, history (`/users/<me>/track_likes`, `/me/library/all`, `/users/<me>/followings`, `/me/play-history/tracks`), shown in the library; like state of the playing track from `/me/track_likes/ids`.
  - _Requirements: 21, 22, 24, 30, 31_
  - Depends on: soundcloud-full#8
  - Wave: 2
  - Checks: fixtures for each list; `design.md` checklist with screenshots; keyboard and reduced-motion checks.
  - Steering: `design.md` section (labelled).

- [ ] 10. Home, mixes, charts and radio (next agent)
  - Crate and files: `nemo-soundcloud` (`home.rs`), `nemo-ui`.
  - Does: `mixed-selections`, `system-playlists/<urn>`, `charts` (the anonymous `charts` call answered 404 on 2026-11-19; read the web app's request first), related-track radio.
  - _Requirements: 23, 28_
  - Depends on: soundcloud-full#9
  - Wave: 3
  - Checks: fixtures; screenshots.
  - Steering: none

- [ ] 11. HLS seek by segment, then the `yt-dlp` fallback (next agent)
  - Crate and files: `nemo-soundcloud` (`hls.rs`), `nemo-audio` (`stream_decoder.rs` only if the decoder must reopen at a segment), `nemo-ytm` (fallback resolver, task 8 and 9 of `soundcloud`).
  - Does: criterion 17; restart the fetch at the segment of the target time (`EXTINF` sums) and let the decoder's accurate-seek skip trim the rest; the fallback resolves through `yt-dlp` only when the native lookup fails.
  - _Requirements: 17_
  - Depends on: soundcloud-full#6
  - Wave: 3
  - Checks: loopback seek test; `bit_exact`; counting-allocator test.
  - Steering: none

- [ ] 12. Account writes (next agent)
  - Crate and files: `nemo-soundcloud` (`write.rs`), `nemo-ui` (like, follow, add to playlist, playlist editor).
  - Does: criteria 25 to 27 and 29. Read the web app's request bodies first; writes are never retried; each write refreshes only the item it changed.
  - _Requirements: 25, 26, 27, 29_
  - Depends on: soundcloud-full#9
  - Wave: 4
  - Checks: fake-HTTP tests for the request method, path and body; undo on refusal; needs the owner for a real-account check.
  - Steering: none

## Definition of Done

Every task meets [section 4 of workflow.md](../../steering/workflow.md#4-definition-of-done). For this spec also:
- `cargo test -p nemo-audio --test bit_exact` and the counting-allocator test pass after every task that touches `nemo-audio` or the HLS source.
- A counting fake host shows zero SoundCloud requests while the switch is off or unconfirmed.
- No SoundCloud track is ever labelled "Lossless", "HD", "Hi-Fi" or "Hi-Res".
- No token, cookie, client id or full URL appears in a log, error text, test fixture or commit.
