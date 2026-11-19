## Goal

The Appearance page of Settings is rebuilt in Material 3 Expressive: a live preview of the window at the top, then grouped cards with a title, a one-line helper and controls on one right edge. It lists every look setting, including the song colours and the gentle breathing. A new setting, Window glass, makes the app window itself see-through, so the desktop and other windows show through the gaps and edges while the panes stay readable. Blur behind the window is the desktop compositor's job; NemoMusic does not draw it.

Owner request, 2026-11-19: "Rebuild the whole Appearance setting, add the new options we made, such as the lively background. Also add a transparent glass background for me: frosted glass that you can see through to other apps, the coolest way." Owner approval: the request itself (common brief, 2026-11-19).

## User Stories

- As a listener, I want to see what a choice does before I leave the page, so that I do not have to hunt for the result.
- As a listener, I want every look setting in one tidy place with a one-line explanation each, so that I know what I am changing.
- As a listener who likes glass, I want the window to be see-through, so that my desktop and other apps show through the edges.
- As a listener with a busy desktop, I want text to stay readable whatever is behind the window, so that glass never costs legibility.
- As a listener sensitive to transparency or motion, I want one switch that makes everything solid or still, so that the window stays calm.

## Acceptance Criteria

### Appearance page
1. THE SYSTEM SHALL show at the top of Appearance a live preview of the window (sidebar, a pane, the player bar and the accent), drawn from tokens with no image assets, and SHALL redraw it at once when any choice on the page changes.
2. THE SYSTEM SHALL group the settings in cards, each with a title, a one-line helper and aligned controls: Background (Match colours to the playing song, Gentle breathing), Surface, Colour theme, Accent intensity, Corners, Now Playing style, Glass (player bar material), Window glass, Motion and accessibility (Reduce motion, Reduce transparency).
3. THE SYSTEM SHALL show every switch with the text `On` or `Off`.
4. WHEN Match colours to the playing song is off or Reduce motion is on (the setting or the OS preference), THE SYSTEM SHALL show Gentle breathing greyed out and ignore presses on it.
5. THE SYSTEM SHALL keep every existing setting, its change message, its default and its stored value. Old config files load unchanged.
6. THE SYSTEM SHALL align the controls on one right edge, give every target at least 32 px and a focus ring, and work at 960 x 600 and at 200% scale. The page scrolls inside its pane.
7. THE SYSTEM SHALL describe the Glass (player bar) row honestly and SHALL NOT say a feature "is not built yet".
8. THE SYSTEM SHALL draw the page in dark, AMOLED black and light.

### Window glass
9. THE SYSTEM SHALL add the setting `window_glass`, off by default, and `window_glass_opacity`, default 80 and clamped to 40 to 95 percent, both saved in the config. A config file without them loads with the defaults.
10. WHEN Window glass is on at start and Reduce transparency is off, THE SYSTEM SHALL create the window transparent and draw the window backdrop (the colour fields, the flat colour and the vignette) and the gaps between the panes at the chosen opacity, so other windows and the desktop show through.
11. WHILE Window glass is on, THE SYSTEM SHALL draw the panes with an alpha of at least 0.80, so that `on-surface` and `on-surface-variant` keep 4.5:1 over a white, black, saturated or noisy desktop.
12. THE SYSTEM SHALL keep the player bar, menus, sheets, toasts, the full-screen Now Playing and the mini player solid. The full-screen lyrics keep their scrim.
13. WHEN Reduce transparency is on, THE SYSTEM SHALL draw the window solid, and the Window glass switch SHALL show that it is overridden.
14. THE SYSTEM SHALL show next to the Window glass switch the note `Takes effect after you restart NemoMusic` and the state of the running window (see-through at N percent, solid, solid until restart, or solid because of Reduce transparency).
15. WHEN Window glass is turned on while the window was created solid, THE SYSTEM SHALL keep drawing it solid until the next start. Turning it off, or changing the opacity of a see-through window, applies at once.
16. THE SYSTEM SHALL say in the helper text that frosted blur needs a compositor that blurs transparent windows (for example KDE with the Blur effect and `Force blur` for this window), and that other desktops show clear glass without blur.
17. THE SYSTEM SHALL work with the wgpu renderer and the CPU renderer and SHALL NOT add a crate.

## Out of Scope

- Blur behind the window drawn by NemoMusic. On Windows, acrylic or mica blur would need the crate `window-vibrancy` (MIT or Apache-2.0), which `tech.md` does not list; it is not added.
- Changing the window at run time from solid to see-through (the window is created once).
- Changes to the drift, the breathing of the colours, the colour mesh and the player bar material (another spec, `living-backdrop` and `player-bar-redesign`).
- Audio, lyrics rendering and the startup flow.

## Open Questions

Decided by the author of this spec (owner, 2026-11-19: decide open points and finish):
1. Windows acrylic or mica blur: not added. It needs the crate `window-vibrancy` (MIT or Apache-2.0), which `tech.md` does not list; adding it is the owner's call. Windows is not verified here.
2. The full-screen Now Playing stays solid, because its lyrics sit straight on the backdrop with no pane.
3. The player bar, menus and sheets stay solid, as `design.md` section 6 requires for text-bearing floating surfaces.
