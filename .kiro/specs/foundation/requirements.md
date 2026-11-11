# Foundation: requirements

## Goal
Stand up the Rust workspace and the first working slice of each core crate, so the owner can play a local lossless file through a verified bit-exact chain and see the lyrics, stream and UI building blocks work on synthetic data. No network calls in tests.

## User Stories
- As the owner, I want a workspace that builds and passes fmt, clippy and tests, so that every later change starts from green.
- As a listener, I want a local FLAC or WAV file to reach the output untouched, so that I can trust the bit-perfect claim.
- As a listener, I want lyrics files parsed and matched correctly, so that synced lyrics show at the right time.
- As the owner, I want YouTube Music stream info and SponsorBlock answers parsed safely, so that later network code has a tested core.
- As the owner, I want a glass player-bar prototype with a frame-time readout, so that I can decide on `iced` with a measurement.

## Acceptance Criteria
1. WHEN `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` run, THE SYSTEM SHALL pass all three.
2. WHEN a 16-bit and a 24-bit WAV test file is decoded and rendered through the default chain into a capture sink, THE SYSTEM SHALL produce samples equal to the source samples.
3. WHILE the audio render function runs for 10 seconds of simulated callbacks, THE SYSTEM SHALL perform zero heap allocations in it.
4. WHEN the output device cannot take the source sample rate, THE SYSTEM SHALL return a format-mismatch error and SHALL NOT resample.
5. WHEN an LRC or enhanced-LRC text is parsed, THE SYSTEM SHALL return time-ordered lines, apply the `[offset:]` tag, and mark word-timed lyrics as `Granularity::Word`.
6. WHEN a lyrics candidate is scored, THE SYSTEM SHALL reject it if both durations are known and differ by more than 3 seconds, and SHALL accept only scores of 70 or more.
7. WHEN `yt-dlp` JSON is parsed, THE SYSTEM SHALL pick the best audio-only format, label its origin `YoutubeMusic`, and keep the URL inside `Secret`.
8. WHEN a `yt-dlp` command is built, THE SYSTEM SHALL pass the video id only after `--`, as an argument list, never through a shell.
9. WHEN a SponsorBlock request is built, THE SYSTEM SHALL send only the first 4 hex characters of the SHA-256 of the video id.
10. WHEN the glass spike example runs, THE SYSTEM SHALL show the average and worst frame time over the last second.

## Out of Scope
Real network calls, sign-in, the lyrics cache, resampling, WASAPI exclusive mode, the local library, packaging, CI.

## Open Questions
See `product.md`, open decisions 9 to 12, and ADRs 0001 to 0003.
