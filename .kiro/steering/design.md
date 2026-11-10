---
inclusion: fileMatch
fileMatchPattern: "crates/nemo-ui/**"
---

# NemoMusic Design Rules (nemo-ui)

Language: Material 3 (Expressive, standard motion scheme) for structure + Liquid Glass for the floating control layer only (owner, 2026-11-10). Rules are tokens and numbers; section 14 holds iced notes. Define tokens once in one theme module; no literals in views. Stack: tech.md. Layout: structure.md. Rust: code.md. Apple publishes no numeric glass values; glass numbers below are NemoMusic choices, tuned in the prototype.

Layout refinements: `docs/ui-reference.md` sections 3 to 6 (responsive grid, wireframes, breakpoints, styling rules) refine sections 4 and 9 and apply to every screen. Where they differ from sections 4 and 9, the refinement wins: sidebar is opaque `surface-container-low`; cards are at least 160 and flexible per the grid formula; Now Playing art is 480 (min 400, shrinks to fit H - 280) and 280 in compact mode (W < 1100 or H < 700); content bottom padding is 112 so the floating bar never hides rows; the queue panel pushes content from W >= 1280 and overlays (opaque) below that.

Owner direction (2026-11-10, evening): the first build looked "raw". Go full Material 3 Expressive: rounded surface panels, pill chips, tonal buttons, real M3 motion on every state change, a proper bundled type family with Vietnamese glyphs, and Google sign-in. The bar to beat is SimpMusic desktop (three rounded panes: navigation + library, content, a right "Now Playing" pane with large art and live lyrics, a floating player capsule). Where this direction and older numbers below disagree, this direction wins; `docs/ui-redesign.md` holds the new detailed spec.

## 1. Principles
- Art first: cover art is the largest, most colorful element. Chrome stays neutral.
- Restraint: one accent set at a time, max 2 type weights per view, no decorative borders.
- Calm: no autoplaying motion, no blinking badges, no counters.
- Honest signal: never show "lossless", "Hi-Res" or "HD" for a YouTube Music stream (lossy only). Badges show real format, rate, depth, bitrate (`Opus 160 kbps`, `FLAC 96 kHz / 24-bit`). Unknown shows `Unknown`.
- Layer rule: Material owns content and structure. Glass owns only what floats above content: player bar, navigation rail overlay, sheets, popovers, menus. Never glass on lists, cards, lyrics body, or cover art.

## 2. Color (Material roles, dark-first; light uses the same role names)
- Use M3 role names as tokens: `primary`, `on-primary`, `primary-container`, `on-primary-container`, `secondary(-container)`, `tertiary(-container)`, `error(-container)`, `surface`, `surface-container-lowest|low|(default)|high|highest`, `on-surface`, `on-surface-variant`, `outline`, `outline-variant`, `scrim`.
- Neutral baseline (dark): `surface` `#0E0F12`, `surface-container` `#16181D`, `surface-container-high` `#1E2128`, `on-surface` `#F2F3F5`, `on-surface-variant` `#A6ABB5`, `outline-variant` `#FFFFFF14`, disabled text `#6B7080` (never for content). Semantic: success `#6FCF97`, warning `#E5B567`, error `#EB7B7B`, info `#7FB2E5`. Never pure `#000`/`#FFF` surfaces.
- Dynamic color from the cover: take the dominant color, build a tonal palette in HCT/OKLCH (M3 tones 0-100), clamp chroma <= 0.14, map to roles with M3 tone rules (dark: primary T80, on-primary T20, container T30, on-container T90, surface tinted at T6). Only the accent roles and the surface tint follow the cover; neutrals stay fixed.
- Contrast guarantee (any cover): every text/role pair >= 4.5:1, non-text >= 3:1. If a derived pair fails, move tone in 1-step increments up to 10 steps; if still failing, use the fallback palette (`primary` `#8AB4F8`). Test fixtures: all-black, all-white, grey, saturated, noisy cover.
- Accent crossfade 400 ms; only one derived palette on screen. Color never carries meaning alone: pair with icon or text.

## 3. Typography (M3 type scale, mapped to px / line px)
- Roles: `font.ui` (system UI sans), `font.display` (one bundled sans), `font.mono-num`. Bundle fonts; CJK/RTL fall back to system fonts.
- Scale: `label-sm` 11/16, `body-sm` 12/18, `body-md` 14/20, `title-sm` 16/24, `title-md` 20/28, `headline-md` 28/36, `display-md` 40/48, plus lyrics sizes in section 10.
- Weights 400 and 600 (500 for display). Tabular figures for time, bitrate, rate, track number. `m:ss`, `h:mm:ss` from 1 h. Thin space before units (`96 kHz`).
- One-line ellipsis in rows; never below 11 px; OS text scale to 200% without clipping.

