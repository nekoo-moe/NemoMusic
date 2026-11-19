# Cold start: design

## Overview

`.kiro/specs/fast-start/` timed a warm process: resolve about 70 ms, open about 60 ms, first audio about 150 ms natively, 3.5 s through yt-dlp. The owner's 7 s is the first resolve in a fresh process, which nobody had measured. This spec adds that measurement (`cold_start`, `cold_net`) and then removes, in the order of the numbers, what a fresh process pays that a warm one does not. Stage names and the `nemo::start` target are those of fast-start; this spec adds four.

Work order (value over effort): (1) measure; (2) the first play never waits for, or fails because of, the installer, and the stream opener's HTTP client is built before the click; (3) a persisted resolved-URL cache; (4) start-up warm-up, precache of two songs and the pointer hint from the UI; (5) a fast fallback when the native path is resting; (6) only if yt-dlp is still the slow path, an in-process native extractor with a local JS cipher solver (own PR, own approvals).

## Affected Crates

- `nemo-ytm`: stage lines (`innertube.rs`, `stream.rs`, `ytdlp/managed.rs`), later the persisted cache (`resolver/`), the warm-up, the fallback timing.
- `nemo-app`: `examples/cold_start.rs`; later the warm-up task and the wiring of the cache, the cache wipe and the pointer hint.
- `nemo-ui`: later one hook in `UiOptions` for the pointer hint, nothing else.
- `nemo-audio`: only the precache count, if needed; the sample path is not touched.

## Data Flow

`cold_start` builds what `backend::build` builds for the play path (credential store and account, catalog client, yt-dlp locator and installer thread, resolver chain, SponsorBlock source, player) with a null sink, then sends `PlayerCommand::Play` and waits for the first non-zero sample. A tracing subscriber prints only events whose target starts with `nemo`, so no HTTP library line (which can carry a URL) is shown.

## Interfaces

No public signature changes in this first step. Later steps add to `nemo-ytm` a persisted store behind the existing `CachedResolver`, and one optional hook in `UiOptions`. Each is listed in the task that adds it.

## Error Handling

Stage lines carry fixed names and numbers only. The examples print an outcome and a time, never an error text from the network.

## Testing Strategy

Fakes only in `cargo test`: the play path does not await the installer, the warm-up runs once and ends on exit, a hover resolve has at most one in flight and ends on leave, nothing logs a URL, an id or a token. Real services only in the two examples, by hand, about 8 requests per run, 1 to 5 s apart.

## Risks and Alternatives

- The measurements come from a Linux Codespace in the same network as the YouTube edge (TCP connect 2 ms). Round trips at the owner's home are 20 to 100 times longer, so TLS and DNS costs are understated here.
- The Codespace gets the bot check for some videos on every client, yt-dlp included, so yt-dlp cold numbers exist only for videos it let through.
- Rejected: a long-lived yt-dlp helper process (complexity; yt-dlp keeps its player cache on disk).

## Persisted stream cache (task 3)

Supersedes criterion 3 of `fast-start` ("the cache is never written to disk"), by the owner's approval of 2026-11-19. `CachedResolver::with_persisted(store, check)` adds a file behind the memory cache. `StreamStore` keeps `cache/streams.json` in the app folder: video id, address, expiry, codec, bitrate, format, language and track list, for the automatic request only (at most 64, the one that expires first leaves first). Written through a temporary file and a rename, mode 0600 on Unix (the per-user `%LOCALAPPDATA%` on Windows). An entry is used only when it is valid for 10 more minutes, and after `UrlCheck` (one range request for the first byte, 4 s) says the address answers; a dead address, or a stream the opener could not open (`stream_failed`), drops the entry. Within one run the memory cache serves the song as before, so the check happens once per song per run. `StreamStore` is a `SignOutWipe` step (`streams`) registered next to the offline cache, and the file sits under the app root, so "clear all data" removes it. Addresses are never logged: stage lines hold the stage, a time and an outcome only.
