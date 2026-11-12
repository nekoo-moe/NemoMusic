# Wave brief: UI colour, motion, daily Home, lyrics switcher

Read first: `.kiro/specs/foundation/agent-brief.md`, `.kiro/specs/app-mvp/agent-brief.md`, `.kiro/steering/design.md`, `docs/ui-redesign.md`, `docs/ui-reference.md`.
Build with `$env:CARGO_TARGET_DIR = "$env:KIROCREW_SCRATCH\target-ui"`. Never `git commit` or `git push`.

## State
A previous agent was cut off. About 100 files in `crates/nemo-ui` are already edited and uncommitted (themes, controls, Library, mock catalog methods). Do NOT revert them. Run `git status --short` and `cargo build -p nemo-ui --features gui` first. `crates/nemo-app/src/merged.rs` has an uncommitted change from another agent: keep it.
You own `crates/nemo-ui/**`, `docs/ui/**`, `docs/ui-redesign.md`. Do not touch `nemo-ytm`, `nemo-audio`, `nemo-lyrics` (another agent works on load speed there).

## Owner feedback (2026-11-11)
Screens: three owner screenshots from 2026-11-11 (not stored in the repo).

1. **Whole-app background colour.** Window background and all panes take the colour of the song now playing, or of the album/playlist page being viewed (page colour wins while it is open). Use `palette.rs`. Soft multi-stop gradient (2-3 tonal colours, dark, low chroma), AA contrast kept for every text pair. Cross-fade with a spring (about 600-900 ms) when song or page changes. Panes become tinted translucent surfaces. Reduce-motion: instant swap. Theme "Dynamic" uses this; fixed palettes keep their own.
2. **Motion with accents.** The owner says the app only has a repeated "rise from bottom" animation. Remove the repeated rise on shelves and scroll. Build a deliberate system: shared-element feel when opening a card (cover grows into the header), play button morph, now-playing cover breathe/scale on play/pause, equalizer bars on the playing row, lyric line spring with scale and glow on the active line, background cross-fade, hover lift with shadow and slight cover zoom, page cross-fade with scale 0.98 to 1, chips with a sliding selection pill, queue reorder animation. Entrance stagger once per page load, short and subtle. Document the list in `docs/ui-redesign.md`.
3. **Home with daily recommendations** that change every day, seeded by the local date (stable within a day): "Your daily mix", "Because you listened to <artist>", "Rediscover" (old liked songs), "Mood of the day" (one mood chip picked by date), "New for you" (new releases), time-of-day greeting, hero banner card. Build on the existing Catalog methods in `crates/nemo-core/src/catalog.rs`: `home_feed`, `library_playlists`, `liked_songs`, `library_artists`, `history`, `explore`, `mood_playlists`. Mock must implement them. Be honest: no fake "for you" when signed out.
4. **Loading skeletons.** Album and playlist pages show a shimmer skeleton at once on click, never a blank page. The error page appears only after the call fails.
5. **Lyrics provider switcher.** Wire the `LyricsService` methods in `crates/nemo-core/src/services.rs` (`lookup_all`, `choose`, `chosen`, `credit`, `providers`, `set_provider_enabled`) into a switcher in the lyrics tab and a Settings list with on/off and the risk note. Show the Unison credit ("Lyrics from Unison" and the submitter) whenever Unison lyrics are shown.

## Verify
`cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo clippy -p nemo-ui --all-targets --features gui -- -D warnings`; `cargo test --workspace`.
Headless snapshots, plus the live mock window driven by an input script (the one used in that wave was a scratch file, not in the repo: write your own). Open every PNG. At least 3 rounds. Include before and after shots of the coloured background across a song change. Measure F3 frame times.

## Report
Max 50 lines: what changed per item, snapshot paths, frame times, flaws left.
