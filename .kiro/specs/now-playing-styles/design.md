## Overview

One setting, `NowPlayingStyle`, picks which view builder draws the Now Playing surfaces. Today `gui/views/now_playing.rs` (full window) and `gui/views/pane.rs` (right pane) build the Material 3 Expressive look. They stay as they are and become the `M3Expressive` branch. Two new branches sit beside them: `Classic` (a calm column) and `AppleMusic` (large lyrics, word-by-word fill). Everything the styles share is already a function: `cover_art`, `titles`, `seek_row`, `transport`, `badge`, `offset_controls`, the lyrics footer. Each style only arranges them, so criterion 5 (same facts everywhere) holds by construction, and nothing below the UI changes (criterion 6).

Animated covers are two layers. The source side is a `nemo-core` trait, `AnimatedArtSource` (defined in the lyrics-plus design, because it shares `nemo-spotify` with Spotify lyrics). The playing side needs the video decoder of `video-playback`. That decoder does not exist yet, so this release ships the trait, the settings switches shown as unavailable (criterion 19a), the `CoverSlot` hook in the UI, and the Spotify Canvas source behind a Cargo feature that stays off until `nemo-video` lands. Still covers are the only thing users see first.

Apple Music animated covers are Later (criterion 22). Checked 2026-11-14: the official route is the Apple Music API attribute `editorialVideo`, which needs a developer token signed with a paid account's private key, and an Apple forum thread reports the attribute is not returned. The route public tools use reads the HLS (`.m3u8`) address from the public album page. That is unofficial scraping, covers albums rather than songs, and needs HLS playback from `nemo-video`. No code or trait variant is added for it.

## Affected Crates

- `nemo-ui`: `NowPlayingStyle` and `AnimatedCoverSettings` in `settings.rs`; new `gui/views/np_classic.rs`, `np_apple.rs`, `np_apple_lyrics.rs`; `now_playing.rs::view` and `pane.rs::view` dispatch on the style; `CoverSlot` in `bar.rs` next to `cover_art`; style cards (with a small snapshot each) and the animated-cover rows in `settings.rs`; `layout.rs` gets the Apple sizes.
- `nemo-core`: uses `AnimatedArtSource` and `LineMeta` (singer, background) from the lyrics-plus design. Needs a video frame trait from `video-playback` (see Interfaces). Adds nothing else.
- `nemo-spotify` (new, designed in lyrics-plus): `SpotifyCanvasSource`, cache folder `canvas/`, table `canvas_miss` in `spotify.sqlite`.
- `nemo-app`: `config.rs` and `ui_bridge.rs` carry the new settings in `settings.json`; builds the source list and passes it to the UI backend; wipes Canvas on sign-out.
- `nemo-video` (video-playback design): decodes the clip. `nemo-ui` never depends on it.
- `nemo-audio`, `nemo-library`, `nemo-lyrics`, `nemo-ytm`: no change.

## Data Flow

- Style change: the UI thread swaps the builder on the next `view()`. Playback state, the lyrics position and the scroll spring live in `App`, not in a view, so nothing restarts (criterion 7). A running clip is stopped on a style change (criterion 27).
- Animated lookup, in order: the track changes; the UI checks switch on, reduced motion off, Now Playing visible, decoder present; it waits 1.5 s (a skipped song never triggers a request); it sends one `find` to each enabled source on the runtime; a hit gives a file path in the source's private cache; the UI hands that path to the decoder through the frame trait. A miss is stored as "none" for 7 days. No retry loop (criterion 23).
- Frames: the decoder runs on its own thread (video-playback) and the UI pulls the newest frame on its animation tick, at most 30 per second, into an `iced` image handle. At most one decoder exists. The audio path never waits for it, shares no buffer with it, and is not touched. The clip is silent: the frame trait has no audio.
- Pause rules (criterion 25): the UI tells the frame source to hold the still frame while the window is minimized or hidden, the screen is not Now Playing, or playback is paused. Reduced motion (the app setting or the OS) means no `find` and no download (criterion 26).
- Apple backdrop: the song-colored canvas of `canvas.rs` with a slow drift of the gradient angle (about 12 degrees over 40 s), redrawn at most 20 times per second and frozen when reduced motion is on. The cover slot may hold a clip; the backdrop never does (settled in the requirements).
- Word fill in the Apple style: each timed piece of the active line is two stacked text widgets, dim and full ink; the full-ink one sits in a container with `clip(true)` and a width equal to the sung fraction of the piece width from `textfit::width`. The fraction comes from `lyric_pieces` timing plus the offset. Lines wrap with `Row::wrap`. Line-timed lyrics fill the whole line at once (criterion 12).

