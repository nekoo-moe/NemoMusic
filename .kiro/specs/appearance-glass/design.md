## Overview

Two parts. First, a pure model in `nemo-ui` (`src/window_glass.rs`): the two settings, the opacity clamp, the state of the running window (`Off`, `Clear`, `Overridden`, `NeedsRestart`), and the alpha of the backdrop and of the panes. Second, the drawing: the window is created with iced's `window::Settings::transparent` when `window_glass` is on at start, the application `style` returns a transparent window colour while the glass is clear, and the backdrop and pane fills multiply their alpha by the model's numbers. The Appearance page is rebuilt on top of the model.

Which surfaces turn see-through, and why:

| Surface | Window glass on | Why |
|---|---|---|
| Window backdrop (flat colour, colour fields, vignette) | alpha 0.40 to 0.95, the chosen opacity | It carries no text. This is what shows the desktop. |
| Gaps and edges between the panes | same (they show the backdrop) | No text. |
| Left and centre panes, and the right pane without a cover | alpha 0.80 or more | Text sits on them. At 0.80, `on-surface` and `on-surface-variant` keep 4.5:1 over any desktop (tests below). |
| Right pane with a cover (cover canvas gradient) | solid | It is opaque today and holds the lyrics. |
| Player bar, menus, popovers, sheets, toasts, mini player | solid | They are floating controls with their own opaque surface (`design.md` sections 6 and 7). |
| Full-screen Now Playing and lyrics | solid, scrim unchanged | Lyrics sit straight on the backdrop with no pane. |

Colour fields are tint on the glass: they are drawn at their own opacity times the window opacity over the flat colour, so a field's core is denser than the flat colour. Blending is in sRGB, as the compositor does it.

Reduce transparency forces every surface solid and the setting shows as overridden. A window created solid cannot become transparent, so the setting applies at the next start; turning it off applies at once.

## Affected Crates

- `nemo-ui`: model, settings, views, window creation, snapshots.
- `nemo-app`: config fields and the bridge to `UiSettings`.

Not touched: `nemo-audio`, `nemo-core` (no public API change), `Cargo.toml`.

## Data Flow

UI thread only. `UiSettings` (`window_glass`, `window_glass_opacity`) goes to the app through the existing `on_settings` callback and is saved in `settings.json` by `nemo-app`. At start `run_with_startup` reads `options.settings.window_glass` for the window and `App::new` stores it as `window_transparent`. Views ask `App::window_glass()` for the model and read `scheme::pane_alpha()`, a process-wide value set where the neutrals are set (start and every settings change). Nothing runs on the audio thread and no frame is added.

## Interfaces

```rust
// nemo_ui::window_glass
pub const OPACITY_MIN: u8 = 40;
pub const OPACITY_MAX: u8 = 95;
pub const OPACITY_DEFAULT: u8 = 80;
pub const PANE_FLOOR: f32 = 0.80;
pub const fn clamp_opacity(percent: u8) -> u8;
pub enum GlassState { Off, Clear, Overridden, NeedsRestart }
pub struct WindowGlass { pub enabled: bool, pub opacity: u8, pub reduce_transparency: bool, pub created_transparent: bool }
impl WindowGlass {
    pub fn state(self) -> GlassState;
    pub fn backdrop_alpha(self, full_screen: bool) -> f32;
    pub fn pane_alpha(self, base: f32) -> f32;
    pub fn status(self) -> String;
}
// nemo_ui::settings
UiSettings { window_glass: bool, window_glass_opacity: u8, .. }
SettingChange::WindowGlass(bool), SettingChange::WindowGlassOpacity(u8)
UiSettings::window_glass(&self, created_transparent: bool) -> WindowGlass
// nemo_ui::scheme
pub fn pane_alpha() -> f32;
pub fn set_pane_alpha(alpha: f32);
// nemo_app::config::Settings
pub window_glass: bool, pub window_glass_opacity: u8
```

## Error Handling

A config value outside 40 to 95 is clamped when it loads and when it is applied. A missing field takes the default. If the compositor gives no alpha channel, the window stays opaque and the page still says "see-through"; the helper text names this as a compositor matter. No error dialog.

## Testing Strategy

No audio is touched, so no bit-exact test. Unit tests: defaults, config migration (old file, round trip, out-of-range value), the opacity clamp, `needs restart` state logic, Reduce transparency overriding, the alpha floors, and the text contrast of both inks over fixture desktops (white, black, saturated, noisy) in dark, AMOLED and light. Snapshots through the tiny-skia renderer composite the transparent window over a mock wallpaper at opacity 40, 60, 80 and 95, dark and light, at 1280 x 800 and 960 x 600, and the whole Appearance page in dark, AMOLED and light at 960 x 600 and 200% scale. Not verifiable here: a real window, the wgpu compositor alpha mode, Windows, real compositor blur.

## Risks and Alternatives

- A compositor without alpha support shows a black or solid window. Mitigation: off by default, helper text, Reduce transparency.
- iced has no group opacity, so the backdrop fades by multiplying each fill's alpha. Colour fields are therefore slightly denser than the flat colour. Alternative: render the backdrop to a texture and draw it with one opacity; more code, a texture per frame. Not chosen.
- Blur: only the compositor can blur what is behind a window. Alternative: the crate `window-vibrancy` for acrylic or mica on Windows. Needs the owner's approval for a new dependency (Open Question 1).
