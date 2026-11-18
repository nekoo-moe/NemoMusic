# Startup flow: requirements

Owner approved 2026-11-18 by request: "A loading screen when the app opens." and "A first-run screen that starts a flow to set the app up: customise the look, sign in to SoundCloud, YouTube Music, and so on."

## Goal

NemoMusic opens a window with a calm splash at once, and shows the real start-up steps while the services open behind it. On the first run, a short full-window setup flow lets the user choose the look, sign in to YouTube Music, turn on SoundCloud's public catalog, add music folders, set ad blocking and SponsorBlock, and read and accept the notice. Every step can be skipped, and the flow can be run again from Settings.

## User Stories

- As a listener, I want to see the app the moment I start it, so that I know it is starting and not frozen.
- As a new user, I want one guided pass over the choices that matter, so that I do not have to hunt through Settings.
- As an existing user, I want nothing forced on me after an update, so that I can keep listening.
- As a privacy-minded user, I want the notice unticked by default and every choice reversible, so that I am never nudged into consent.

## Acceptance Criteria

1. WHEN the app starts, THE SYSTEM SHALL open the window and draw the splash (mark, wordmark, progress line, step name) before the services are built, on the neutral baseline of the saved base mode and never on a song colour.
2. WHEN a start-up step finishes (settings, library and offline cache, credential store, player, first screen), THE SYSTEM SHALL move the progress line by one step and name the running step in `label-sm`. THE SYSTEM SHALL NOT show a percentage or a time estimate.
3. THE SYSTEM SHALL keep the splash on screen for at least 500 ms, even when every step is done sooner.
4. WHEN the services are not ready 6 s after the first frame, THE SYSTEM SHALL say so under the progress line and keep waiting (the app has nothing to show without its services). WHEN they are ready, THE SYSTEM SHALL fade the splash out at once.
5. WHEN the splash ends, THE SYSTEM SHALL fade it out over `motion.base`; WHILE reduced motion is on, THE SYSTEM SHALL remove it instantly.
6. IF a service cannot start, THEN THE SYSTEM SHALL keep the splash, show a plain sentence and a Close button.
7. THE SYSTEM SHALL keep audio, `--signin` and every other command-line path unchanged, and the window title and taskbar behaviour as they are.
8. WHEN the app starts and `setup_completed` is false and `tos_acknowledged` is false, THE SYSTEM SHALL show the setup flow full-window. WHEN `tos_acknowledged` is true and `setup_completed` is missing from the stored settings, THE SYSTEM SHALL treat the setup as completed and show no flow.
9. THE SYSTEM SHALL show the steps Welcome, Look, Privacy and risk, Sources, Playback, Done (the notice comes before the sources because signing in needs it), with step dots, Back, Next and a Skip setup button that is always available. Enter SHALL be Next; Esc SHALL change nothing.
10. WHEN the user changes a Look choice, THE SYSTEM SHALL apply it live.
11. WHEN the user presses `Sign in with Google` on the Sources step, THE SYSTEM SHALL start the existing sign-in and show the account name when it returns; `Use without an account` SHALL move on. THE SYSTEM SHALL NOT show tokens, cookies or account ids.
12. THE SYSTEM SHALL show SoundCloud as an On/Off switch for the public catalog and the text `Public tracks only. No sign-in needed.` While the SoundCloud catalog is not part of the app, THE SYSTEM SHALL show the switch Off, unusable, with a sentence that says why.
13. THE SYSTEM SHALL show the ad blocking and SponsorBlock switches with the wording of `design.md` section 12 and their stored values as `On` or `Off`.
14. THE SYSTEM SHALL show the notice of the first-run sheet with its checkbox unticked, and SHALL keep Continue disabled until it is ticked. Not now SHALL work as in the first-run sheet.
15. WHEN a network or sign-in call fails, THE SYSTEM SHALL show a plain sentence and a way forward, and SHALL NOT block Skip setup.
16. WHEN the flow ends by Start listening or Skip setup, THE SYSTEM SHALL store `setup_completed` as true. WHEN the user presses `Run setup again` in Settings, THE SYSTEM SHALL show the flow again without resetting sign-in, settings or data.
17. THE SYSTEM SHALL store `setup_completed` additively: a settings file without it loads.
18. THE SYSTEM SHALL NOT send telemetry, and SHALL NOT offer Last.fm, Discord, Cast or AI in the flow.

## Out of Scope

- SoundCloud sign-in. It does not exist: `nemo-soundcloud` reads public tracks only. A sign-in needs its own spec and an owner decision on the login method (`coc.md`, stop and ask).
- The song-colour splash (the splash uses the neutral baseline).
- A native folder picker (none is wired in `nemo-app`; see Open Questions).
- Optional integrations (Last.fm, Discord, Cast, AI): "More options in Settings" only.

## Open Questions

- Folder picker: `UiOptions.on_add_music_folder` is `None` in the app, so the Local music card shows `Add a folder` only when a picker exists, and otherwise points to Settings. A picker needs a dialog crate (`Cargo.toml` change, owner decision).
- Song-colour switches in the Look step wait for the `song-colour-theme` setting on `main`.
- The 6 s ceiling cannot "continue anyway" without a backend. Owner: is the sentence under the progress line enough?
