## Tasks

Cross-spec ids written `<spec>#?` point at the task of that spec that adds the named item (ADR 0004 Decision 4 row in brackets). Replace `?` with its number once that spec's `tasks.md` is merged. This spec owns no `nemo-core` item, so it has no wave 0 task; its shared types come from lyrics-plus#1 (`LineMeta`) and lyrics-plus#6 (`AnimatedArtSource`).

- [ ] 1. Spike: Spotify Canvas lookup with an owner test account
  - Crate and files: probe in `$KIROCREW_SCRATCH` (not merged); result in `.kiro/specs/now-playing-styles/design.md` Risks.
  - Does: Using the token route of lyrics-plus#8, proves that either the Pathfinder GraphQL query on `api-partner.spotify.com` or the `canvaz-cache` protobuf call returns a Canvas clip address for a track known to have one, names the video CDN host (for example `canvaz.scdn.co`), and that the clip downloads under 12 MB. Writes a hand-made fixture with no token.
  - _Requirements: none (groundwork)_
  - Depends on: lyrics-plus#8
  - Wave: 1
  - Checks: needs the owner (throwaway Spotify account). Pass: one clip found and downloaded, hosts recorded. Fail: the Spotify source stays out; tasks 3, 13 and 15 are dropped, task 16 shows the switch as "Unavailable", and only the interface and still covers ship.

- [x] 2. Store the style and the animated-cover switches
  - Crate and files: `nemo-ui`: `src/settings.rs` (`NowPlayingStyle`, `AnimatedCoverSettings`, `UiSettings` fields); `nemo-app`: `src/config.rs`, `src/ui_bridge.rs` (`settings.json`).
  - Does: Three styles with `M3Expressive` as default, so existing users see no change; all animated-cover switches off. A missing or bad value falls back to `M3Expressive` and off.
  - _Requirements: 1, 2, 3, 19_
  - Depends on: none
  - Wave: 1
  - Checks: round-trip of `settings.json`; an old file without the fields loads as `M3Expressive`.

- [ ] 3. Add the Canvas cache and its miss table
  - Crate and files: `nemo-spotify`: new `src/canvas_store.rs` (table `canvas_miss` migration in `spotify.sqlite`, `canvas/` folder), `src/lib.rs` (`wipe` also empties both). Behind the Cargo feature `canvas`, off by default.
  - Does: Stores clips in the app's private cache only, caps 12 MB per clip and 100 MB for the folder (oldest first), keeps a miss as "none" for 7 days, and empties everything on Spotify sign-out, NemoMusic sign-out and "Clear cache".
  - _Requirements: 21, 23, 28_
  - Depends on: now-playing-styles#1, lyrics-plus#14
  - Wave: 1
  - Checks: migration test; eviction at the cap; miss expiry with a fake clock; wipe test finds no clip and no row.

- [x] 4. Dispatch the Now Playing views on the style
  - Crate and files: `nemo-ui`: `src/gui/views/now_playing.rs`, `src/gui/views/pane.rs`.
  - Does: `view` picks the builder per style for the right pane, full-screen Now Playing and the compact layout; `M3Expressive` is today's code unchanged. Playback, lyrics position, scroll spring and `compact_tab` live in `App`, so a style change restarts nothing.
  - _Requirements: 4, 5, 6, 7, 9_
  - Depends on: now-playing-styles#2
  - Wave: 2
  - Checks: style change keeps position, offset, scroll and `compact_tab`; the facts list of criterion 5 is in every tree; existing snapshots of the M3 style unchanged; `cargo test -p nemo-audio --test bit_exact` stays green (audio untouched).

- [x] 5. Add the Classic style
  - Crate and files: `nemo-ui`: new `src/gui/views/np_classic.rs`.
  - Does: A calm column: large still cover, title, artist, seek bar, controls, badge, signal-path entry, lyrics one tap away on a tab. Existing tokens and plain `surface`; no breathing cover, no canvas gradient, no new color roles.
  - _Requirements: 5, 8_
  - Depends on: now-playing-styles#4
  - Wave: 2
  - Checks: same-facts test; `design.md` checklist with screenshot.
  - Steering: `structure.md` lists the new view files `np_classic.rs`, `np_apple.rs`, `np_apple_lyrics.rs` (one edit for all three).

- [ ] 6. Add the Apple Music style layout and its `design.md` exceptions
  - Crate and files: `.kiro/steering/design.md` (sections 8, 9, 10); `nemo-ui`: new `src/gui/views/np_apple.rs`, `src/gui/views/np_apple_lyrics.rs` (`apple_style`, `AppleLine`), `src/layout.rs` (Apple sizes).
  - Does: First UI task of the style, so it makes the `design.md` change: an "Apple Music style" subsection with the four exceptions of criterion 18 and the size table, and a line in section 9 that an animated cover only fills the cover slot. Builds the wide layout (cover, titles and controls on one side, large lyrics on the other) and the compact single column with a lyrics tab; lyrics sit on a scrim, never glass; footer, credit, granularity label and offset control stay visible and keyboard reachable.
  - _Requirements: 5, 10, 16, 17, 18_
  - Depends on: now-playing-styles#4
  - Wave: 2
  - Checks: same-facts test; targets of at least 32 px, visible focus ring, reduce-transparency fallback; `design.md` checklist with screenshot. The owner approved the `design.md` change by approving the spec (ADR 0004, Actions for the owner).
  - Steering: `design.md` sections 8, 9 and 10 as above.

