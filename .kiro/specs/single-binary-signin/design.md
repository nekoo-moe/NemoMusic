## Overview

`nemo-signin` becomes a library crate with `pub fn run(args) -> i32`; it keeps `cookies.rs`, the `--data-dir <folder>` argument, the exit codes 0/2/3/64 and the one-JSON-line stdout protocol, and it still depends on no `nemo-*` crate. `nemo-app` links it. The first thing `main` does is look for `--signin` as the first argument; if present it calls `nemo_signin::run` with the rest and exits with its code. The sign-in window therefore still lives in its own process (tech.md: tao/wry's event loop must never share a process with iced), but that process is the same executable. `nemo-ytm` starts the program it is given with leading arguments, and `nemo-app` gives it `current_exe()` and `--signin`.

## Affected Crates

- `nemo-signin`: `main.rs` becomes `lib.rs`; no binary target. A missing display (tao panics in GTK init) is caught and reported as exit 3.
- `nemo-ytm`: `AccountConfig` gains `helper_args: Vec<OsString>`, put before `--data-dir`. `HELPER_NAME` and `default_helper_path` are removed (public API change inside the workspace; the only caller is `nemo-app`).
- `nemo-app`: depends on `nemo-signin`; `main` dispatches `--signin`; `backend.rs` passes `current_exe()` and `--signin`.
- Docs and CI: `README.md`, `structure.md` (layout line, crate row, `nemo-app -> nemo-signin` edge), the tech.md "Sign-in window" row, `.github/workflows/release.yml`.

## Data Flow

Unchanged except for the program that is started: UI -> `AccountService::sign_in` -> `nemo-ytm` (tokio task) spawns `<app exe> --signin --data-dir <webview>` -> child process: `main` -> `nemo_signin::run` -> tao event loop on the child's main thread -> one JSON line on stdout -> parent reads at most 64 KiB -> `Vault::save`. No audio buffers are involved.

## Interfaces

```rust
// nemo-signin/src/lib.rs
pub const SIGNIN_ARG: &str = "--signin";
pub const EXIT_CANCELLED: i32 = 2;
pub const EXIT_UNAVAILABLE: i32 = 3;
pub const EXIT_USAGE: i32 = 64;
pub fn run(args: impl IntoIterator<Item = OsString>) -> i32; // never returns once the window is open

// nemo-ytm/src/account.rs
pub struct AccountConfig {
    pub helper: PathBuf,
    pub helper_args: Vec<OsString>, // new: "--signin" in the app
    pub webview_dir: PathBuf,
    pub cache_file: PathBuf,
}
```

## Error Handling

- Bad arguments: exit 64. `nemo-ytm` maps any exit code other than 0, 2 and 3 to "the sign-in window failed".
- No display or no webview runtime: exit 3, mapped to the existing `WEBVIEW_MISSING` text. The panic hook is silenced while the event loop is built, so nothing reaches stderr.
- `current_exe()` fails: a warn log, and the window cannot start ("cannot be started"); cookie paste still works.

## Testing Strategy

- `nemo-signin`: argument parsing and `run` returning 64 for bad arguments (unit tests).
- `nemo-ytm`: the existing account tests (fake helper script, empty `helper_args`), plus `the_app_is_started_in_sign_in_mode`: a fake executable that answers only when its first two arguments are `--signin --data-dir`.
- Manual: the debug `nemo-app --signin` with bad arguments exits 64; `--signin --data-dir <dir>` with no `DISPLAY` and no `WAYLAND_DISPLAY` exits 3 with no output.
- No audio code changes; no bit-exact test needed.

## Risks and Alternatives

- Linux load-time libraries: `nemo-app` now links GTK 3, WebKitGTK 4.1 and libsoup 3 at load time, so their runtime packages are needed to start the app at all, not only to sign in. The release notes already list "the GTK 3, WebKitGTK 4.1, libsoup 3, ALSA and OpenSSL runtime libraries", and the README Linux notes now say so too. Checked with `ldd` on the debug build: `libwebkit2gtk-4.1.so.0`, `libgtk-3.so.0` and `libsoup-3.0.so.0` are now direct load-time dependencies of `nemo-app`. On Windows the WebView2 runtime should still be looked up only when the window opens (exit 3 otherwise); not verified on Windows.
- Binary size: not measured with a release build (shared build machine, limited disk). The debug `nemo-app` with the window inside is 157 MB (no debug info). The release archive loses one file; the app grows by about the size of the old `nemo-signin` minus the shared standard library and `serde_json`.
- lyrics-plus: its `nemo-signin --site spotify` becomes `nemo-app --signin --site spotify`. That spec is not edited here.
- Alternative rejected: running the window in the app process on its own thread. tao needs the main thread on several platforms and would fight iced's event loop (tech.md).
