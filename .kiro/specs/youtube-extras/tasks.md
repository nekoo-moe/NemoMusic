## Tasks

Cross-spec ids written `<spec>#?` name the task in that spec that adds the ADR 0004 Decision 4 row given in brackets; the number is filled in once that spec's `tasks.md` is committed. The `UrlResolver` trait, `Transport` and `HlsSource` are owned by the soundcloud spec (ADR 0004 rows 10 to 12) and land there in Waves 0 and 1.

- [ ] 1. Add search scope, video and channel types and the defaulted `Catalog` methods
  - Crate and files: `nemo-core` (touches nemo-core/src/catalog.rs).
  - Does: `SearchScope` (default `Music`), `ChannelRef`, `VideoSummary`, `ChannelSummary`, `ChannelDetail`, `SearchResults.{videos, channels}` with `Default`, and `Catalog::{search_scoped, channel, live_streams}` defaulting to `CatalogError::Unavailable`, so every existing catalog keeps building.
  - _Requirements: 5_
  - Depends on: none
  - Wave: 0
  - Checks: existing catalog tests pass unchanged; a test that the default methods answer `Unavailable`.
  - Steering: none

- [ ] 2. Add comment and dislike types, `CommentSource`, `VideoRatings` and the `Backend` fields
  - Crate and files: `nemo-core` (touches nemo-core/src/catalog.rs, nemo-core/src/services.rs); mock backends in `nemo-ui` and `nemo-app` set `None`.
  - Does: `CommentId`, `CommentCursor` (opaque, untrusted text), `Comment`, `CommentPage`, `CommentError`, `Dislikes`, the two traits, and `Backend.{comments, ratings}` as `None` by default.
  - _Requirements: none (groundwork)_
  - Depends on: youtube-extras#1 (same file), soundcloud#5 (services.rs)
  - Wave: 0
  - Checks: none beyond the usual.
  - Steering: none

- [ ] 3. Add `PlayerSnapshot.live`
  - Crate and files: `nemo-core` (touches nemo-core/src/player.rs).
  - Does: a `bool`, false by default. Only one task at a time edits player.rs (ADR 0004 Decision 4, third rule); order against the player.rs tasks of audio-effects, output-devices, account-actions and cast.
  - _Requirements: none (groundwork)_
  - Depends on: none
  - Wave: 0
  - Checks: none beyond the usual.
  - Steering: none

- [ ] 4. Add `captions.rs` with `SubtitleSource`, `SubtitleCue` and `CaptionError`
  - Crate and files: `nemo-core` (new nemo-core/src/captions.rs, touches nemo-core/src/lib.rs).
  - Does: the trait (`tracks`, `cues`) and types that video-playback (subtitles) and lyrics-plus (captions provider) consume. Signatures only, no I/O.
  - _Requirements: 29_
  - Depends on: video-playback#? (row 37: `SubtitleTrack` in video.rs)
  - Wave: 0
  - Checks: none beyond the usual.
  - Steering: none

- [x] 5. Add the defaulted `LyricsProvider::timeout` and honor it in the chain
  - Crate and files: `nemo-core` (touches nemo-core/src/lyrics.rs), `nemo-lyrics` (fallback chain).
  - Does: `fn timeout(&self) -> Duration` defaulting to 4 s; the chain uses each provider's value instead of the fixed limit. Approved by the owner on 2026-11-14 (third round).
  - _Requirements: none (groundwork for 31)_
  - Depends on: none
  - Wave: 0
  - Checks: chain test with a slow fake provider: cut at 4 s by default and at its own value when overridden; existing `nemo-lyrics` tests pass.
  - Steering: ADR 0002 records `LyricsProvider::timeout` (4 s default, captions 20 s).

- [ ] 6. Spike: YouTube captions through `yt-dlp`
  - Crate and files: none shipped; invented json3 fixtures in `crates/nemo-ytm/tests/fixtures/captions/` (no real lyrics text); result written into the Risks section of `.kiro/specs/youtube-extras/design.md`.
  - Does: for three songs, list `subtitles` and `automatic_captions` from `yt-dlp -J --skip-download`, write one json3 file per track without a cookie, and record whether the body is empty (PO token, issue 13075), whether the original auto track is named `<code>-orig`, and whether the audio id or the music-video id carries the captions.
  - _Requirements: 31, 35_
  - Depends on: none
  - Wave: 1
  - Checks: pass = non-empty json3 for at least one track of two songs. Fail: tasks 9 and 10 still ship, return `Empty` as "no captions", and the owner is told captions rarely work; the music-video id fallback in task 10 is dropped if the audio id always carries them; `pick_track` (task 8) follows the observed naming.
  - Steering: none

