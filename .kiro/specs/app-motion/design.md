# App motion: design

Owner pre-approved 2026-11-15. Rules: `design.md` sections 5, 7, 8 and 9; `docs/ui-redesign.md`
section 4 (the motion table this builds on).

## Overview

Most of the motion machinery exists: `motion.rs` (easing, `Crossfade`, `Spring`), `anim.rs`
(page entrance and stagger, play morph, pane, capsule, sign-in sheet, cover and lyric line
fades), the `Fx` widget (animated state layers, lift, entrance, focus ring) and the frame
subscription that runs only while `App::needs_frames()` is true. This spec adds what the review
below found missing, in three PRs:

1. Core: `motion::Presence` (a timed enter/exit tween for surfaces), the OS reduce-motion read,
   a spring rest rule that stops invisible tail frames, and app-level frame-idle tests.
2. Surfaces: every popover, menu, sheet, dialog and toast enters and leaves with a `Presence`;
   the full-window Now Playing grows its cover; the capsule rise is cut to 16 px; tooltips fade.
3. Details: sliding pills for the Now Playing tabs, the navigation and the search chips; picture
   fade-in; a still skeleton; title cross-fade on track change; the heart morph; the menu hold.

### Inventory (review of 2026-11-15)

| Surface | Change | Animated today? | Target motion (token) |
|---|---|---|---|
| Page navigation (Home, Explore, Search, Library, Mood, Settings) | route change | Yes: fade + grow 98 to 100%, 240 ms; Home staggers its items | keep (emphasized-decelerate 240 ms) |
| Detail pages (album, artist, playlist) | route change | Yes: page fade, header cover grows 86 to 100% | keep; owned by another agent now, no edits |
| Account | sign-in sheet | Yes: `anim.sheet` spring, 16 px rise, scrim fades | keep (`spatial.default`) |
| Back and forward | route change | Yes: same page fade (`after_update` restarts it on any route change) | keep; no direction slide (owner removed slides, round 4) |
| Sidebar selection | nav entry changes | No: fill swaps | one pill slides between entries (`spatial.fast`) (task 3) |
| Home and Library chips | selection | Yes: sliding pill (`spatial.fast`) | keep |
| Search filter chips | selection | No: fill swaps | sliding pill like Home (`spatial.fast`) (task 3) |
| Lyrics / Queue / Info tabs | selection | No: fill swaps | sliding pill (`spatial.fast`) (task 3) |
| Right pane (push mode) | open, close | Yes: width on `anim.pane` (`spatial.default`) | keep |
| Right pane (overlay below 1280) | open, close | Open only: entrance on the pane spring; close snaps | keep open; close follows the spring (task 2) |
| Queue panel | open, close | It is the Queue tab of the right pane: same as above | same as right pane |
| Full-window Now Playing | open, close | No: the whole window swaps | page fade + cover grows 86 to 100% (240 ms); closing gets the shell page fade (task 2) |
| Player bar: play/pause | state | Yes: circle to rounded square, expressive 1400/0.8 | keep (the one overshoot) |
| Player bar: like heart | like, unlike | No: glyph swaps | fill fades in over the outline (`effects.fast`) (task 3) |
| Player bar: progress | hover, drag, time | Progress live every playing frame; handle +1 px on hover (iced, instant) | keep; iced slider has no timed hover (out of scope) |
| Player bar: volume | drag | iced slider, instant | same as progress |
| Capsule | first track | Yes: rises 28 px and fades | rise cut to 16 px (`spatial.default`) (task 2) |
| List rows, cards | hover, press | Yes: `Fx` state layers on `effects.fast`, card lift on `spatial.fast` | keep |
| Loaded list | first appearance | Yes: items 0 to 4 stagger 28 ms, once per page | keep (capped at 5) |
| Images | picture arrives | No: pops in | fade 150 ms (`design.md` section 9) (task 3) |
| Track change | cover | Yes: cover cross-fade 400 ms in pane, capsule, backdrop | keep |
| Track change | title, artist | No: text swaps | title block fades in with the cover (`anim.cover`, 400 ms cap) (task 3) |
| Track change | accent | Yes: `Crossfade` 400 ms | keep |
| Loading skeleton | page loading | Shimmer loops every 1.4 s | still skeleton that fades in once (`motion.base`) (task 3) |
| Buffering, signing in | spinner | Turns while busy | kept (allowed in ui-redesign section 4; see open question) |
| Refresh busy | reload runs | Dimmed, no spinner | keep (nothing moves on purpose) |
| Toasts and notices | show, hide | No | `Presence` popup: fade + 8 px rise in `motion.base`, out `motion.fast` (task 2) |
| Track menu | open, close | No | `Presence` popup, scrim fades with it (task 2) |
| Signal-path popover | open, close | No | `Presence` popup (task 2) |
| Timing popover (full-screen lyrics) | open, close | No | `Presence` popup (task 2) |
| Tools panel | open, close | No | `Presence` popup (task 2) |
| Create playlist and shortcut sheets | open, close | No | `Presence` popup over a fading scrim (task 2) |
| First-run notice | show, accept | No | `Presence` popup over a fading scrim (task 2) |
| Tooltips | appear after 500 ms | No: pops | 4 px rise + text fade in `motion.fast` (task 2) |
| Queue reorder | move up or down | Yes: the two rows slide (`spatial.default`) | keep |
| Queue drag | drag | No drag in the queue (buttons move rows) | n.a. |
| Lyrics scroll to active line | line change | Yes: glide `spatial.default`, line cross-fade 240 ms | checked, keep |
| Focus ring | Tab moves focus | Ring appears at once on the focused control | keep: moving it between controls needs layout of two widgets; the instant ring is the accessible default |
| Window resize | reflow | No motion | checked: pane width follows the window at once, nothing animates on resize |
| Backdrop drift | song plays, window focused | Yes: 48 s drift at 24 fps | keep (owner override) |

