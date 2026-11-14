## Tasks

Cross-spec ids written `<spec>#?` name the task in that spec that adds the ADR 0004 Decision 4 row given in brackets; the number is filled in once that spec's `tasks.md` is committed.

- [x] 1. Add `Origin::SoundCloud` and `MediaUrl` to nemo-core
  - Crate and files: `nemo-core` (touches nemo-core/src/audio.rs, nemo-core/src/track.rs); the exhaustive `match` arms this breaks in `nemo-audio` and `nemo-ui`.
  - Does: `Origin::SoundCloud` always maps to `QualityClass::Lossy`, never lossless. `MediaUrl::parse` accepts https only, no user or password in the URL, and a host that matches the given list (exact or `*.suffix`).
  - _Requirements: 8, 15_
  - Depends on: none
  - Wave: 0
  - Checks: unit tests for `MediaUrl::parse` (http, credentials in URL, wrong host, `*.sndcdn.com` suffix, punycode look-alike); a test that `Origin::SoundCloud` never yields a lossless class. Bit-exact test must pass (arms in `nemo-audio` change).
  - Steering: none

- [x] 2. Add `Transport`, the four new `ResolvedStream` fields and `Track.source_url` in one change
  - Crate and files: `nemo-core` (touches nemo-core/src/stream.rs, nemo-core/src/track.rs); struct literals in `nemo-ytm`, `nemo-audio`, `nemo-ui` tests.
  - Does: adds `Transport { File, Hls { live } }`, `ResolvedStream.{transport, preview, audio_language, audio_tracks}` and `Track.source_url: Option<MediaUrl>`, all with `Default` (`File`, `false`, `None`, empty), so no later task edits every literal again (ADR 0004 Decision 4, first rule).
  - _Requirements: none (groundwork)_
  - Depends on: soundcloud#1, account-actions#? (row 13: `AudioTrackInfo` type in stream.rs; if that task already adds the two account-actions fields, this task adds only `transport`, `preview` and `source_url`)
  - Wave: 0
  - Checks: existing tests pass unchanged apart from literals; bit-exact test.
  - Steering: none

- [x] 3. Add `ByteSource::is_seekable` and forward it through `MediaAdapter`
  - Crate and files: `nemo-core` (touches nemo-core/src/services.rs), `nemo-audio/src/source.rs`.
  - Does: defaulted method returning `true`; `MediaAdapter::is_seekable` reports it to `symphonia` so a live stream is not seeked. No change on the audio callback path.
  - _Requirements: 23_
  - Depends on: none
  - Wave: 0
  - Checks: unit test with a non-seekable fake source; bit-exact test; counting-allocator test keeps passing.
  - Steering: none

- [ ] 4. Add `url_resolver.rs` with the `UrlResolver` trait and `rank_formats`
  - Crate and files: `nemo-core` (new nemo-core/src/url_resolver.rs, touches nemo-core/src/lib.rs).
  - Does: `UrlResolver` (`resolve`, `list`, `state`), `MediaQuery`, `MediaEntry`, `EntryKind`, `Site`, `FormatPolicy`, `FormatCandidate`, `ResolverState`, and the pure `rank_formats`: drop encrypted and unplayable codecs, score bitrate x efficiency (Opus 2.0, AAC 1.25, Vorbis 1.15, MP3 1.0, starting values), file before HLS of the same codec, previews below every full stream, ties to the newer codec.
  - _Requirements: 10, 11, 22_
  - Depends on: soundcloud#2
  - Wave: 0
  - Checks: table tests from the design (64 kbps Opus beats 128 kbps MP3, Opus not playable, encrypted dropped, preview last, missing bitrate playable, tie rules); proptest that the result index is always in range.
  - Steering: ADR 0004 Decision 4 rows 9 to 12 already list these items; add `UrlResolver::list` to row 11 if the text still lacks it.

- [ ] 5. Add `Backend.soundcloud`
  - Crate and files: `nemo-core` (touches nemo-core/src/services.rs), `nemo-app` and `nemo-ui` mock backends set `None`.
  - Does: `Backend.soundcloud: Option<Arc<dyn Catalog>>`, `None` by default, so the UI shows no SoundCloud entry until the app builds one.
  - _Requirements: 2_
  - Depends on: soundcloud#3 (same file)
  - Wave: 0
  - Checks: none beyond the usual.
  - Steering: none