- [ ] 7. Spike: live streams and the innertube calls
  - Crate and files: none shipped; stripped fixtures in `crates/nemo-ytm/tests/fixtures/web/` (search, browse, next in both comment layouts, comments off) and a 2-second live sample; result written into the design's Risks section.
  - Does: resolve one live stream with the pinned `yt-dlp` (audio-only HLS or muxed only; MPEG-TS or fMP4), and capture `search`, `browse` and `next` answers from `www.youtube.com/youtubei/v1/`.
  - _Requirements: 6, 7_
  - Depends on: none
  - Wave: 1
  - Checks: pass = a live format the decoder plays. If muxed MPEG-TS only, task 11 includes `ts.rs`; if fMP4, `ts.rs` is not built; if neither plays, live is shown as not playable and tasks 14 and 20 list streams only. Parsers in tasks 12, 13 and 15 follow the captured shapes.
  - Steering: none

- [ ] 8. Add the pure caption functions: `parse_json3`, `cues_to_doc`, `pick_track`
  - Crate and files: `nemo-ytm` (new `captions.rs`, `captions/json3.rs`).
  - Does: json3 events to cues (input capped at 2 MiB, end capped at the next start), drop sound-tag lines and roll-up repeats, line timing only, `None` when nothing is left; `pick_track` takes an uploaded track in the video's language, else the original auto-generated one, never a translation.
  - _Requirements: 31, 32, 34, 35_
  - Depends on: youtube-extras#4, youtube-extras#6
  - Wave: 1
  - Checks: fixture tests from the design (plain, roll-up, `[Music]` and note lines, empty events, 2 MiB cap, Vietnamese and CJK); proptest that `parse_json3` never panics.
  - Steering: none

- [ ] 9. Implement `YtmCaptions` over the `yt-dlp` runner
  - Crate and files: `nemo-ytm` (`captions/runner.rs`).
  - Does: `tracks` from one `-J --skip-download` run cached 10 minutes by video id; `cues` writes only the picked track with `--skip-download --write-subs --write-auto-subs --sub-langs <code> --sub-format json3 --ignore-config` into a private temp folder, reads and deletes it (drop guard, start-up sweep). 20 s timeout, one run at a time, no `--cookies`, stderr never logged; cues kept for the last 4 videos.
  - _Requirements: 35, 36, 37, 38_
  - Depends on: youtube-extras#8
  - Wave: 1
  - Checks: fake `yt-dlp` script tests: no `--cookies` argument, temp folder gone after success, failure and timeout, empty file gives `Empty`.
  - Steering: none

- [ ] 10. Add `CaptionsLyricsProvider` and register it last in the lyrics chain
  - Crate and files: `nemo-ytm` (`captions/provider.rs`), `nemo-app` (lyrics chain wiring).
  - Does: provider "YouTube captions", off by default, its `OptInInfo` (video id to `www.youtube.com` through `yt-dlp`, unofficial access may restrict the account, text can be wrong), timeout 20 s, length check (more than 5 s or 3 percent off gives no captions), "Not lyrics-grade (auto-generated)" credit note. Registered last, built only when the switch is on and confirmed.
  - _Requirements: 29, 30, 31, 32, 33, 37_
  - Depends on: youtube-extras#5, youtube-extras#9, integrations#? (row 43: `OptInInfo`)
  - Wave: 1
  - Checks: chain test: called last and once per track; length check at 5 s and 3 percent; zero processes start while the switch is off.
  - Steering: ADR 0002 adds the "YouTube captions" row (off by default, risk note, last in the chain).

- [ ] 11. Play live streams through the shared HLS source
  - Crate and files: `nemo-ytm` (`live.rs` policy, `ts.rs` if task 7 needs it), `nemo-audio` (sets `PlayerSnapshot.live` from `Transport::Hls { live: true }`).
  - Does: the live policy picks audio-only HLS, else the lowest-bandwidth muxed rendition; `ts.rs` takes AAC packets (stream type 0x0F) out as ADTS and drops video. The stream has no length and is not seekable; it ends with an "ended" event and is never restarted by the app. The audio callback is unchanged and an empty ring gives silence without allocation (`code.md` real-time rules).
  - _Requirements: 6, 7, 8_
  - Depends on: youtube-extras#3, youtube-extras#7, soundcloud#9 (row 11: `UrlResolver` in nemo-ytm), soundcloud#12 (row 12: `HlsSource` live window), local-library#? (row 14: AAC and Opus decoders and `supported_stream_codecs()`)
  - Wave: 1
  - Checks: TS parser test on the captured 2-second sample; bit-exact test; counting-allocator test with a live source that underruns.
  - Steering: none