## 4. Shape, spacing, layout
- Spacing unit 4 px; allowed 4, 8, 12, 16, 24, 32, 48, 64.
- Shape scale (M3): `shape.xs` 4, `sm` 8, `md` 12, `lg` 16, `xl` 28, `full` 999. Cards and rows `md`; hero art `lg`; sheets `xl`; artists `full`.
- Min window 960 x 600, default 1280 x 800. Sidebar 240 (64 icons below 1100 px width). Player bar 88 high, floating, 12 px inset from window edges. Queue panel 320. Content max 1200. Track row 48 (compact 40). Padding 24. Works at 100/150/200% DPI.

## 5. Elevation and state layers
- Elevation (M3 levels): L0 none, L1 surface-container-low, L2 container, L3 container-high, L4 container-highest, L5 reserved. Express depth by tone first; shadow only on L3+: `0 4px 16px #00000052`.
- State layers on `on-surface` (or role color): hover 8%, focus 10%, pressed 10%, dragged 16%. Disabled content 38%, container 12%.
- Focus ring: 2 px `primary` plus 2 px offset, >= 3:1 against surroundings. Never remove without replacement.

## 6. Liquid Glass layer (floating controls only)
| Token | Value |
|---|---|
| `glass.blur` | 24 px radius (tier High); 12 px (Medium) |
| `glass.tint` | `surface-container` at 0.55 alpha (range 0.40 to 0.70; adapts to backdrop luminance) |
| `glass.edge` | 1 px inner highlight `#FFFFFF` at 0.22, top edge brighter (0.32), plus 1 px outer `outline-variant` |
| `glass.specular` | one soft highlight, alpha <= 0.12, follows pointer or window light; static in reduced motion |
| `glass.lens` | refraction offset <= 6 px at the edge, only on player bar and sheets; Medium tier and below: off |
| `glass.radius` | `shape.xl` 28 for bar and sheets, `shape.full` for pill controls |
| `glass.shadow` | `0 8px 24px #00000052` |
- Max ONE glass layer stacked: a glass surface never sits over another glass surface. A popover opened from the glass bar becomes opaque `surface-container-high`.
- Glass never sits behind lyric body text, track titles, or long text without a scrim: a scrim of `scrim` at >= 0.60 under the text block, or use the tonal fallback.
- Legibility rule (guaranteed AA over ANY cover): sample the backdrop luminance under the glass each frame at low rate (<= 10 Hz) and on cover change; compute the worst-case contrast of `on-surface` over tint+backdrop. If < 4.5:1 (text) or < 3:1 (icons), raise tint alpha in 0.05 steps up to 0.90; if still failing, switch that element to the tonal fallback. Controls inside glass use `on-surface` only, never the accent as text.
- Glass inside content flow is forbidden; max glass area 35% of the window.

## 7. Fallbacks and quality tiers
- Reduce transparency (OS setting `Transparency effects` off, high-contrast mode, or user toggle): all glass becomes opaque `surface-container-high`, no blur, no lens, no specular, 1 px `outline-variant` border. Layout and contrast unchanged. Also used when the GPU is software or integrated-only and misses budget.
| Tier | Blur | Lens | Specular | Dynamic tint | Budget (1280x800, 60 fps = 16.6 ms frame) |
|---|---|---|---|---|---|
| High | 24 px live backdrop | on | on | on | UI total <= 8 ms GPU, glass <= 3 ms |
| Medium | 12 px, half-res | off | static | on | UI <= 8 ms, glass <= 1.5 ms |
| Low | pre-blurred cover texture, no live sampling | off | off | off | UI <= 5 ms |
| Off | tonal surface | off | off | off | UI <= 4 ms |
- Auto-downgrade one tier when 30 consecutive frames exceed 16.6 ms; upgrade only on user request. Idle (no motion) must render 0 frames.
- No UI effect may touch, wait on, or block the audio thread. Effects run on the UI/GPU side, read only via `PlayerEvent`; frame drops never affect playback (code.md RT rules).

