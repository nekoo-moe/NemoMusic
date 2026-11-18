# Startup flow: tasks

Owner approved the request 2026-11-18.

## Tasks

- [x] 1. Splash (`feat/nemo-ui-startup-splash`). Files: `nemo-ui` `splash.rs`, `gui/shell.rs`, `gui/views/splash.rs`, `gui/mod.rs`; `nemo-app` `main.rs`, `backend.rs`, `ui_bridge.rs`; snapshots `startup.rs`. Does: window and splash at once, services built on the `startup` thread with real steps, 500 ms floor, fade over `motion.base`, failure sentence. _Requirements: 1, 2, 3, 4, 5, 6, 7_
- [x] 2. Setup state and setting (`feat/nemo-ui-startup-setup-state`, stacked on 1). Files: `settings.rs`, `state.rs`, `setup.rs`, `nemo-app` `config.rs`, `ui_bridge.rs`. Does: `setup_completed`, first-run versus existing user, wizard state machine (steps, Back, Next, Skip, notice gating). _Requirements: 8, 9, 14, 16, 17_
- [x] 3. Setup screens (`feat/nemo-ui-startup-setup-screens`, stacked on 2). Files: `gui/views/setup*`, `gui/app/*`, `copy.rs`, Settings Info. Does: the six steps, live look, sources, playback, notice, Done, `Run setup again`; snapshots of every step. _Requirements: 9 to 16, 18_
- [ ] 4. TODO (needs `song-colour-theme` on main): the two song-colour switches in the Look step.
- [ ] 5. TODO (needs the `soundcloud` spec tasks 7 to 20 and an owner-approved risk text): make the SoundCloud switch in the Sources step real. Today it is shown Off and cannot be turned on, because `Backend.soundcloud` is `None`, `opt_in_gate::OPT_INS` is empty and no Settings row exists. SoundCloud sign-in does not exist and needs its own spec and an owner decision on the login method (`coc.md`).
- [ ] 6. TODO (needs a folder picker, which needs a dialog crate and an owner decision): `UiOptions.on_add_music_folder` is `None` in `nemo-app`, so the Local music card shows no `Add a folder` button.
- [ ] 7. TODO (when the Apple Music Now Playing layout ships): offer it in the Look step; today only Classic and Material 3 Expressive are offered, because Apple Music draws the same view as Material 3 Expressive.

## Definition of Done

See section 4 of `.kiro/steering/workflow.md`.