- [x] 6. Create the `nemo-soundcloud` crate skeleton
  - Crate and files: new `crates/nemo-soundcloud` (`lib.rs`, `hosts.rs`, `opt_in.rs`); workspace member list.
  - Does: `HOSTS` (the four design hosts until the capture of task 7 replaces them), the `OptInInfo` for the switch (sends search text and page URLs through `yt-dlp`, the hosts, the draft risk text, `version` 1, `default_on` false). Depends on `nemo-core` only.
  - _Requirements: 1_
  - Depends on: integrations#? (row 43: `OptInInfo`)
  - Wave: 0
  - Checks: `cargo tree -p nemo-soundcloud --edges normal` shows only `nemo-core` among `nemo-*`; needs the owner to approve the risk text wording (requirements Open Question 1) before task 19 ships it.
  - Steering: `structure.md` adds `nemo-soundcloud` (depends on `nemo-core` only) and the edge `nemo-app -> nemo-soundcloud`.

- [ ] 7. Spike: what `yt-dlp` returns for public SoundCloud tracks, and whether `symphonia` reads AAC HLS
  - Crate and files: none shipped; fixtures in `crates/nemo-ytm/tests/fixtures/soundcloud/` (stripped of URLs and the `client_id`); result written into the Risks section of `.kiro/specs/soundcloud/design.md`.
  - Does: with the pinned `yt-dlp`, list the formats for 5 public tracks, one playlist, one user, one Go+ preview and one encrypted track; feed one AAC HLS (fragmented MP4) stream to `symphonia` 0.6.1 without an index; record every host contacted in a network capture.
  - _Requirements: 24_
  - Depends on: none
  - Wave: 1
  - Checks: pass = formats listed match the design and `symphonia` decodes the fMP4 segments. Fail on fMP4: task 11 joins the init segment and fragments into one stream; if that also fails, AAC HLS leaves `FormatPolicy.playable` (task 14) and such tracks show "cannot play here". Fail on formats: tasks 8 and 9 follow the captured JSON, not the design list. The captured host list replaces `HOSTS` (task 6) and needs the owner's approval before task 16. Needs the owner for the approval and a network capture tool.
  - Steering: none

- [ ] 8. Implement `YtDlpUrlResolver::list` and `state`
  - Crate and files: `nemo-ytm` (new `url_resolver.rs`, uses `ytdlp.rs` runner).
  - Does: `scsearch10:` and page URLs through `yt-dlp --flat-playlist -J`, one process at a time, 20 s timeout, mapped to `MediaEntry`; a 429 or rate message starts a backoff of at least 60 s for all SoundCloud calls; `state` reports `Ready`, `Missing` or `TooOld`. URLs are `Secret<String>`; `yt-dlp` stderr is never logged.
  - _Requirements: 12, 14, 15, 16_
  - Depends on: soundcloud#4, soundcloud#7
  - Wave: 1
  - Checks: parser tests on the task 7 fixtures (search, track, playlist, user, private, removed); a fake `yt-dlp` script proves one process at a time, the backoff and the timeout; no test starts the real `yt-dlp`.
  - Steering: none

- [ ] 9. Implement `YtDlpUrlResolver::resolve` with format ranking
  - Crate and files: `nemo-ytm` (`url_resolver.rs`, a `formats.rs` split if the file passes 400 lines).
  - Does: one `yt-dlp -J`, reads every audio format, drops encrypted and non-audio ones, builds `FormatCandidate`s, calls `rank_formats`, returns `ResolvedStream` with `transport`, `preview` and measured `StreamInfo`. Encrypted-only maps to `StreamError::NotAvailable`; no bypass is tried.
  - _Requirements: 5, 10, 11, 22_
  - Depends on: soundcloud#8
  - Wave: 1
  - Checks: fixture tests for full, preview-only and DRM-only answers; the chosen index matches `rank_formats`; no cookie or credential argument is passed.
  - Steering: none