## 8. Motion (M3 motion tokens, standard scheme)
- Springs, no overshoot by default: `spatial.fast` (stiffness 1400, damping 1.0), `spatial.default` (700, 1.0), `spatial.slow` (300, 1.0); `effects.*` (opacity/color) same stiffness, damping 1.0. Expressive overshoot (damping 0.8) only for the play button morph.
- Duration cap for non-spring fades: `motion.fast` 100 ms, `base` 200, `slow` 400. Nothing over 400 ms. Easing fallback: emphasized-decelerate `cubic-bezier(0.05,0.7,0.1,1)` in, `cubic-bezier(0.3,0,0.8,0.15)` out, `linear` only for progress.
- Animate only: opacity, translate <= 16 px, shape morph, accent crossfade, progress, focus ring. Not: visualizers, loops, marquees, anything while a menu is open.
- Reduced motion: instant or 0 ms opacity swap, no lens or specular movement, no lyric glide (jump instead); progress keeps updating.

## 9. Album art
- Sizes `art.row` 40, `card` 160, `player` 64, `hero` 320. Radius per section 4. Never upscale above 1.0x source pixels (center at native size on `surface-container` or use a smaller slot). Request nearest size to slot x DPI.
- Missing art: neutral placeholder with note glyph. 1:1 `cover` crop. 150 ms fade-in; cache thumbnails.
- Backdrop blur from cover: >= 60 px, overlay alpha >= 0.60, static, `on-surface` keeps >= 4.5:1. Accent glow alpha <= 0.25, blur <= 48 px, hero only.

## 10. Lyrics view
- Content on `surface` with the blurred cover backdrop (section 9), never glass. Only the floating bar and header controls use glass.
- Sizes: active line `headline-md` 28/36 at 600; neighbor lines `title-md` 20/28 at 400; others `title-sm` 16/24. Active `on-surface`; others `on-surface-variant` at 0.72 (contrast >= 4.5:1 still). Max line width 640 px; left aligned (RTL aware).
- Word-level highlight: when the provider gives word timing, the active word goes `on-surface`, past words 0.9, future words 0.55, transition `effects.fast`; line-level only otherwise. No karaoke wipes or bouncing.
- Scroll: active line anchored at 35% of the viewport height, spring `spatial.default`, 200 ms equivalent; line change when `position >= start - 150 ms`. Manual scroll pauses auto-follow for 4 s, then a `Back to current line` pill returns. Seeking jumps with `motion.base`.
- Provider label: always visible footer, `label-sm`: `Lyrics: <provider name>` plus `Synced` or `Not synced`. Never hide attribution. Unknown provider shows `Unknown`.
- Unsynced fallback: static text in `title-sm` 16/24, user scrolls, no auto-scroll, label `Not synced`.
- Offset control: per track, -10.0 to +10.0 s, 50 ms steps (ADR 0002), buttons `-50 ms` / `+50 ms` and reset, tabular numerals, value shown (`+0.30 s`), stored locally per track. Keyboard `[` and `]`.
- No lyrics: `No lyrics found` with provider list tried; no fake text. Lyrics cannot be exported or copied in bulk.

## 11. Account and sign-in screen
- Primary button `Sign in with Google` (neutral Material filled tonal, no Google logo imitation). Method per ADR 0001; never an embedded password field drawn by NemoMusic.
- Plain message, `body-md`, above the button: `Everything stays on this computer. NemoMusic has no server. Your sign-in and library data are stored here, in Windows Credential Manager.`
- ToS notice (always visible, not collapsed, text per owner once set): `NemoMusic uses YouTube Music without an official API. This may break YouTube's Terms of Service and your account could be restricted.` Acknowledge checkbox is NOT pre-checked.
- Signed in: show account name and avatar (local cache), `Sign out` button. Sign out confirm text: `This removes your sign-in, library cache, history and lyrics cache from this computer.` Wipe completes before the screen returns to signed-out; show result.
- Never show tokens, cookies, or account IDs in UI or logs.

## 12. SponsorBlock indicator and ad blocking
- Skip indicator: a pill above the player bar, `label-sm`, `Skipped sponsor segment (0:42)` with `Undo` button (target >= 40 px) valid for 5 s (ADR 0003); Undo seeks to the segment start and marks that segment ignored for this play. Segment ranges tinted on the seek bar at 0.35 alpha with category in accessible text. No sound, no flashing; polite announcement.
- Settings `SponsorBlock`: switch `Skip non-music parts of music videos`, source line `Data from SponsorBlock` plus the license text required by ADR 0003 (license unverified here).
- Ad blocking switch label: `Don't request or play ads`. Helper text: `This can restrict your account and reduces income for artists. It does not unlock Premium quality.` Default state is an owner decision (ADR 0003); state always shown as text `On`/`Off`.

