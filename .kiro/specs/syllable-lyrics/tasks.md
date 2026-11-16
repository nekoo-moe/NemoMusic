Spec pre-approved by the owner on 2026-11-15.

## Tasks

- [x] 1. Add the LyricsPlus syllable provider to the lyrics chain, off by default. _Requirements: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13_
  - Crate and files: `nemo-lyrics` (`src/lyricsplus.rs`, `src/lyricsplus/{parse.rs,tests.rs}`, `src/http.rs`, `src/service.rs`, `src/service/tests.rs`, `src/lib.rs`, `tests/providers_smoke.rs`, `tests/fixtures/{README.md,lyricsplus-v2-word.json,lyricsplus-404-skipped.json,lyricsplus-404-miss.json}`); `nemo-ui` (`src/gui/views/settings_lyrics.rs`, test only); `README.md` (egress list); `docs/adr/0002-lyrics-providers.md` (addendum).
  - Does: parses V2 answers into syllable, word or line docs with background, singer and romanization data; never reads spread or label-only timing; scores the matched song; credits LyricsPlus and the upstream service; a 404 with a skipped source is a short-cached error; sits after Better Lyrics API in the default chain, off with a risk note.
  - Checks: `cargo fmt --all -- --check`; `cargo clippy -p nemo-lyrics -p nemo-ui -p nemo-app --all-targets --all-features -- -D warnings`; `cargo test -p nemo-lyrics -p nemo-ui -p nemo-app --all-features`.
  - Steering: coc.md (lyrics rights, egress list, risk out loud), code.md (errors, tests, file size), tech.md (no new dependency), ADR 0002.

## Definition of Done

See [workflow.md section 4](../../steering/workflow.md#4-definition-of-done).