- [ ] 10. Add `HlsSource` for on-demand HLS in nemo-ytm
  - Crate and files: `nemo-ytm` (new `hls.rs`, `hls/playlist.rs`, `hls/fetch.rs`), dependency `m3u8-rs` 6.0.1.
  - Does: reads the media playlist, one fetch thread, a bounded `sync_channel` of 3 segment buffers to the decoder thread, 3 tries per segment then `Network`, a playlist with `#EXT-X-KEY` other than `NONE` ends with `NotAvailable` before any segment is fetched. Uses the HTTPS-only redirect, retry and redaction rules of `http.rs`. Nothing runs on the audio callback: decoder -> `rtrb` ring -> `cpal` is unchanged (`code.md` real-time rules).
  - _Requirements: 11, 23_
  - Depends on: soundcloud#2, soundcloud#3, soundcloud#7
  - Wave: 1
  - Checks: tests against `test_server.rs`: `EXTINF` playlist, key line refused, segment 429 retried, three failures end the stream; bit-exact test; counting-allocator test keeps passing.
  - Steering: `tech.md` row for `m3u8-rs` 6.0.1 (MIT, released 2026-07-30, pure Rust, parser only; check its `nom` against `Cargo.lock` `nom` 8.0.0 and record its `rust-version`).

- [ ] 11. Add seek by segment to `HlsSource`, and the fMP4 join if the spike needs it
  - Crate and files: `nemo-ytm` (`hls.rs`, `hls/fetch.rs`).
  - Does: maps a time to a segment through the `EXTINF` sums and restarts the fetch there; the decoder's accurate-seek skip trims the rest. When task 7 failed on fMP4, prepends the init segment so `symphonia` reads one stream.
  - _Requirements: 23_
  - Depends on: soundcloud#10
  - Wave: 1
  - Checks: seek test against `test_server.rs`; fMP4 fixture decodes (when built); bit-exact test.
  - Steering: none

- [ ] 12. Add the live window to `HlsSource`
  - Crate and files: `nemo-ytm` (`hls/live.rs`).
  - Does: for `Transport::Hls { live: true }`: reload the playlist every half target duration, start 3 segments behind the live edge, drop segments that leave the window, end with an "ended" result on `#EXT-X-ENDLIST` or 3 failed reloads in a row; `is_seekable()` is false. Serves youtube-extras live streams.
  - _Requirements: none (groundwork; serves youtube-extras 6 and 8)_
  - Depends on: soundcloud#10
  - Wave: 1
  - Checks: live-window tests against `test_server.rs` (window drop, reload failure, `ENDLIST`); counting-allocator test with a source that underruns.
  - Steering: none

- [ ] 13. Add `YtmStreamOpener` and use it in place of `HttpStreamOpener`
  - Crate and files: `nemo-ytm` (`stream.rs` or new `opener.rs`), `nemo-app` wiring.
  - Does: one `StreamOpener` that reads `ResolvedStream.transport` and returns the ranged file source or `HlsSource`. YouTube Music playback is unchanged.
  - _Requirements: 5, 23_
  - Depends on: soundcloud#11, soundcloud#12
  - Wave: 1
  - Checks: opener tests for both transports; existing YouTube stream tests pass; bit-exact test.
  - Steering: none

- [ ] 14. Resolve SoundCloud tracks in `prepare`
  - Crate and files: `nemo-audio` (`prepare.rs`, `PrepareDeps.url_resolver`).
  - Does: for `Origin::SoundCloud` call `UrlResolver::resolve(source_url, policy)` with `FormatPolicy.playable` from `supported_stream_codecs()`; fill the badge from measured codec and bitrate or "Unknown", source name "SoundCloud" in the signal path, class Lossy. A resolve failure gives a plain reason and the queue moves on, as for other sources. Runs on the decoder-prepare thread, never the callback.
  - _Requirements: 5, 6, 7, 8, 9, 12, 23_
  - Depends on: soundcloud#4, soundcloud#13, local-library#? (row 14: `supported_stream_codecs()`)
  - Wave: 2
  - Checks: fake-resolver tests (badge fields, "Unknown" bitrate, failure skips to next item, a mixed queue of a YouTube song and a SoundCloud track plays in order); bit-exact test; counting-allocator test.
  - Steering: none

- [ ] 15. Implement `SoundcloudCatalog`
  - Crate and files: `nemo-soundcloud` (`catalog.rs`, `map.rs`).
  - Does: `search` (tracks to `songs`, playlists, users to `artists`), playlist and user pages through one `list` call each, `album` answers `NotFound`; titles, uploader, artwork and duration kept as the source gives them. Encrypted-only shows "cannot play here", private, removed and region-blocked map to plain `CatalogError` reasons; offline is `Network`. No sign-in path exists.
  - _Requirements: 3, 4, 11, 12, 13, 14, 21_
  - Depends on: soundcloud#5, soundcloud#6, soundcloud#8, youtube-extras#1 (row 20: catalog types)
  - Wave: 2
  - Checks: fake `UrlResolver` tests (mapping, attribution fields untouched, error texts, one call at a time); no process starts.
  - Steering: none

