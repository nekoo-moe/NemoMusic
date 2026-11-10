---
inclusion: always
---

# NemoMusic Tech Stack

Priorities: 1) sound quality, 2) beauty, 3) calm daily use. Versions checked on 2026-11-10 against crates.io. Status is `decided` or `proposed - owner to confirm`. Pin exact versions in `Cargo.toml` (no `^` ranges for audio and UI crates). See `structure.md` for crate placement and `code.md` for coding rules.

## Ground rules

- YouTube Music serves only lossy audio (about 256 kbps AAC, Opus). The UI must never label a YouTube stream "lossless". Lossless badges apply only to local FLAC, ALAC, WAV.
- "Bit-perfect" means: no resampling, no volume scaling in software, no DSP, no mixer when the device and format allow. Any stage that alters samples must show in the signal-path view.
- Unofficial YouTube Music access may breach YouTube's Terms of Service. This is a recorded risk (see YouTube Music section). Do not hide it in UI or docs. The owner accepted it on 2026-11-10, including ad blocking and SponsorBlock.
- Local only: no server owned by the project. Every third-party host the app contacts is listed in the README. A new host needs owner approval.

## Stack

| Area | Choice | Reason | Main rejected alternative | Status |
|---|---|---|---|---|
| Language | Rust, edition 2024, stable toolchain | Memory safety with no GC pauses on the audio thread | C++ (unsafe by default) | decided |
| Decode | `symphonia` 0.6.1 | Pure Rust, FLAC/ALAC/WAV/AAC/Opus-container/MP3 support, gapless info | `ffmpeg` bindings (large C dependency, build pain on Windows) | decided |
| Opus decode | `symphonia` lacks an Opus codec; add `opus` or `audiopus` crate behind `nemo-audio` | Needed for YouTube Opus streams | Forcing AAC only (loses the best YouTube stream) | decided (owner ok 2026-11-10); symphonia Opus gap unverified |
| Device output | `cpal` 0.18.2 (shared mode, default path) | Maintained, cross-platform for Linux/macOS later | `rodio` (hides the stream, no bit-perfect control) | decided |
| Exclusive mode | `wasapi` 0.27.0 directly for WASAPI exclusive on Windows | cpal's WASAPI backend targets shared mode; exclusive needs direct control | Shared mode only (mixer may resample) | decided (owner ok 2026-11-10); cpal/WASAPI exclusive support unverified |
| ASIO | Optional cargo feature `asio` using cpal's `asio-sys` 0.4.0 | Some DACs ship only ASIO drivers; needs the Steinberg SDK, so keep it opt-in | Mandatory ASIO (licensing and build burden) | decided (owner ok 2026-11-10) |
| Resampling | `rubato` 5.0.1, only when the device rejects the source rate | Quality sinc resampler in pure Rust; unused on the bit-perfect path | Always resample to a fixed rate (breaks bit-perfect) | decided |
| Gapless | Own pre-buffer: decode the next track into a ring buffer before the current ends, trim encoder delay and padding from tags | Needs control over the output stream | Per-track stream reopen (audible gap) | decided |
| Audio thread queue | `rtrb` 0.4.1 lock-free SPSC ring buffer | No locks or allocation in the callback | `std::sync::mpsc` (allocates, may block) | decided (owner ok 2026-11-10) |
| Async runtime | `tokio` 1.54.1 | De facto standard; `reqwest` and `ytmapi-rs` depend on it | `async-std` (less maintained) | decided |
| Library DB | `rusqlite` 0.41.1 (bundled SQLite, WAL mode) | Embedded, one file, sync API fits a worker thread | `sqlx` 0.9.0 (async and compile-time checks, heavier for a local file) | proposed - owner to confirm |
| Tags | `lofty` 0.25.5 | Reads and writes ID3v2, Vorbis, MP4, APE tags and embedded art | `symphonia` metadata only (read-only, thinner) | decided |
| YouTube Music | `ytmapi-rs` 0.3.4 for catalog; `yt-dlp` subprocess for stream URL resolution | See below | Pure innertube client for streams | decided (owner ok 2026-11-10) |
| Ad blocking | Client never requests or plays ads; no filter lists expected | Direct stream resolution, no ad player | Filter-list blocker (needs a browser engine) | proposed - ADR 0003 |
| SponsorBlock | Public SponsorBlock API, hash-prefix lookup, in `nemo-ytm` | Skips non-music parts of music videos | Self-hosted mirror (breaks "no project server") | proposed - ADR 0003 (database license to check) |
| Google login | ADR 0001 recommends WebView2 sign-in that captures session cookies, with guided cookie paste as the permanent fallback; OAuth rejected (needs a Google Cloud client secret, innertube acceptance reported broken) | Local only, no project server, no shipped secret | Project-run auth relay (forbidden) | proposed - ADR 0001, spike first |
| Credential store | Windows Credential Manager via `keyring-core` + `windows-native-keyring-store` (the `keyring` crate is now a sample/CLI crate; versions unverified) | Tokens never in files or logs | Config file (forbidden by `coc.md`) | proposed - ADR 0001 |
| Lyrics | Provider chain behind a `LyricsProvider` trait: YTM timed lyrics, LRCLIB, local `.lrc`/tags, more providers | Several open sources, best sync wins | One provider only | proposed - ADR 0002 |
| UI | `iced` 0.14.0, design language Material + Liquid Glass (owner, 2026-11-10) | See below and `design.md` | `egui`, `slint`, `gpui`, Tauri + web | framework proposed - owner to confirm; design language decided |