Type and spring choices of the Apple style against `design.md` sections 8 to 10 (all sizes already exist in `theme.rs`):

| Item | `design.md` today | Apple Music style |
|---|---|---|
| Active line | `headline-md` 28/36 at 600 | `display-md` 40/48 at 700 (wide), `headline-md` 28/36 (pane, compact) |
| Neighbor line | `title-md` 20/28 at 400 | `headline-sm` 24/32 at 600 |
| Other lines | `title-sm` 16/24 | `title-lg` 22/28 at 600 |
| Inactive ink | `on-surface-variant` at 0.72 | the same; no line goes below 4.5:1 |
| Background vocals | not specified | one step smaller, same ink alpha |
| Duets | left aligned | singer 0 left, singer 1 right-aligned, max width 640 px |
| Word highlight | per-piece fade, no wipe | smooth fill inside the active piece, as long as the piece lasts (at least 100 ms) |
| Line scroll | `spatial.default` 700, 1.0, anchor 35% | the same spring and anchor |
| Active-line scale | none | `spatial.slow` 300, damping 1.0, 1.0 to 1.0 (size changes by style, no overshoot) |
| Backdrop | static | slow drift, frozen under reduced motion |

## Interfaces

```rust
// nemo-ui/src/settings.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NowPlayingStyle { Classic, #[default] M3Expressive, AppleMusic }
impl NowPlayingStyle { pub const ALL: [Self; 3]; pub const fn label(self) -> &'static str; }

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AnimatedCoverSettings { pub spotify_canvas: bool }       // all off by default
// UiSettings gains: pub now_playing_style: NowPlayingStyle, pub animated_covers: AnimatedCoverSettings

// nemo-ui/src/gui/views/np_apple_lyrics.rs
pub struct AppleLine<'a> { pub line: &'a LyricLine, pub meta: &'a LineMeta, pub role: LineRole }
pub fn piece_fill(line: &LyricLine, t_ms: i64) -> Vec<PieceFill>;     // sung fraction 0.0..=1.0 per piece
pub struct PieceFill { pub text: String, pub fraction: f32 }
pub const fn apple_style(role: LineRole, dense: bool) -> TextStyle;   // sizes below

// nemo-ui/src/gui/views/bar.rs
pub enum CoverVisual { Still, Loop { frame: iced::widget::image::Handle } }
pub fn cover_slot<'a>(app: &App, size: f32, radius: f32) -> Element<'a, Message>;  // wraps cover_art, cross-fades at most 400 ms

// nemo-core/src/animated_art.rs: defined in lyrics-plus/design.md, used here unchanged
// AnimatedArtSource { id, hosts, info, find, clear_cache }, ArtQuery, ArtLookup, AnimatedArt, ArtSourceInfo

// nemo-spotify
pub struct SpotifyCanvasSource;       // impl AnimatedArtSource; label "Animated cover: Spotify"
```

Needed from `video-playback` (not defined here): a `nemo-core` trait that opens a local MP4 file, silent and looping; reports width and height; `pause`, `resume`, `stop`; and returns the newest RGBA frame without blocking. It must allow only one open clip, and report "no decoder in this build" so the switches can show as unavailable.

## Error Handling

- Any failure of lookup, download or decode keeps the still cover and shows nothing on Now Playing (criterion 30). The reason is one line under the switch in Settings > Now Playing ("Spotify: signed out", "Offline", "Video not supported"). `AnimatedArtError` is `thiserror`: `SignedOut`, `Offline`, `Refused(status)`, `Unsupported`, `TooLarge`, `Store(String)`.
- A clip over the size cap (12 MB per clip, 100 MB for the folder, oldest first) is dropped and counted as "none".
- A decoder stall is the decoder thread's problem: the UI holds the last frame and shows the still after 2 s without a new frame. The audio thread cannot see it.
- Style or settings that fail to load default to `M3Expressive` and all switches off.

## Testing Strategy