- [ ] 16. Wire SoundCloud in nemo-app behind the opt-in switch
  - Crate and files: `nemo-app` (wiring, settings), `README.md` egress list.
  - Does: builds `SoundcloudCatalog` and sets `Backend.soundcloud` only when the switch is on and `confirmed_version == version`; injects the resolver into `PrepareDeps` and the opener. Nothing is built, requested or shown while off. No SoundCloud value is written to a file or log.
  - _Requirements: 1, 2, 15, 19, 24_
  - Depends on: soundcloud#9, soundcloud#14, soundcloud#15, integrations#? (row 43: opt-in gate and the README-list comparison test)
  - Wave: 2
  - Checks: counting fake host test proves zero requests while off or unconfirmed; the `nemo-app` test comparing `HOSTS` with the README egress list. Needs the owner's approval of the captured host list.
  - Steering: README egress list adds each approved host (design list: `soundcloud.com`, `api-v2.soundcloud.com`, `*.sndcdn.com`, `playback.media-streaming.soundcloud.cloud`), why it is contacted, and "only when SoundCloud is on".

- [ ] 17. Show the SoundCloud result group, the "Preview" tag and the badge in the UI
  - Crate and files: `nemo-ui` (search view, result rows, now-playing badge).
  - Does: SoundCloud results in their own group apart from YouTube Music; "Preview" tag on preview tracks; badge never shows "Lossless", "HD", "Hi-Fi" or "Hi-Res"; no save-for-offline action on SoundCloud tracks; plain reason on tracks that cannot play; offline message on SoundCloud screens only.
  - _Requirements: 3, 7, 8, 10, 12, 13, 17, 18, 20_
  - Depends on: soundcloud#16
  - Wave: 3
  - Checks: `design.md` checklist with screenshot; keyboard-only reach with visible focus; reduced motion adds no motion; a test that no quality string above appears for `Origin::SoundCloud`.
  - Steering: none

- [ ] 18. Add SoundCloud track, playlist and user pages
  - Crate and files: `nemo-ui` (page views).
  - Does: title, uploader, artwork, duration and uploader name as given, with no attribution removed; play and add-to-queue like YouTube pages.
  - _Requirements: 4, 6, 17, 18_
  - Depends on: soundcloud#17
  - Wave: 3
  - Checks: `design.md` checklist with screenshot; keyboard and reduced-motion checks.
  - Steering: none

- [ ] 19. Add the SoundCloud settings entry
  - Crate and files: `nemo-ui` (settings view through `OptInRow`).
  - Does: the switch, what is sent, the hosts, the approved risk text, and the `yt-dlp` state line ("yt-dlp missing", "too old", version). No sign-in, cookie or token entry.
  - _Requirements: 1, 16, 17, 21_
  - Depends on: soundcloud#16, integrations#? (row 43: `OptInRow`)
  - Wave: 3
  - Checks: `design.md` checklist with screenshot; the confirm panel shows before the first request. Needs the owner's approved risk text.
  - Steering: none

- [ ] 20. Run the listening test and the manual checks
  - Crate and files: `nemo-core/src/url_resolver.rs` (factor constants only, if changed); results in the Risks section of `.kiro/specs/soundcloud/design.md`.
  - Does: the owner compares 5 tracks to set the `rank_formats` factors; manual play of one public track, one playlist, one preview and one encrypted track; a second network capture confirms no host beyond the approved list.
  - _Requirements: 22, 24_
  - Depends on: soundcloud#18, soundcloud#19
  - Wave: 3
  - Checks: needs the owner (listening test, capture review); `rank_formats` table tests updated to the new factors.
  - Steering: none

## Definition of Done

Every task meets [section 4 of workflow.md](../../steering/workflow.md#4-definition-of-done). For this spec also:
- `cargo test -p nemo-audio --test bit_exact` and the counting-allocator test pass after every task that touches `nemo-audio` or the HLS source.
- A counting fake host shows zero SoundCloud requests while the switch is off or unconfirmed.
- No SoundCloud track is ever labelled "Lossless", "HD", "Hi-Fi" or "Hi-Res".