## YouTube Music access

| Option | For | Against |
|---|---|---|
| innertube / ytmusicapi-style client (`ytmapi-rs`) | In-process, fast, typed search/browse/library | Stream URLs need signature deciphering and PO tokens; breaks when YouTube changes; the crate is young (0.3.x) |
| `yt-dlp` subprocess | Large community patches extractor breakage within days; handles signatures and tokens | Extra binary to ship and update; process spawn latency; license of the bundled binary must be checked before shipping |
| `rusty_ytdl` | Pure Rust | Last release 2024-08-10, effectively unmaintained: rejected |

Pick: `ytmapi-rs` for search, browse, and library metadata; `yt-dlp` for resolving playable stream URLs. Both sit behind one trait in `nemo-ytm` so either can be replaced.

Risks (all must stay in README and the first-run dialog):
- May breach YouTube's Terms of Service; the account used could be restricted. Never ship a default account or API key.
- Can break at any time; the app must degrade to local-only playback with a clear message.
- Streams are lossy; no "lossless" label, show codec and bitrate instead.
- Offline caching of YouTube audio is an open owner decision (see `product.md`, `coc.md`). Until the owner decides, build no audio cache; keep only metadata and thumbnails.
- Cookies or auth headers go in the OS credential store, never in logs or config files.

## UI framework comparison

| Framework | GPU animation | Typography | Custom drawing (album-art effects) | Windows maturity |
|---|---|---|---|---|
| iced 0.14.0 | wgpu renderer, animation via subscriptions | Good (cosmic-text, shaping, fallback) | Canvas and custom wgpu `shader` widget | Good; API still changes between releases; last release Dec 2025 |
| egui 0.37.0 | Immediate mode redraws, fine | Weaker text layout and styling | Painter and custom wgpu callbacks | Very mature; look is tool-like, hard to make calm and refined |
| slint 1.18.1 | Declarative animations, smooth | Good | Limited; custom shaders are awkward | Mature; license is GPL, royalty-free, or commercial, must be checked |
| gpui 0.2.2 | Excellent (Zed) | Excellent | Strong, GPU-first | Windows supported by Zed; crate is young, docs thin, API unstable |
| Tauri 2.12.5 + web | Best (CSS, WebGL, canvas) | Best (browser text engine) | WebGL and canvas, very flexible | Mature; WebView2 adds RAM, a JS toolchain and a second language to the audio app |

Liquid Glass needs backdrop blur and refraction. This is the main risk of iced. The first prototype must prove a glass player bar at 60 fps (see `design.md`, iced notes), else use the fallback below.

Pick: iced. Reason: pure Rust, wgpu gives GPU effects through a custom shader widget, text is good enough for a refined look, and no web runtime sits beside the audio engine. Fallback if the album-art effects cannot be built in iced within the first prototype: Tauri + web with `nemo-audio` kept as a Rust sidecar crate. The UI must stay in `nemo-ui` so the swap touches one crate.

## Change rules

- Adding or swapping a crate in the table above requires updating this file in the same change.
- A new dependency must be maintained (release within 12 months), license-compatible with the repo `LICENSE`, and checked with `cargo deny` once configured.
- No dependency on `nemo-ui` from audio code (see `structure.md`).

<!-- Sources (crates.io API, queried 2026-11-10):
https://crates.io/crates/symphonia 0.6.1
https://crates.io/crates/cpal 0.18.2
https://crates.io/crates/wasapi 0.27.0
https://crates.io/crates/asio-sys 0.4.0
https://crates.io/crates/rubato 5.0.1
https://crates.io/crates/rtrb 0.4.1
https://crates.io/crates/tokio 1.54.1
https://crates.io/crates/rusqlite 0.41.1
https://crates.io/crates/sqlx 0.9.0
https://crates.io/crates/lofty 0.25.5
https://crates.io/crates/ytmapi-rs 0.3.4
https://crates.io/crates/rusty_ytdl 0.7.4 (last update 2024-08-10)
https://crates.io/crates/iced 0.14.0
https://crates.io/crates/egui 0.37.0
https://crates.io/crates/slint 1.18.1
https://crates.io/crates/gpui 0.2.2
https://crates.io/crates/tauri 2.12.5
Not verified by search: cpal WASAPI exclusive-mode support, symphonia Opus support, slint licensing terms. Confirm before relying on them.
-->
