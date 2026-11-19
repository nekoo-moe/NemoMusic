# Fast start: design

## Overview

Most of the chain exists: a per-video stream cache (`CachedResolver`), the native innertube path with yt-dlp as fallback (`FallbackResolver`), fast yt-dlp arguments, a 256 KiB first window, a 200 ms prefill and a prefetch of the next song (`Core::prefetch_next`). What was missing is the evidence: no stage was timed, so nobody could say where 5 s go. This spec adds the timing first (stage lines at debug level and one measuring example), then changes only what the numbers and the code reading support:

- The prefetched next song is reused when the user presses Next (it was thrown away and opened again), and only the next song is resolved ahead (the hint covered two).
- When the native request has not answered after 1.5 s (its own timeout is 8 s per client profile, up to 16 s), yt-dlp starts beside it and the first good answer wins. The loser is cancelled: the resolvers' spawned tasks now abort when their future is dropped, which kills the yt-dlp child (`kill_on_drop`).

Nothing else changes in what is asked of YouTube.

## Affected Crates

- `nemo-ytm`: `timing.rs` (stage lines), `resolver.rs` (stage lines, the race), `task.rs` (`AbortOnDrop`), `ytdlp.rs` and `innertube.rs` (stage lines, abort on drop), `examples/fast_start_stages.rs`.
- `nemo-audio`: `timing.rs`, `prepare.rs`, `control.rs`, `control_tick.rs` (timing calls); `control.rs` and `control_cmd.rs` (reuse of the opened next song; hint for one entry). No change in the sample path.

## Data Flow

Play or Next reaches the control thread, which starts a prepare thread (`resolve`, `open`, `probe`). The control thread then loads the decoder into the feeder, which decodes 200 ms into the ring and releases the callback (`first audio` is logged when the control thread sees the release). Timing uses `Instant` on the control and prepare threads only; the audio callback is untouched. The prefetch result (`NextSlot`) is an owned `Prepared` held by the control thread; reusing it moves it into the current run, no copy, no new thread.

## Interfaces

No public signature changes. Test-only: `timing::capture::lines_of` in `nemo-ytm`. Internal: `Core::take_ready_next(entry) -> Option<Box<Prepared>>`.

## Error Handling

Stage lines carry fixed names and numbers only. A prefetched song that failed stays `NextState::Failed` and the normal path runs (resolve again, fallback). A reused decoder is a normal `Prepared`: its failures go through `fail_current` as before.

## Testing Strategy

- `nemo-ytm` unit tests: stage line fields; no URL or id in any log line over a miss, a hit, a warm and a failed stream; the race (the first good answer wins whichever side it is, the loser is cancelled, a failing side waits for the other, a fast native answer starts nothing); a dropped resolve kills its yt-dlp process (Linux).
- `nemo-audio` integration tests with fakes (no device, no network): Next starts the prefetched song with no second resolve or open; only one entry is warmed; a queue edit drops the prefetch.
- Bit-exact: `cargo test -p nemo-audio --test bit_exact` and `--test rt_no_alloc` (audio crate touched).
- Real network only in `examples/fast_start_stages.rs`, by hand.

## Risks and Alternatives

- The Codespace IP is refused by the YouTube bot check after a few requests (both native clients and yt-dlp answer "sign in"), so yt-dlp extraction time could not be measured here. The yt-dlp arguments are therefore left as they are.
- The race has no field number behind it: native answered in 71 ms or refused in about 100 ms here, and no slow case was seen. It bounds the worst case (a stalled native request) at 1.5 s plus a yt-dlp run instead of up to 16 s plus a yt-dlp run. A race can start one extra yt-dlp run when native is merely slow; that is the same fallback request that would follow a native failure.
- Not changed for lack of evidence: the 10 minute rest of the native path after two refusals (see Open Questions in the requirements).
- Rejected without numbers: a shared HTTP client across songs (the open took 73 ms including TLS; the prefetch hides it), opening the output device early (no device here).
- Linux onedir (`yt-dlp_linux.zip`): the one-file build took 0.9 s per `--version` run here, once per yt-dlp resolve and twice when the fast run fails. Unpacking needs a zip reader and no crate for it is approved, so it would call `unzip` and keep the one-file build when `unzip` is missing. Not part of the first PRs.