- [ ] 12. Search all of YouTube
  - Crate and files: `nemo-ytm` (new `web_search.rs`, `WebCatalogExt`).
  - Does: `search_scoped(AllOfYoutube)` runs the music search, then `POST /youtubei/v1/search`, one after the other; parses `videoRenderer`, `channelRenderer`, `playlistRenderer` and the "LIVE NOW" badge into `videos` and `channels`. `Music` calls `search` as today. Backoff signals are honored.
  - _Requirements: 1, 2, 5, 23_
  - Depends on: youtube-extras#1, youtube-extras#7
  - Wave: 2
  - Checks: parser tests on the stripped fixtures (video, channel, live badge, mixed); a counting fake proves the two requests are sequential.
  - Steering: none

- [ ] 13. Open channel pages
  - Crate and files: `nemo-ytm` (`web_search.rs` or `channel.rs`).
  - Does: `Catalog::channel` via `POST /youtubei/v1/browse` with the `UC...` id; tab `params` read from the answer's tab endpoints, never constants; uploads and playlists.
  - _Requirements: 4_
  - Depends on: youtube-extras#12
  - Wave: 2
  - Checks: parser test on the channel fixture.
  - Steering: none

- [ ] 14. List live stations and streams
  - Crate and files: `nemo-ytm` (`live.rs`).
  - Does: `Catalog::live_streams` from feed items with a live badge; an empty list is a valid answer.
  - _Requirements: 6, 9_
  - Depends on: youtube-extras#11, youtube-extras#12
  - Wave: 2
  - Checks: parser test on a feed fixture with and without live items.
  - Steering: none

- [ ] 15. Read comments (posting comes later)
  - Crate and files: `nemo-ytm` (new `comments.rs`, `comments/parse.rs`).
  - Does: `YtmComments::comments` and `replies` via two `next` calls, both layouts (`commentRenderer` and `commentViewModel` mutations), "Comments are turned off" as `Disabled`, pages only on request, kept in memory for the current item. `post`, `reply` and `set_liked` answer `Unavailable` and `can_post` is false until task 24 (owner decision: reading ships first).
  - _Requirements: 11, 12, 13, 15, 23_
  - Depends on: youtube-extras#2, youtube-extras#7
  - Wave: 2
  - Checks: parser tests in both layouts, replies, comments off; a counting fake proves nothing loads before the panel asks and page two only on "Load more".
  - Steering: none

- [ ] 16. Add the Return YouTube Dislike client
  - Crate and files: `nemo-ytm` (new `ryd.rs`).
  - Does: `RydClient` implements `VideoRatings`: `GET /votes?videoId=`, own limit 1 a second and 30 a minute, once per item per session, 24 h memory cache, 10-minute pause after a 429; 404, 400, `deleted`, timeout and parse failure give `Unknown`, never zero.
  - _Requirements: 18, 19, 20, 23_
  - Depends on: youtube-extras#2
  - Wave: 2
  - Checks: tests against `test_server.rs` (200, 404, 400, 429 pause, deleted, timeout); limiter with a fake clock; cache expiry at 24 h.
  - Steering: none

- [ ] 17. Register the "ytm-extras" sign-out step
  - Crate and files: `nemo-ytm` (`comments.rs`, `ryd.rs`, `captions/runner.rs`), `nemo-app` registration with `GoogleAccount`.
  - Does: one `SignOutWipe` step that empties comment pages, the dislike cache and subtitle cues, and reports a failure without stopping the other steps.
  - _Requirements: 22_
  - Depends on: youtube-extras#9, youtube-extras#15, youtube-extras#16, account-actions#? (row 26: `SignOutWipe`)
  - Wave: 2
  - Checks: sign-out test: all three stores are empty afterwards.
  - Steering: none

- [ ] 18. Wire the new services in nemo-app behind their switches
  - Crate and files: `nemo-app` (wiring), `README.md` egress list.
  - Does: builds `WebCatalogExt`, `YtmComments`, `RydClient` and `YtmCaptions` (for video subtitles) only when each switch is on and confirmed; sets `Backend.{comments, ratings}`; video results play their audio through the existing YouTube path.
  - _Requirements: 3, 10, 17, 23, 24, 37, 38_
  - Depends on: youtube-extras#10, youtube-extras#13, youtube-extras#14, youtube-extras#17, integrations#? (row 43: opt-in gate and README-list test)
  - Wave: 2
  - Checks: counting fake host test: zero requests while each switch is off or unconfirmed; the README-list comparison test.
  - Steering: README egress list adds `returnyoutubedislikeapi.com` (only when dislike counts are on; sends the video id and the IP) and notes that `www.youtube.com` also serves comments, all-YouTube search and captions (through `yt-dlp`) when those are on.

