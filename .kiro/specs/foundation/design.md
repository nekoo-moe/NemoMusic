# Foundation: design

## Overview
`nemo-core` (written by Kiro, frozen for this wave) holds the shared types and traits. Four crates each get one first slice, written by separate agents with one owner per crate. No crate edits another.

## Affected Crates
`nemo-core` (done), `nemo-audio`, `nemo-lyrics`, `nemo-ytm`, `nemo-ui`. Untouched stubs: `nemo-library`, `nemo-app`.

## Data Flow
Local file -> symphonia decoder thread -> `rtrb` SPSC ring of `f32` -> render function (real-time safe) -> `AudioSink` (cpal or capture sink). `f32` holds 16-bit and 24-bit PCM exactly, so the default chain is lossless to the sink. The decoder thread owns allocation; the render function owns none.

## Interfaces
Traits in `nemo-core`: `LyricsProvider`, `SegmentSource`, `StreamResolver`. Types: `AudioFormat`, `StreamInfo`, `SignalPath`, `LyricsDoc`, `Segment`, `ResolvedStream`, `Secret<T>`, `VideoId`. Crate-local traits are allowed (`AudioSink`, `HttpGet`).

## Error Handling
`thiserror` enums per crate. No `unwrap` or `expect` outside tests. Secrets never appear in `Debug`, errors or logs.

## Testing Strategy
- Audio: `crates/nemo-audio/tests/bit_exact.rs` (the name `workflow.md` expects) and `tests/rt_no_alloc.rs` with a counting global allocator. Test files are generated in code, no binary fixtures. Device tests are `#[ignore]`.
- Lyrics and ytm: synthetic inline fixtures, `proptest` for parsers.
- UI: pure token and contrast tests; the example is measured by hand.

## Risks and Alternatives
- cpal 0.18 API and WASAPI exclusive support are unverified. Wave 1 uses shared mode only.
- Symphonia 0.6 Opus support is unverified. Wave 1 decodes FLAC and WAV only.
- `iced` glass at 60 fps is unproven. The spike measures it; the fallback is in `tech.md`.
- Parallel agents building on 4 cores and about 6 GB RAM may block on cargo's build lock. They run with `CARGO_BUILD_JOBS=2`.