## Affected Crates

- `nemo-ui`: `motion.rs` and the new `motion/presence.rs`, `gui/app/*` (state, sync, frame
  loop), views of the surfaces listed above, `gui/fx`.
- `nemo-app`: `os_motion.rs` (OS preference) and one line in `ui_bridge.rs`.

## Data Flow

All on the UI thread. iced's `window::frames()` subscription is active only while
`App::needs_frames()` is true; each frame calls `App::on_frame`, which advances every spring,
cross-fade and `Presence` by the frame time and re-points their targets (`sync_anim`). Views
read the current values; no allocation per frame. The OS preference is read once at start on a
helper thread (300 ms cap) in `nemo-app` and passed in `UiOptions.reduced_motion`; the reducer
joins it with the Settings switch (`UiState.reduced_motion`). Nothing reads or writes audio
state: the player is only asked for its position, as before.

## Interfaces

```rust
// nemo-ui, crate::motion
pub fn emphasized_accelerate(t: f32) -> f32;
pub struct Presence { /* private */ }
impl Presence {
    pub fn new(enter_ms: u32, exit_ms: u32) -> Self;
    pub fn popup() -> Self;                       // motion.base in, motion.fast out
    pub fn set(&mut self, on: bool, reduced: bool);
    pub fn restart(&mut self, reduced: bool);
    pub fn finish(&mut self);
    pub fn advance(&mut self, dt_ms: f32);
    pub fn value(&self) -> f32;
    pub fn is_settled(&self) -> bool;
    pub fn visible(&self) -> bool;
    pub fn is_on(&self) -> bool;
}
// nemo-app (private): os_motion::reduce_motion() -> bool
```

## Error Handling

No new errors. A missing `gsettings`, a failed process or a slow answer reads as "motion on".

## Testing Strategy

- Pure tests of `Presence` values at fixed times, reversal without a jump, the 400 ms cap and
  the reduced-motion swap; spring rest tests in `motion.rs`.
- App-level frame tests (`gui/app/frame_tests.rs`): a settled paused window needs no frames; a
  pane toggle and a page change ask for frames and stop within 400 ms; with reduced motion the
  same changes need none.
- The `gsettings` answer parser in `nemo-app`.
- Headless snapshots of mid-transition frames where they help (tasks 2 and 3).
- No audio code changes; no bit-exact test needed.

## Risks and Alternatives

- iced 0.14 has no layer opacity, so "fade" over mixed content is drawn as a cover in the
  colour behind (the existing `Fx` entrance), or through the alpha of the surface's own colours.
- A `Spring` now counts as settled when it moves less than its rest distance per 60 fps frame;
  the old speed test kept a unit spring running about 150 ms longer with no visible change.
- Alternative: one keyframe crate (for example `lilt`). Rejected: a new dependency for what
  `motion.rs` already does.

## Steering conflicts

- Owner override, 2026-11-15 (this wave): none needed so far; every motion here is inside
  `design.md` section 8. Proposed `design.md` edit: name the overlay enter/exit
  (`motion.base` in, `motion.fast` out) in section 8 so later surfaces follow it.