- [ ] 19. Add the scope switch, video and channel results and the channel page to the UI
  - Crate and files: `nemo-ui` (search view, result rows, channel view).
  - Does: "Music" / "All of YouTube" switch, the one-time note on first use, results labelled by kind, video items show the channel name and the real codec and bitrate badge, channel page with uploads and playlists. Where video-playback is on, a video result may show its picture through `VideoController`.
  - _Requirements: 1, 2, 3, 4, 5, 25, 27, 28_
  - Depends on: youtube-extras#18, video-playback#? (row 37: `VideoController`, only for the picture)
  - Wave: 3
  - Checks: `design.md` checklist with screenshot; keyboard-only reach with visible focus; the note shows once.
  - Steering: none

- [ ] 20. Add the Live area and the live player state
  - Crate and files: `nemo-ui` (live view, now-playing seek bar).
  - Does: "Live" label, no duration and no seek bar that implies an end, "No live streams right now", an "ended" message with one retry on the user's action; badge never says lossless.
  - _Requirements: 6, 7, 8, 9, 25, 26_
  - Depends on: youtube-extras#18
  - Wave: 3
  - Checks: `design.md` checklist with screenshot; reduced motion shows the live marker without animation.
  - Steering: none

- [ ] 21. Add the read-only comments panel
  - Crate and files: `nemo-ui` (now-playing comments panel).
  - Does: loads only when opened; author, text, like count, reply count, "Load more"; "Comments are turned off"; no post actions while `can_post` is false.
  - _Requirements: 11, 12, 13, 15, 25, 26_
  - Depends on: youtube-extras#18
  - Wave: 3
  - Checks: `design.md` checklist with screenshot; keyboard and reduced-motion checks.
  - Steering: none

- [ ] 22. Show the dislike label
  - Crate and files: `nemo-ui` (item header).
  - Does: the estimate labelled "Estimated by Return YouTube Dislike" with a link to returnyoutubedislike.com; "Unknown" when there is no figure, nothing during a 429 pause.
  - _Requirements: 20, 21, 25_
  - Depends on: youtube-extras#18
  - Wave: 3
  - Checks: `design.md` checklist with screenshot.
  - Steering: none

- [ ] 23. Add the four settings entries
  - Crate and files: `nemo-ui` (settings view through `OptInRow`).
  - Does: entries for "All of YouTube" search, "Show comments in Now Playing", "Show dislike counts" and the "YouTube captions" lyrics provider, each off by default with the texts of criteria 10, 17, 27 and 30.
  - _Requirements: 10, 17, 25, 27, 30_
  - Depends on: youtube-extras#18, integrations#? (row 43: `OptInRow`)
  - Wave: 3
  - Checks: `design.md` checklist with screenshot; each confirm panel shows before the first request.
  - Steering: none

- [ ] 24. Post, reply and like comments (later task, owner decision)
  - Crate and files: `nemo-ytm` (`comments/write.rs`).
  - Does: `create_comment`, `create_comment_reply` and `perform_comment_action` with params and tokens copied from the read answer, never built; signed-in session only (cookie and `SAPISIDHASH` through `Secret<T>`, never logged); one call per press; `Rejected` and `RateLimited` keep the user's text.
  - _Requirements: 14, 16_
  - Depends on: youtube-extras#15, account-actions#? (signed request builder and `SessionSource` in `nemo-ytm`)
  - Wave: 3
  - Checks: fixture tests for each answer; a counting fake proves one request per press. Needs the owner to approve shipping it and a throwaway test account.
  - Steering: none

- [ ] 25. Add the post, reply and like actions to the comments panel
  - Crate and files: `nemo-ui` (comments panel).
  - Does: actions shown only when signed in; "Post" sends once, shows the result and keeps the text on failure.
  - _Requirements: 14, 15, 16, 25_
  - Depends on: youtube-extras#21, youtube-extras#24
  - Wave: 3
  - Checks: `design.md` checklist with screenshot; keyboard check.
  - Steering: none

- [ ] 26. Run the manual checks
  - Crate and files: none; results in the Risks section of `.kiro/specs/youtube-extras/design.md`.
  - Does: play one live stream to its end, open a video with comments off, captions as lyrics on three songs and as subtitles on one video, one post and one reply on the test account; re-check the Return YouTube Dislike terms before release.
  - _Requirements: 8, 13, 16, 21, 32, 36_
  - Depends on: youtube-extras#19, youtube-extras#20, youtube-extras#22, youtube-extras#23, youtube-extras#25
  - Wave: 3
  - Checks: needs the owner (test account, terms re-check).
  - Steering: none

## Definition of Done

Every task meets [section 4 of workflow.md](../../steering/workflow.md#4-definition-of-done). For this spec also:
- `cargo test -p nemo-audio --test bit_exact` and the counting-allocator test (with an underrunning live source) pass after every task that touches the live path.
- A counting fake host shows zero requests and zero `yt-dlp` processes for each feature while its switch is off or unconfirmed.
- Nothing loads in the background: comments, dislike counts and captions are fetched only on the user action or chain step the criteria name.
