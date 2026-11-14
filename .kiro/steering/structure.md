---
inclusion: always
---

# NemoMusic Project Structure

Cargo workspace. All crates live under `crates/` and are named `nemo-*`. Stack choices are in `tech.md`; coding rules in `code.md`; visual rules in `design.md`.

## Layout

```
NemoMusic/
  Cargo.toml            # [workspace], shared [workspace.dependencies], exact versions
  rust-toolchain.toml
  crates/
    nemo-core/          # domain types, traits, errors, events
    nemo-audio/         # decode, ring buffer, output, gapless, resample
    nemo-library/       # local files, SQLite, tags, scanning
    nemo-ytm/           # YouTube Music client, login, SponsorBlock, YTM lyrics
    nemo-lyrics/        # lyrics providers, parsers, matching, cache (proposed, ADR 0002)
    nemo-secrets/       # OS credential store behind the nemo-core SecretStore trait
    nemo-soundcloud/    # SoundCloud catalog of public tracks, through the yt-dlp URL resolver
    nemo-ui/            # iced views, theme, widgets, album-art effects
    nemo-app/           # binary: wiring, config, logging, startup
  assets/               # fonts/, icons/, shaders/, images/ (with licenses)
  docs/                 # architecture, signal path, ADRs in docs/adr/
  tests/                # workspace-level integration tests
  .kiro/steering/       # steering files
```

## Crate ownership

- nemo-core: `Track`, `Album`, `Source` (Local or Ytm), `AudioFormat`, `SignalPath`, `PlayerCommand`, `PlayerEvent`, shared `Error`, the `SecretStore` trait, the AI provider contract (`ai` module), the opt-in data type `OptInInfo` (`opt_in` module), the shared play-count rule `PlayRule` (`hooks` module), the OS notification contract `OsNotifier` (`notify` module), the integrations service contract `IntegrationsService` (`integrations` module), the remote-playback contract `RemotePlayback` and the one-remote-at-a-time `RemoteSlot` (`remote` module, cast) and the `Backend.{integrations, notifier}` fields, per-line lyrics data (`LineMeta`) and the own-lyrics, romanizer and lyrics-translation contracts (`lyrics_plus` module, lyrics-plus), the effects and transition settings and the `GaplessPolicy` trait (`effects` module, audio-effects). No I/O, no async runtime, no GUI. Feature `test-util` adds `MemoryStore`, a map-backed `SecretStore` for other crates' tests (dev-dependencies only).
- nemo-audio: symphonia decode, Opus decode, device enumeration, WASAPI and cpal output, optional ASIO, rubato, gapless queue, volume, signal-path report, the parked mode (`Park` and `Unpark`: a remote device plays, the queue still moves, nothing decodes and no device is held). Real-time code lives only here.
- nemo-library: SQLite schema and migrations, folder scan, lofty tag read/write, cover art extraction, search over the local catalog.
- nemo-ytm: `ytmapi-rs` catalog calls, `yt-dlp` stream resolver, Google sign-in and the session layout in the credential store (`Vault`; the store comes in as `Arc<dyn SecretStore>`, no keyring dependency), account data fetch, SponsorBlock `SegmentSource`, the YTM timed-lyrics provider, rate limiting. Returns `nemo-core` types. Owns the ToS risk notice text.
- nemo-signin: small binary that opens Google's sign-in page in a webview window (wry + tao; WebView2 on Windows, WebKitGTK on Linux), waits until YouTube Music is reached, and writes the session cookies to its stdout pipe for `nemo-ytm`. No other logic. Depends on no `nemo-*` crate. Shipped next to `nemo-app` (`nemo-app.exe` on Windows).
- nemo-lyrics: other `LyricsProvider` implementations (LRCLIB and more), LRC and enhanced-LRC parsers, match scoring, fallback chain. The `LyricsProvider` and `SegmentSource` traits live in `nemo-core`. Depends only on `nemo-core`.
- nemo-secrets: the OS credential store (Windows Credential Manager, the Linux Secret Service) behind the `nemo-core` `SecretStore` trait, and `open_platform_store()`. Owns the `keyring-core` and platform store crates. `nemo-app` opens the store and hands it to every crate that needs a secret. Depends only on `nemo-core`.
- nemo-soundcloud: the SoundCloud catalog (public tracks only, no sign-in), its host list and its opt-in settings entry. Never starts a process or opens a socket: search, pages and streams go through the `nemo-core` URL-resolver trait that `nemo-ytm` implements with `yt-dlp`. Depends only on `nemo-core`.
- nemo-ui: all iced code, theme, fonts, widgets, animations. Talks to the player only through `PlayerCommand` and `PlayerEvent`.
- nemo-app: `main`, config file, tracing setup, crate wiring, packaging metadata. No business logic.

## Allowed dependency direction

Arrows point to the crate that is depended on. Anything not listed is forbidden.

```
nemo-app
 ├─> nemo-ui ──────> nemo-core
 ├─> nemo-audio ───> nemo-core
 ├─> nemo-library ─> nemo-core
 ├─> nemo-ytm ─────> nemo-core
 ├─> nemo-lyrics ──> nemo-core
 ├─> nemo-secrets ─> nemo-core
 ├─> nemo-soundcloud ─> nemo-core
 └─> nemo-core
```

Rules:
- nemo-audio MUST NOT depend on nemo-ui, nemo-library, or nemo-ytm. Audio receives a stream or file source through a `nemo-core` trait.
- nemo-ui MUST NOT depend on nemo-audio, nemo-library, or nemo-ytm directly. It uses `nemo-core` traits and messages; `nemo-app` supplies the implementations.
- Library, ytm, lyrics and soundcloud crates MUST NOT depend on each other.
- No cycles. `nemo-core` depends on no other `nemo-*` crate.
- Check with `cargo tree -p <crate> --edges normal` before merging a dependency change.

## Where things go

- Unit tests: in the same file under `#[cfg(test)] mod tests`.
- Crate integration tests: `crates/<crate>/tests/`.
- Cross-crate end-to-end tests: `/tests/`.
- Test audio fixtures: `crates/nemo-audio/tests/fixtures/` (short, under 200 KB each, license noted in `fixtures/README.md`).
- Benchmarks: `crates/<crate>/benches/`.
- Fonts, icons, shaders: `/assets/` only. Each asset folder has a `LICENSES.md`. No binary blobs elsewhere.
- Design notes and decisions: `/docs/`. One ADR per file: `docs/adr/0001-short-title.md`.
- Steering for agents: `/.kiro/steering/`.

## Naming rules

- Crate directory and package name are the same: `nemo-audio`. Rust path uses `nemo_audio`.
- Files and modules: `snake_case`, singular nouns (`device.rs`, `resampler.rs`). No `utils.rs`, `helpers.rs`, `misc.rs`.
- One public concept per file. Split a file over 400 lines.
- Use `foo.rs` plus `foo/` for submodules, not `foo/mod.rs`.
- Types `UpperCamelCase`; traits name a capability (`AudioSink`, `StreamResolver`); errors end in `Error`.
- Messages and events: `PlayerCommand::Play`, `PlayerEvent::TrackChanged`. Past tense for events.
- Cargo features are lowercase kebab-case (`asio`, `wasapi-exclusive`).
- Asset files: `kebab-case`, with size or theme suffix (`icon-play-24.svg`).
- Test names state behavior: `gapless_trims_encoder_padding`.
- ADR and doc files: `kebab-case.md`.