- [ ] 7. Compute the word fill fraction
  - Crate and files: `nemo-ui`: `src/gui/views/np_apple_lyrics.rs` (`piece_fill`, `PieceFill`).
  - Does: Returns the sung fraction per piece from `lyric_pieces` timing plus the user's offset. Line-timed lines fill as a whole; unsynced lyrics never fill; missing end times use the next start.
  - _Requirements: 11, 12_
  - Depends on: now-playing-styles#6
  - Wave: 2
  - Checks: pure tests for syllable-, word- and line-timed lines, offset shift, missing end, unsynced; proptest that fractions stay in 0.0..=1.0.

- [ ] 8. Draw the word-by-word fill and line roles
  - Crate and files: `nemo-ui`: `src/gui/views/np_apple_lyrics.rs`.
  - Does: Two stacked texts per piece (dim and full ink), the full one clipped to the sung width from `textfit::width`, lines wrapped with `Row::wrap`. Active, neighbor and other lines use the Apple sizes with the `spatial.default` spring, no overshoot; inactive lines dimmed but at 4.5:1. Line-timed lyrics fill the whole line and the footer says "Synced by line".
  - _Requirements: 11, 12, 14, 18_
  - Depends on: now-playing-styles#7
  - Wave: 2
  - Checks: `frame_stats.rs` at 60 fps with a 40-word active line. If it fails, fall back to the per-piece fade of `lyric_pieces.rs` (no `design.md` exception needed) and report it. No work on the audio thread.

- [ ] 9. Show duets and background vocals
  - Crate and files: `nemo-ui`: `src/gui/views/np_apple_lyrics.rs`, backend call to `LyricsService::line_meta`.
  - Does: Background-vocal lines one step smaller at the same ink alpha; singer 0 left, singer 1 right-aligned, max width 640 px. Without meta the style still works with no sides.
  - _Requirements: 13_
  - Depends on: now-playing-styles#8, lyrics-plus#1, lyrics-plus#11
  - Wave: 2
  - Checks: TTML duet fixture renders both sides; empty meta renders like a single singer; snapshot.

- [ ] 10. Draw the Apple backdrop
  - Crate and files: `nemo-ui`: `src/gui/views/np_apple.rs`, `src/canvas.rs`.
  - Does: The cover-colored canvas drifts its gradient angle about 12 degrees over 40 s, redrawn at most 20 times per second; it is static when reduced motion is on (setting or OS) or animated covers are off. No clip in the backdrop.
  - _Requirements: 14, 15, 17, 18_
  - Depends on: now-playing-styles#6
  - Wave: 2
  - Checks: the `canvas.rs` contrast test covers every Apple ink on the synthetic covers, still and drifted, at 4.5:1 and 3:1; frame budget at 60 fps.

- [ ] 11. Add the cover slot with still covers
  - Crate and files: `nemo-ui`: `src/gui/views/bar.rs` (`CoverVisual`, `cover_slot`), the three style builders.
  - Does: All styles draw the cover through `cover_slot`, which wraps `cover_art` and only knows `CoverVisual::Still` for now. The cross-fade to a later loop is at most 400 ms and the still always sits under it.
  - _Requirements: 19, 24_
  - Depends on: now-playing-styles#5, now-playing-styles#6
  - Wave: 2
  - Checks: snapshots unchanged for still covers; reduced motion gives no cross-fade.

- [ ] 12. Add the animated-cover lookup controller
  - Crate and files: `nemo-ui`: new `src/animated_cover.rs`.
  - Does: On track change, checks switch on, reduced motion off, Now Playing visible and `ClipPlayer::available()`, waits 1.5 s, sends one `find` per enabled source, caches a miss for 7 days, never retries. Holds the still frame while minimized, hidden, off Now Playing or paused; stops the clip on a style change; at most one clip.
  - _Requirements: 19a, 23, 25, 26, 27_
  - Depends on: now-playing-styles#2, now-playing-styles#11, lyrics-plus#6, video-playback#? (`ClipPlayer` trait, row 39)
  - Wave: 2
  - Checks: fake sources and fake `ClipPlayer`: no `find` while a switch is off, reduced motion is on, off screen or no decoder; a skip within 1.5 s makes no call; no repeat within 7 days (fake clock); one clip at most; style change stops it.

