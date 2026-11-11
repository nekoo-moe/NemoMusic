# App MVP: requirements

## Goal
When the owner is back, `cargo run -p nemo-app --release` opens a window in the Material + Liquid Glass style. The owner can search YouTube Music, play a track with real audio, see synced lyrics, use the queue, and play local files. Playback is glitch-free and the UI is smooth on the owner's machine (Intel HD Graphics 530, so the default glass tier is Low).

Owner go-ahead (auto-nudge goal, 2026-11-10): approve everything that stays inside the steering files. Crates and hosts needed for this spec are approved when they are recorded in `tech.md` and the README egress list. Sign-in, ToS and credential topics keep the safeguards in `coc.md`.

## User Stories
- As a listener, I want to search and play any track, so that the catalog is usable from the first launch.
- As a listener, I want a Now Playing view with large art, quality badge, signal path and synced lyrics, so that listening feels like an art piece.
- As a listener, I want a queue, seek, volume and next/previous, so that normal listening works.
- As an owner of local FLAC/WAV files, I want to play them with a true-lossless badge.
- As the owner, I want honest labels everywhere: a YouTube stream is never "lossless", and any resample or gain shows in the signal path.

## Acceptance Criteria
1. WHEN the app starts, THE SYSTEM SHALL open one window within 2 seconds on the reference machine and show the shell: navigation, content area and floating player bar.
2. WHEN the user searches for text, THE SYSTEM SHALL show songs, albums, artists and playlists from YouTube Music, with covers, without signing in.
3. WHEN the user plays a YouTube track, THE SYSTEM SHALL resolve the stream with `yt-dlp`, stream it over HTTP, decode it and output it, and SHALL show the real codec and bitrate in the badge.
4. WHEN the device sample rate differs from the stream, THE SYSTEM SHALL resample only then, and SHALL show a Resample stage in the signal path.
5. WHILE a track plays, THE SYSTEM SHALL report zero underruns in a 10-minute test with the UI scrolling, and the render path SHALL allocate nothing.
6. WHEN the user seeks, pauses, resumes, skips or changes volume, THE SYSTEM SHALL respond without a click or a stall of more than 200 ms.
7. WHEN a track starts, THE SYSTEM SHALL fetch lyrics from LRCLIB and the local chain, show the best match with provider name and sync state, and highlight the current line from the audio position.
8. WHEN the cover changes, THE SYSTEM SHALL derive the accent palette and keep every text pair at 4.5:1 or better.
9. WHEN `yt-dlp` is missing, THE SYSTEM SHALL say so in plain words with the install step, and local files SHALL still play.
10. WHEN SponsorBlock is on and a music video has a non-music segment, THE SYSTEM SHALL seek past it and show an Undo for 5 seconds.
11. THE SYSTEM SHALL list every contacted host in the README and SHALL send no telemetry.
12. WHEN `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` run, THE SYSTEM SHALL pass.
13. WHEN a UI screen is built, THE SYSTEM SHALL have a PNG snapshot of it in `docs/ui/snapshots/` that an agent has looked at and judged against `design.md` and `docs/ui-reference.md`.

## Out of Scope
Google sign-in UI (wave 4, needs the owner at the keyboard), WASAPI exclusive mode (wave 4), global media keys, mobile, installers, auto-update.

## Open Questions
Whether the Intel HD 530 can hold live glass blur at 60 fps (default tier stays Low until measured). Whether `symphonia` decodes the Opus that YouTube serves (the AAC format is the fallback).