## 13. Signal path, icons, keyboard, accessibility
- Quality badge in the player bar opens the signal-path popover (opaque, not glass): `Source` -> `Decode` -> `Process` -> `Output`, from real engine state only. `Lossless` only for lossless local sources; `Bit-perfect` only when no resample/gain/EQ and exclusive mode; lossy shows `Lossy` in `on-surface-variant`, never success green. `Unknown` for missing values; text copyable. Badge name reads: `Opus, 160 kilobits per second, lossy`.
- Icons: one stroke set, 24 px grid, 1.5 px stroke, sizes 16/20/24; tooltip 500 ms and accessible name on icon buttons; no emoji.
- Keys: `Space` play/pause, `Left/Right` seek 5 s (`Shift` 30 s), `Up/Down` volume 5%, `N`/`P`, `/` or `Ctrl+K` search, `Ctrl+,` settings, `Esc` close. Media keys work unfocused. Single-letter keys off in text fields. Tab order follows visuals.
- Contrast >= 4.5:1 text (3:1 large and UI), targets >= 32 px (primary 40), 4 px gaps, sliders expose value text, now-playing announced politely, high-contrast OS mode: 1 px solid `on-surface` borders and no glass.

## 14. Implementation notes for iced
- Feasible: iced 0.14 uses wgpu; a custom `shader` widget can draw a glass quad with its own pipeline. Springs run from subscriptions. Cheap path (Tier Low, ship first): pre-blur the cover on a worker thread (60 px blur), draw as a texture behind the bar plus tint/edge via canvas or quad; no live backdrop.
- Live backdrop blur/refraction (Tier High/Medium): render the window content to an offscreen texture, then sample it with a dual-Kawase blur and a lens offset in a custom shader widget. The shader widget API gives access to wgpu device/queue/target; whether it exposes the already-rendered backdrop without a second render pass is unverified. Expect to render content twice or to reproduce the bar's backdrop from the known layout (cover + static panels).
- Unverified: 60 fps of High tier on integrated GPUs; DPI-correct blur radii; detecting the OS `Transparency effects` and high-contrast settings from iced (read the Windows registry/UWP UISettings in `nemo-app`, pass as a message); text-rendering over a custom shader layer.
- First prototype gate (tech.md): glass player bar at 60 fps with the budget in section 7, else Tier Low is the shipped glass and the fallback stack decision goes to the owner.
- Keep effects in `nemo-ui`; no dependency on audio crates (structure.md).

## 15. Design review checklist (all must be yes)
1. Only tokens used, no literal colors, sizes, durations?
2. Spacing on the 4 px scale; shapes from the shape scale?
3. Glass used only on floating controls, max one layer, none behind lyric text without scrim?
4. Contrast >= 4.5:1 text and 3:1 non-text over any cover, with the fallback rule?
5. Reduce transparency and the tier fallback tested?
6. Every quality label true for the source (no "lossless" on YouTube)?
7. Covers never upscaled; radius, shadow, glow within limits?
8. Motion <= 400 ms and reduced motion honored?
9. Keyboard operable with visible focus ring; targets >= 32 px?
10. Lyrics show provider, sync state, offset control, unsynced fallback?
11. Sign-in, ToS, SponsorBlock, ad-block wording matches this file and coc.md?
12. 60 fps budget met, and nothing touches the audio thread?

<!-- Sources (searched 2026-11-10):
https://m3.material.io/styles/motion/overview/specs
https://m3.material.io/styles/motion/overview/how-it-works
https://m3.material.io/styles/shape/corner-radius-scale
https://m3.material.io/blog/building-with-m3-expressive
https://m3.material.io/foundations/watches (26 color roles)
https://developer.apple.com/videos/play/wwdc2025/219/ (Meet Liquid Glass)
https://developer.apple.com/videos/play/wwdc2025/310/
https://developer.apple.com/videos/play/wwdc2025/323
https://gist.github.com/eonist/a8f0d160c7e9e37f634a15c3a33a8109 (community note: glass for navigation layer only)
Not verified from primary pages: exact M3 spring values, state layer opacities, elevation levels, tonal mapping (from M3 knowledge); all glass numbers are NemoMusic choices.
-->