- Pure tests: `piece_fill` for syllable-timed, word-timed and line-timed lines (no invented timing for line-timed), the offset shifts the fill, missing end times use the next start, unsynced lyrics never fill.
- Style tests: changing the style keeps `playback position`, `lyrics offset`, `scroll` and `compact_tab` unchanged; the same facts list (criterion 5) is present in all three trees.
- Contrast: extend the `canvas.rs` test over every ink of the Apple style on the synthetic covers, on the still and the drifted backdrop, at 4.5:1 and 3:1.
- Animated: with fake sources, no `find` call happens while a switch is off, reduced motion is on, the screen is not Now Playing, or the decoder is absent; a skipped track within 1.5 s makes no call; a miss is not asked again for 7 days (fake clock); one decoder at most; a style change stops the clip; the cache evicts at the cap; sign-out empties it.
- Snapshots in `docs/ui/snapshots/` for each style at 1280 x 720, 1920 x 1080 and compact, in dark and light, with reduced motion and reduced transparency.
- Frame budget: the existing `frame_stats.rs` check at 60 fps for the Apple style with a 40-word active line.
- Audio is untouched, so `cargo test -p nemo-audio --test bit_exact` only has to stay green. No real-time code is added; the counting-allocator test does not apply.

## Risks and Alternatives

- Spotify Canvas is unofficial. Public write-ups name a protobuf call (`canvaz-cache`), which one tool calls deprecated, and the Pathfinder GraphQL query on `api-partner.spotify.com`; the video comes from a Spotify CDN. Host, request and token are unverified and have changed before. A spike with an owner-supplied account comes first; if it fails, the source stays out and only the interface ships. Spotify may restrict the account (the owner accepts this for lyrics; the switch repeats the warning for Canvas, criterion 20).
- Everything animated waits on `nemo-video`. If it slips, users get three styles and no animation. That is a smaller product, not a broken one. Alternative rejected: a private decoder here (a second video stack to maintain, against ADR 0004).
- The word fill uses a clipped copy of the text. It is untested at 40 words on `tiny-skia`; the fallback is the per-piece fade of `lyric_pieces.rs`, which already passes the rules and needs no `design.md` exception.
- The Apple style needs the singer and background flags, which `LyricsService` does not return today. The lyrics-plus design adds `LineMeta` and `LyricsService::line_meta`. Without them the style shows no duet sides or quieter background lines but still works.
- Apple animated covers: Later. If the owner later accepts page scraping, the work is an `AnimatedArtSource` plus HLS in `nemo-video`; nothing here blocks it.
- Classic drops the cover "breathing" animation and the canvas gradient and uses plain `surface` tokens, to be calm. That is a design choice; `design.md` sections 6 to 9 are not changed by it.
- No departure from ADR 0004. The brief says Rust 1.97 and `rust-toolchain.toml` says 1.99.1; this spec adds no crate.

## Steering and README changes

- `design.md`: add an "Apple Music style" subsection to sections 8, 9 and 10 with the four exceptions of criterion 18 and the table above, and a line in section 9 that an animated cover only fills the cover slot and never the backdrop. The change rule of `design.md` needs the owner's approval, given by approving this spec.
- `structure.md`: the new `nemo-ui` view files; `nemo-spotify` (from lyrics-plus).
- `tech.md`: no new row in this spec. The video decoder row belongs to `video-playback`.
- README egress list: the Spotify Canvas hosts (`api-partner.spotify.com`, a Spotify video CDN host such as `canvaz.scdn.co`, both unverified), added in the task that first contacts them. `accounts.spotify.com` and `open.spotify.com` come with lyrics-plus.
- ADR 0004: mark Apple Music animated covers as Later in the consequences or in the `nemo-spotify` row note.
- `docs/ui-redesign.md`: add the three styles to section 6c once built.

<!-- Sources:
.kiro/specs/now-playing-styles/requirements.md; docs/adr/0004-feature-wave-crates.md; .kiro/steering/design.md sections 6 to 10
crates/nemo-ui/src/gui/views/now_playing.rs (view, wide, compact), pane.rs, backdrop.rs, bar.rs (cover_art), lyrics.rs; crates/nemo-ui/src/lyric_pieces.rs, canvas.rs, settings.rs, theme.rs (type scale), gui/textfit.rs
crates/nemo-app/src/config.rs, ui_bridge.rs (settings.json)
https://github.com/m8tec/apple-music-animated-artworks (HLS read from public pages, scraping); https://developer.apple.com/forums/thread/696843 (editorialVideo)
https://github.com/squeeeezy/Wolframe-spotify-canvas; https://github.com/Delitefully/spotify-canvas-downloader (sp_dc needed); https://github.com/itsmeow/Spicetify-Canvas
-->
