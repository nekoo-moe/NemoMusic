# Startup flow: design

## Overview

`nemo-ui` gets a `Shell` program (`gui/shell.rs`) in place of `App` as the program iced runs. It shows the splash at the first frame, runs the caller's start function on its own thread, and builds `App` under the splash when the start function returns. `nemo-app` moves the service build (`backend::build`) into that start function and reports each real step. The setup flow (tasks 3 and 4) is a full-window layer of `App` driven by a pure state machine in `nemo-ui` (`setup.rs`).

## Affected Crates

- `nemo-ui`: `splash.rs` (pure), `gui/shell.rs`, `gui/views/splash.rs`, later `setup.rs` and `gui/views/setup/`.
- `nemo-app`: `main.rs`, `backend.rs` (`Startup`), `ui_bridge.rs`, later `config.rs`.
- No change to `nemo-audio`, `nemo-core` or any `Cargo.toml`.

## Data Flow

The main thread builds the window and the `Shell`. The start thread (`startup`) calls `backend::build`, which reports `StartStep`s through an unbounded `futures` channel; the shell listens through `Task::run` and moves the progress line. The final `Started(Ok(Backend))` message makes the shell call `App::new` and start the first jobs. The play hub is stored in a shared slot that `main` stops after the window closes. The output watcher starts with the player. The audio thread is untouched: the player is spawned in `build` exactly as before, only on another thread and after the first frame. Settings are read before the window (they are needed for the base mode and reduced motion).

## Interfaces

```rust
pub enum StartStep { Settings, Library, CredentialStore, Player, FirstScreen }
pub struct Splash; // new, finish, fail, tick, phase, opacity, fraction, status, is_slow
pub type StepReporter<'a> = &'a (dyn Fn(StartStep) + Send + Sync);
pub type StartFn = Box<dyn for<'a> FnOnce(StepReporter<'a>) -> Result<Backend, String> + Send>;
pub fn run_with_startup(start: StartFn, options: UiOptions) -> iced::Result;
```

Setup (later tasks): `SetupStep`, `SetupState` with `next`, `back`, `skip`, `tos_gate`, and `first_run(settings)`.

## Error Handling

A failed service build is a plain sentence on the splash with a Close button; nothing is logged with secrets. A missing start function payload or a thread that cannot start also ends in that sentence.

## Testing Strategy

No audio code changes (no bit-exact run needed). `splash.rs` tests drive a fake clock: floor of 500 ms, fade over `motion.base`, reduced motion, 6 s note, failure. `gui/shell.rs` tests prove that `boot` returns while the start function is still blocked and that the first view builds with no backend. Snapshot PNGs of the splash in each base mode. Setup tests: first-run versus existing user, navigation and Skip, notice gating, config migration.

## Risks and Alternatives

- Moving `build` off the main thread: the services use the tokio handle and plain threads only, and none needs the main thread. The Windows credential store call now runs on the `startup` thread; it is thread-agnostic. Not checked on Windows.
- Alternative: a placeholder backend swapped in later. Rejected: `Backend` holds the player and the account service, and swapping them under a running UI is a larger change.
- The 6 s ceiling cannot continue without services (see requirements, Open Questions).
- The splash fade draws the app underneath; the first frames of `App` are built at full size under it.