- [ ] 13. Add the Spotify Canvas source
  - Crate and files: `nemo-spotify`: new `src/canvas.rs` (`SpotifyCanvasSource`), behind the `canvas` feature.
  - Does: Implements `AnimatedArtSource` with the route of task 1: finds the clip for the song now playing, downloads it into the private cache, labels it "Animated cover: Spotify". `info` says it uses the user's own sign-in, is unofficial, may restrict the account, and sends the Spotify track id. Never claims audio quality.
  - _Requirements: 20, 23, 28, 29, 31_
  - Depends on: now-playing-styles#1, now-playing-styles#3, lyrics-plus#23
  - Wave: 2
  - Checks: fixture from the spike; size cap returns `TooLarge` and counts as "none"; counting fake host sees zero requests while off.
  - Steering: README egress `api-partner.spotify.com` (if lyrics-plus did not add it) and the Canvas CDN host, "only when Spotify animated covers are on".

- [ ] 14. Play the loop in the cover slot
  - Crate and files: `nemo-ui`: `src/gui/views/bar.rs` (`CoverVisual::Loop`), `src/animated_cover.rs`.
  - Does: Pulls the newest frame from `VideoFrameSource` on the animation tick, at most 30 per second, silent. A stall holds the last frame and shows the still after 2 s; a lookup, download or decode failure keeps the still with no error on Now Playing. The audio path shares nothing with it.
  - _Requirements: 24, 25, 27, 29, 30_
  - Depends on: now-playing-styles#12, video-playback#? (`ClipPlayer` with frame source in `nemo-video`, row 39)
  - Wave: 2
  - Checks: fake frame source: stall falls back after 2 s; pause rules hold; `frame_stats.rs` at 60 fps with a clip; `cargo test -p nemo-audio --test bit_exact` stays green.

- [ ] 15. Wire the animated-art sources into the app
  - Crate and files: `nemo-app`: backend wiring of the source list and `ClipPlayer`.
  - Does: Builds `SpotifyCanvasSource` only when the `canvas` feature is built, the switch is on and `confirmed_version` matches; passes the list to the UI through `nemo-core` traits. The "spotify" sign-out step of lyrics-plus also clears Canvas.
  - _Requirements: 19, 21, 23_
  - Depends on: now-playing-styles#13, lyrics-plus#25
  - Wave: 2
  - Checks: `cargo tree -p nemo-ui --edges normal` has no `nemo-spotify` or `nemo-video`; sign-out test finds no clip.

- [ ] 16. Add the style cards and animated-cover rows in Settings > Now Playing
  - Crate and files: `nemo-ui`: `src/gui/views/settings.rs`.
  - Does: Three style cards with a small preview each, applied at once without a restart. The Spotify Canvas switch uses `OptInRow` with the warning of criterion 20; while no decoder is built it shows "Unavailable" with the reason and looks nothing up. A failure shows one line under the switch ("Spotify: signed out", "Offline", "Video not supported"). No Apple Music animated-cover row.
  - _Requirements: 1, 3, 19, 19a, 20, 22, 30_
  - Depends on: now-playing-styles#5, now-playing-styles#6, now-playing-styles#12, integrations#? (`OptInRow`, row 43)
  - Wave: 3
  - Checks: keyboard order, focus ring, 200% text; `design.md` checklist with screenshot.
  - Steering: none; ADR 0004 Consequences already marks Apple Music animated covers Later.

- [ ] 17. Take the style snapshots and update the UI docs
  - Crate and files: `docs/ui/snapshots/`, `docs/ui-redesign.md` (section 6c).
  - Does: Snapshots of each style at 1280 x 720, 1920 x 1080 and compact, dark and light, with reduced motion and reduced transparency; adds the three styles to section 6c.
  - _Requirements: 2, 4, 5, 14, 17_
  - Depends on: now-playing-styles#9, now-playing-styles#10, now-playing-styles#16
  - Wave: 3
  - Checks: manual review of the snapshots against the `design.md` checklist.

- [ ] 18. Turn on animated covers once `nemo-video` ships
  - Crate and files: `nemo-app` and `nemo-spotify` features (`canvas` on in the release build).
  - Does: Starts only when video-playback's decoder is part of the app. Enables the feature, so the switch becomes available (still off by default), and runs the live path end to end.
  - _Requirements: 19a, 24, 26, 27_
  - Depends on: now-playing-styles#14, now-playing-styles#15, now-playing-styles#16, video-playback#? (`nemo-video` decoder in the app)
  - Wave: 3
  - Checks: manual check with the owner's Spotify test account: loop plays silently, pauses when hidden, never downloads under reduced motion, playing a music video blocks the clip (`Overloaded`); `cargo test -p nemo-audio --test bit_exact` stays green.

## Definition of Done

See section 4 of [`.kiro/steering/workflow.md`](../../steering/workflow.md#4-definition-of-done). Also for this spec:
- Audio is not touched: `cargo test -p nemo-audio --test bit_exact` stays green; no real-time path is added, so no counting-allocator test.
- `M3Expressive` stays the default and its snapshots do not change.
- No animated cover is looked up or downloaded while its switch is off, reduced motion is on, or no decoder is built.
