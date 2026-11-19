# Fast start: requirements

## Goal

A song starts to play quickly after the click on Play, and the next song of the queue starts at once. Target on this machine and network: under 1.5 s from Play to the first audible sample for a cold start, under 300 ms for the next song of the queue. The owner reported about 5 s for one song (2026-11-19). The work measures where the time goes first, then removes the biggest stages without asking YouTube for more than before (no new host, no new endpoint, no new access method, nothing beyond the next song's stream).

## User Stories

- As a listener, I want a song to start within a second or two of my click, so that the app feels as quick as YouTube Music.
- As a listener, I want the next song to start at once when a song ends or when I press Next, so that I never wait inside a queue.
- As the owner, I want per-stage timings in the debug log and one measuring example, so that I can see where a slow start spends its time on my machine.

## Acceptance Criteria

1. WHEN a song starts, THE SYSTEM SHALL log at debug level, with target `nemo::start`, one line per stage (`resolve`, `resolve native`, `resolve yt-dlp`, `yt-dlp fast run`, `yt-dlp full run`, `open`, `probe`, `output`, `prepared`, `first audio`) holding the stage name, the milliseconds since the stage began and, for resolver stages, whether it gave a stream.
2. THE SYSTEM SHALL NOT put a URL, a video id, a title, a cookie, a token or an error text in a stage line, or in any other log line of the resolver chain.
3. WHEN a stream was resolved less than its expiry minus 10 minutes ago, THE SYSTEM SHALL serve it from memory, with no new request (replay, seek after a stall, queue jump). The cache is never written to disk.
4. WHEN the next song of the queue is already opened by the prefetch and the user presses Next (or the song before it fails), THE SYSTEM SHALL start the opened song, with no new resolve and no new stream request.
5. WHILE a song plays, THE SYSTEM SHALL resolve and open at most the one song after it. WHEN the queue changes so that another song is next, THE SYSTEM SHALL drop the prefetch and prefetch the new next song.
6. WHEN the native path has not answered after 1.5 s, THE SYSTEM SHALL start the fallback (yt-dlp) beside it and use the first good answer; the other resolve is cancelled (its request or process ends). WHEN the native path answers or fails within 1.5 s, THE SYSTEM SHALL behave as before and start nothing extra.
7. WHEN the audio code changes, THE SYSTEM SHALL keep the output samples bit-identical (`bit_exact` and `rt_no_alloc` pass) and SHALL allocate and lock nothing in the audio callback.
8. WHEN the measuring example runs (development only, real network, `NEMO_PROBE_QUERY` or `NEMO_PROBE_IDS`), THE SYSTEM SHALL print the stage times of each song and never a URL or an id.

## Out of Scope

- A new host, endpoint or access method; any request beyond the next song's stream; a warm yt-dlp process kept running between songs.
- The player bar, lyrics, skeletons, the setup flow, the UI crates (a hover hint from the UI would need one; it is listed under Open Questions).
- New crates.
- Signed-in Premium streams.

## Open Questions

- Warm the first song on hover or on selection of a row: needs a call from the UI crate to the resolver (`warm`). Not done here; owner decides.
- Native refusal accounting: two refused native answers in a row rest the native path for 10 minutes, and a per-video "not available" counts like a bot check. If an owner log (`NEMO_LOG=debug`, lines `resolve native ok=false` and `native resolver paused after repeated refusals`) shows the pause is what sends songs to yt-dlp, tell a bot check (`LOGIN_REQUIRED`) from a missing video and count only the first. Not changed without that evidence (the Codespace IP got the bot check, so it could not be measured here).
- Open the output device while the stream resolves (hides the device open time). Needs the sample rate before the probe; not done without a measured device-open time (the Codespace has no audio device).
- Linux: yt-dlp `yt-dlp_linux.zip` (onedir) instead of the one-file build. See the design, Risks and Alternatives; `tech.md` would change its Linux sentence.
