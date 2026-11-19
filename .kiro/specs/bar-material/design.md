## Overview

Three small parts, one PR each, in this order.

1. The bar. `bar/surface.rs` draws one container: a flat `surface-container-high` fill at `glass::BAR_ALPHA` (0.90) or `glass::BAR_ALPHA_LIGHT` (0.92), corners `shape::xl()`, the level 3 shadow from `glass::ELEVATED_*` and `glass::SHADOW`, and a border of width 0. The rim, the glint, the gradient and the 1 px `outline-variant` edge are removed, together with the glass fill tokens they used. The fill is opaque when `reduce_transparency` is on or `App::window_glass().is_clear()`. `GlassTier` and the `glass_tier` key stay, unused by the bar. The Appearance card becomes a text-only `Player bar` card, and the preview bar uses the same fill function (`bar::capsule_fill`).
2. Restart now. `appearance.rs` shows a pill beside the Window glass switch while `WindowGlass::needs_restart()` holds. The pill sends `Message::Restart`; `App::update` calls the new `UiOptions::on_restart` callback (`Option<RestartHandler>`, `Arc<dyn Fn() -> bool + Send + Sync>`) only while the pill is on offer (the switch needs a restart, the hook exists, no restart has started). On `true` it returns `iced::exit()` and ignores later presses; on `false` it shows one toast sentence and keeps running. `nemo-app` implements the callback in `src/restart.rs`: ask the settings saver thread to answer once everything sent before is on disk (`SettingsMsg::Flush`, 2 s at most), then spawn `std::env::current_exe()` with `std::env::args_os().skip(1)`, and return whether both worked. The pill uses the app's `Presence::popup()` (`motion.base`, rise 8 px, Reduce motion instant) and sits in a reserved slot under the switch, so nothing moves when it comes and goes.
3. Windows blur. `nemo-app` gets `window-vibrancy = "=0.8.1"` under `[target.'cfg(windows)'.dependencies]`. A new `UiOptions` callback `on_window_ready: Option<WindowHandler>` (`Arc<dyn Fn(&dyn iced::window::Window, WindowLook) + Send + Sync>`) receives the window through `iced::window::run`, run once by the shell right after the app opens, and a `WindowLook` (dark or light, the `surface` colour of the theme, the opacity). It runs only when the window was created see-through and Reduce transparency is off. `WindowLook::tint()` (pure, in `nemo-ui`) gives the acrylic tint: the base colour at the opacity as alpha. The callback body lives in `nemo-app/src/blur.rs` behind `cfg(windows)`: `window_vibrancy::apply_mica` first, `apply_acrylic` if that fails, one `tracing::warn!` if both fail. `nemo-ui` never names `window-vibrancy`, and the Linux build passes no hook.

## Affected Crates

- `nemo-ui`: bar surface, tokens, settings docs, Appearance page, preview, copy, snapshots, `UiOptions` callbacks, the restart event.
- `nemo-app`: `on_restart` and `on_window_ready` implementations, the Windows dependency.

Not touched: `nemo-audio`, `nemo-core`, `nemo-ytm`.

## Data Flow

UI thread, plus the settings saver thread that already exists. The restart request goes from the pill to `Message::Restart`, to `App::update`, to `UiOptions::on_restart` in `nemo-app`, which waits for the saver thread to write the settings (they are saved on every change already, so this is a flush) and spawns the child. The parent leaves through `iced::exit()`, so the audio device is released by the normal shutdown path before the child opens it. Nothing runs on the audio thread. The bar draws no more frames than before; the pill's rise and fade use the shared motion clock and draw 0 frames when idle.

## Interfaces

```rust
// nemo-ui: theme/glass.rs
pub const BAR_ALPHA: f32 = 0.90;
pub const BAR_ALPHA_LIGHT: f32 = 0.92;

// nemo-ui: gui/views/bar/surface.rs
pub fn solid(app: &App) -> bool;
pub fn fill_alpha(light: bool, solid: bool) -> f32;
pub fn fill(app: &App) -> Rgba;

// nemo-ui: UiOptions (PR 2 and 3)
pub type RestartHandler = Arc<dyn Fn() -> bool + Send + Sync>;
pub on_restart: Option<RestartHandler>;
pub on_window_ready: Option<WindowHandler>;
```

## Error Handling

- Spawn or save failure: the callback logs the reason with `tracing::warn!` and returns `false`; the app shows one plain sentence (`NemoMusic could not restart. Close it and start it again.`) and keeps running. No panic, no `unwrap`. If the settings saver does not answer within 2 s, nothing is started, because the new copy would read the old settings.
- Windows effect failure: one `tracing::warn!` with the effect name and the error text, never a handle value; the plain transparent window stays.
- Old config files with `glass_tier`: load as before.

## Testing Strategy

- Bar: unit tests for the alpha rule (0.90, 0.92, opaque), for the style (no border, flat fill, shadow numbers) and the existing contrast sweep over all fixture covers and modes, adapted to the new fill. Snapshot PNGs `bar-material-*` and a pixel probe across each edge at 2x (no 1 px step, see the PR text).
- Restart: model tests for when the pill shows (only `NeedsRestart`), for one request per press, for the spawn-failure path with a fake callback, and for no pill under Reduce transparency or with nothing changed. A snapshot of the card with the pill, and 3 to 5 key frames of its entrance.
- Windows: not testable here; CI's Windows job compiles it. The tint maths is a pure function with a unit test on every platform.
- No audio path is touched, so no bit-exact test applies.

## Risks and Alternatives

- The CPU renderer draws the shadow under a translucent fill, so the shadow shows through at 10% and darkens the capsule by about 3%. The contrast test includes it.
- The restart may race the audio device release. The parent exits through iced before it matters; the child opens the device after its own start-up, which takes longer than the parent's exit. If a device is still busy the child's existing device fallback applies.
- Alternative for the bar: keep a tier switch. Rejected: two looks, two sets of tests, and the owner asked to drop glass.
- Alternative for the restart: a `Command` that relaunches from `nemo-ui`. Rejected: `nemo-ui` must not spawn processes.
- Alternative for the blur: a custom compositor call. Rejected: `window-vibrancy` is approved and has safe functions.
