## Overview

Most crates are already portable. The Windows-only parts are few and small: the credential store, the sign-in helper's webview setup, the `yt-dlp` search paths, the iced window backend features, Windows wording in UI copy, and tests that use `.cmd` scripts. Each gets a Linux branch behind `cfg`. Windows code paths stay byte-for-byte the same in behavior.

Audio needs no code change: `cpal` 0.18.2 picks ALSA on Linux, which reaches PipeWire or PulseAudio through their ALSA plugins. The signal-path view already reports shared mode for `cpal`, so it never claims bit-perfect on Linux (criterion 5).

## Affected Crates

- `nemo-ui`: enable iced `x11` and `wayland` features on Linux; platform-correct storage wording in `copy.rs`.
- `nemo-ytm`: `SecretServiceStore` in `credentials.rs`; `yt-dlp` lookup without winget paths on Linux; helper name without `.exe`; helper error text; Linux twins of the `.cmd` tests.
- `nemo-signin`: enable wry `os-webview` on Linux and attach the webview to tao's GTK box.
- `nemo-app`: pick the platform store and helper name.
- `nemo-audio`, `nemo-core`, `nemo-library`, `nemo-lyrics`: no change.

## Data Flow

No change. Audio still flows decoder thread -> `rtrb` ring -> `cpal` callback. The sign-in flow is still `nemo-app` -> `nemo-ytm` spawns `nemo-signin` -> one JSON line on stdout -> `Vault::save` -> platform `SecretStore`. On Linux the store talks to the Secret Service over D-Bus (`zbus`, its own executor thread), called from the same places the Windows store is called.

## Interfaces

```rust
// nemo-ytm/src/credentials.rs
#[cfg(target_os = "linux")]
pub struct SecretServiceStore { /* Arc<zbus_secret_service_keyring_store::Store> */ }
#[cfg(target_os = "linux")]
impl SecretServiceStore { pub fn new() -> Result<Self, StoreError>; }
#[cfg(target_os = "linux")]
impl SecretStore for SecretServiceStore { /* set, get, delete */ }

/// The store for this platform.
#[cfg(windows)] pub type PlatformStore = WindowsStore;
#[cfg(target_os = "linux")] pub type PlatformStore = SecretServiceStore;

// nemo-ytm/src/account.rs
pub const HELPER_NAME: &str; // "nemo-signin.exe" on Windows, "nemo-signin" elsewhere
```

`locate_yt_dlp` keeps its signature. Its base folder is `%LOCALAPPDATA%` on Windows and `$XDG_DATA_HOME` (or `~/.local/share`) on Linux; winget folders are searched only on Windows.

New dependency (Linux only): `zbus-secret-service-keyring-store` =1.0.1, MIT OR Apache-2.0, released 2026-08-15, feature `crypto-rust` (pure Rust, no `libdbus`, no OpenSSL). Approved by the owner on 2026-11-12.

## Error Handling

- No Secret Service on D-Bus: `SecretServiceStore::new` fails, `nemo-app` logs a warning and uses `NoAccount`, same as a missing Credential Manager. Nothing is written to a file (criterion 7).
- Helper exit code 3 on Linux maps to "the WebKitGTK webview is not available" instead of the WebView2 text.
- `yt-dlp` missing keeps the existing `StreamError::ResolverMissing`.

## Testing Strategy

- All existing pure tests run on Linux unchanged.
- The three `#[cfg(windows)]` sign-in tests get `#[cfg(unix)]` twins that use `/bin/sh` scripts with mode 0755.
- `locate_prefers_path_then_winget_then_app_bin` is split: the winget order runs on Windows, a Linux test checks onedir, `PATH`, then app `bin`.
- An `#[ignore]` round-trip test for the real Secret Service, like the Windows one.
- Audio is not touched, so the bit-exact test only has to keep passing: `cargo test -p nemo-audio --test bit_exact`.
- Manual: start the app, list devices, play a track, sign in through the helper.

## Risks and Alternatives

- Google may refuse sign-in inside WebKitGTK. Cookie paste stays the fallback.
- WebKitGTK and GTK on Wayland vs X11 may behave differently; tested on the owner's Linux Mint session only.
- ALSA through PipeWire may resample. It is labelled shared mode, so the badge stays honest. A direct `hw:` exclusive path is a later spec.
- Alternative store `dbus-secret-service-keyring-store`: needs `libdbus` C library. Rejected for the pure-Rust `zbus` one.
- Alternative store `linux-keyutils-keyring-store`: kernel keyring is lost on reboot. Rejected.
