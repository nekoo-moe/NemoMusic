# App MVP: design

## Overview
`nemo-core` holds the contracts. Services implement them. `nemo-ui` draws and talks only to core traits. `nemo-app` creates the tokio runtime and wires real services into the UI. One owner per crate.

## Affected Crates
`nemo-core` (new contracts), `nemo-ytm` (network, catalog), `nemo-lyrics` (LRCLIB, chain, cache), `nemo-audio` (streaming, resample, player), `nemo-library` (local files), `nemo-ui` (screens), `nemo-app` (wiring).

## Data Flow
```
nemo-ui (iced) --PlayerCommand--> PlayerHandle --> nemo-audio Player thread
        <--PlayerEvent / position atomic--
nemo-ui --Catalog / LyricsService (async, via nemo-app) --> nemo-ytm / nemo-lyrics
Player: StreamResolver (yt-dlp) -> StreamOpener (HTTP range, ByteSource) -> symphonia decode thread
        -> [rubato only if device rate differs] -> rtrb ring -> RT render -> cpal
```
- The decode thread does blocking reads. The audio callback never blocks or allocates.
- Position for lyrics = frames written minus device latency, read from an atomic.
- Gain is a stage that exists only when volume is below 100%; at 100% there is no arithmetic.
- SponsorBlock skip is a seek on the decode side.

## Interfaces
New in `nemo-core` (written by Kiro first): `Catalog` (search, album, artist, playlist), result and `Thumbnail` types, `ByteSource` and `StreamOpener`, `PlayerControl` (send command, read position, subscribe to events), `LyricsService`, `ImageSource`. Details live in the code.

## Error Handling
Typed `thiserror` enums. A failed lyrics or SponsorBlock call never stops playback. A missing `yt-dlp` is a user-visible state, not a crash. Network failures show a calm inline message with Retry.

## Testing Strategy
- Audio: existing `bit_exact` and `rt_no_alloc` tests stay green. Add a resample test (output rate and length) and a streaming test over a fake `ByteSource`.
- Network crates: fake HTTP, synthetic fixtures. Real-network tests are `#[ignore]`.
- UI: `iced` headless snapshot tests write PNGs to `docs/ui/snapshots/`; agents open and judge them. Pure logic (palette, layout math) has unit tests.
- Contrast tests for every derived palette.

## Risks and Alternatives
- YouTube changes break `yt-dlp` or `ytmapi-rs`. Mitigation: traits, clear error states, local files still work.
- Opus decode: if `symphonia` has no Opus, prefer AAC (itag 140 style m4a) for MVP. Say so in the badge.
- Intel HD 530: live blur is unlikely at 60 fps. Default tier Low (pre-blurred cover texture), auto-downgrade per `design.md`.
- `reqwest` TLS backend: prefer the Windows native one (Schannel). Avoid backends that need cmake or NASM.
- Crates approved for this spec: `reqwest`, `image`, `rubato`, `lofty`, `rusqlite` (bundled), `unicode-normalization`, `tracing-subscriber`, `ytmapi-rs`. Each is recorded in `tech.md` with its version.
