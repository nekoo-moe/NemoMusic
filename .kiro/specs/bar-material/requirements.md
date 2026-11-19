## Goal

The floating player bar becomes a Material 3 tonal surface: `surface-container-high` at a good opacity, with no outline, no rim and no glint, so no thin bright line runs round the capsule. Depth comes from tone and a soft shadow. The Window glass card on the Appearance page shows a `Restart now` pill when the switch needs a restart, and the pill restarts the app on click. On Windows, Window glass gets a real blur behind the window (Mica or acrylic) through the crate `window-vibrancy`.

Owner request, 2026-11-19, with screenshots: "The music bar still has that outline line. Fix this line problem with a better design. I think the music bar should be a Material design surface with good opacity. Drop the glass style." "When Window glass is switched on, instead of only the text `Takes effect after you restart NemoMusic`, show a pill that pops up, like `Restart now`." "Add crates whenever they make the app better." Owner approval: the request itself (common brief, 2026-11-19). `window-vibrancy` is approved in `tech.md`.

## User Stories

- As a listener, I want the player bar to look like one clean Material surface, so that no hard line cuts round it.
- As a listener with a see-through window, I want the bar to stay readable over any cover and any desktop.
- As a listener who turns Window glass on, I want a `Restart now` button next to the switch, so that I do not have to quit and start the app by hand.
- As a listener who did not ask for a restart, I want the app never to restart on its own, so that my music is never cut.
- As a Windows listener, I want the see-through window to blur what is behind it, so that glass looks frosted as on other apps.

## Acceptance Criteria

### Player bar (PR 1)
1. THE SYSTEM SHALL draw the capsule as a flat `surface-container-high` fill at alpha 0.90 in dark and AMOLED and 0.92 in light, with corners `shape.xl`, the level 3 shadow (`0 4px 16px` at 0.32 alpha) and nothing else: no border, no rim, no glint, no inner highlight and no gradient.
2. WHEN the capsule is rendered on the CPU renderer at 1x or 2x scale, THE SYSTEM SHALL show no 1 px line at any edge: the pixel rows and columns across each edge change from backdrop to fill in one step.
3. WHILE Reduce transparency is on, or the window is see-through (Window glass clear), THE SYSTEM SHALL draw the capsule fully opaque.
4. THE SYSTEM SHALL keep `on-surface` and `on-surface-variant` at 4.5:1 or more over the capsule over every fixture backdrop (all covers, dark, AMOLED, light, song colours on and off), with and without the shadow under the fill.
5. THE SYSTEM SHALL draw the same capsule for `GlassTier::Low` and `GlassTier::Off`. The enum and the `glass_tier` config key stay, so old config files load; no new setting is added.
6. THE SYSTEM SHALL show the Appearance card `Player bar` with one short honest helper line and no tier chips, and the live preview SHALL draw the bar on the same surface.
7. THE SYSTEM SHALL keep every control and behaviour of the bar.

### Restart now (PR 2)
8. WHEN the Window glass switch is on, Reduce transparency is off and the running window was created solid (`WindowGlass::needs_restart`), THE SYSTEM SHALL show a pill button `Restart now` (accent tonal, `shape.full`, 40 px high) beside the switch and a short helper line instead of the sentence `Takes effect after you restart NemoMusic`.
9. WHEN the pill appears, THE SYSTEM SHALL rise it by 8 px and fade it in over `motion.base`; under Reduce motion it SHALL appear at once.
10. THE SYSTEM SHALL NOT show the pill when nothing changed, when the switch is off, or while Reduce transparency overrides the switch.
11. WHEN the user presses `Restart now`, THE SYSTEM SHALL send exactly one restart request through the new `UiOptions::on_restart` callback. `nemo-ui` SHALL NOT spawn a process.
12. WHEN `nemo-app` gets the request, THE SYSTEM SHALL make sure settings are saved, start `std::env::current_exe()` with the same arguments (`std::env::args_os().skip(1)`), then exit through `iced::exit()` so the audio device is released.
13. IF the new process cannot be started, THEN THE SYSTEM SHALL show one plain sentence and keep running.
14. THE SYSTEM SHALL NEVER restart without the click.

### Windows blur (PR 3)
15. WHEN Window glass is on at start, Reduce transparency is off and the window was created transparent on Windows, THE SYSTEM SHALL apply Mica (Windows 11 build 22000 or newer) or acrylic (Windows 10 1809 or newer) behind the window, tinted from the theme base and the opacity setting.
16. IF applying the effect fails, THEN THE SYSTEM SHALL log one `tracing::warn!` with no handle values and keep the plain transparent window.
17. WHEN Reduce transparency is on or Window glass is off, THE SYSTEM SHALL apply no effect.
18. THE SYSTEM SHALL keep the code `cfg(windows)` only, add the crate `window-vibrancy` `=0.8.1` to `nemo-app` only, and use no `unsafe`. The Linux build SHALL NOT change.
19. THE SYSTEM SHALL say in the helper of the Window glass card: on Windows the blur is built in, on KDE it needs the compositor Blur effect, on other desktops the glass is clear without blur.

## Out of Scope

- Skeleton loading, the splash, the setup flow, the folder picker, lyrics and audio.
- Blur drawn by NemoMusic on Linux or macOS. The compositor does it.
- A setting to choose the bar material. The bar has one look.
- Removing the `glass_tier` config key.
- Restarting the app in any case other than the click.

## Open Questions

Decided by the author of this spec (owner, 2026-11-19: decide open points and finish):
1. The Low and Off chips are dropped, not replaced by a `Solid` toggle: Reduce transparency already makes the bar opaque, and a second switch would say the same thing twice.
2. The bar draws opaque while the window is see-through, because `design.md` section 6 keeps the bar solid there.
3. Windows 10 older than 1809 gets the plain transparent window, with no warning to the listener.
4. The Windows effect is not tested here: there is no Windows machine in this environment. CI compiles it.
