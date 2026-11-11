# Foundation: tasks

## Tasks
- [x] 1. Workspace scaffold and `nemo-core` types and traits. _Requirements: 1_
- [x] 2. `nemo-audio`: decode FLAC and WAV, ring buffer, real-time safe render, capture and cpal sinks, bit-exact and no-alloc tests, `play` example. _Requirements: 2, 3, 4_ (code and tests done; `CpalSink`, `play` and the FLAC branch have not run on a real device or file: owner to run)
- [x] 3. `nemo-lyrics`: LRC and enhanced-LRC parsers, text normalization, match scoring, best-candidate pick. _Requirements: 5, 6_
- [x] 4. `nemo-ytm`: `yt-dlp` argument builder and JSON parser, `yt-dlp` resolver, SponsorBlock request builder and response parser. _Requirements: 7, 8, 9_ (never run against live YouTube)
- [x] 5. `nemo-ui`: theme tokens with contrast tests, `iced` glass spike example with frame-time readout, findings note. _Requirements: 10_ (compiles; the window and frame times are unmeasured: owner to run)
- [x] 6. Parent review: run all checks, read every diff, commit. _Requirements: 1_

## Definition of Done
See section 4 of `.kiro/steering/workflow.md`.
