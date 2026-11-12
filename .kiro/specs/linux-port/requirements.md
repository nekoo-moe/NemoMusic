## Goal

NemoMusic builds and runs on Linux (x86_64, glibc, PipeWire or PulseAudio or ALSA) with the same features as on Windows: YouTube Music playback, Google sign-in, lyrics, SponsorBlock and the signal-path view. Windows behavior does not change. Linux output goes through the system sound server first; the Linux bit-perfect path (direct ALSA `hw:` device) is a later step.

## User Stories

- As a Linux listener, I want to install and run NemoMusic, so that I can use it on my main system.
- As a Linux listener, I want to sign in with Google, so that my library and likes show up.
- As a Linux listener, I want my session stored in the system keyring, so that no cookie sits in a plain file.
- As a maintainer, I want one codebase for both systems, so that fixes reach both.

## Acceptance Criteria

1. WHEN `cargo build --workspace` runs on Linux with the documented system packages, THE SYSTEM SHALL compile without errors.
2. WHEN `cargo build --workspace` runs on Windows, THE SYSTEM SHALL still compile and every existing test SHALL still pass.
3. WHEN `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` run on Linux, THE SYSTEM SHALL pass them. Tests that need Windows are gated with `#[cfg(windows)]` and have a Linux twin.
4. WHEN the app starts on Linux, THE SYSTEM SHALL list output devices through `cpal` and play a track through the default device.
5. WHEN the app plays on Linux in the default mode, THE SYSTEM SHALL label the output path as shared mode, never as bit-perfect.
6. WHEN the user signs in on Linux, THE SYSTEM SHALL open Google's page in a webview window, read the session cookies and store them in the Secret Service keyring.
7. IF no Secret Service is running, THEN THE SYSTEM SHALL say so and SHALL NOT write the session to a file.
8. WHEN the user signs out on Linux, THE SYSTEM SHALL delete the keyring entries, the webview profile and the cached account data.
9. WHEN `yt-dlp` is needed on Linux, THE SYSTEM SHALL find it on `PATH` or in `$XDG_DATA_HOME/NemoMusic/bin`, and SHALL NOT use the `.exe` or winget paths.
10. WHEN data paths are resolved on Linux, THE SYSTEM SHALL use `$XDG_DATA_HOME/NemoMusic` or `~/.local/share/NemoMusic`.
11. WHEN the README is read, THE SYSTEM SHALL list the Linux system packages and any new network host (none expected).

## Out of Scope

- macOS.
- Wayland-only or X11-only polish beyond what `iced` gives.
- Direct ALSA `hw:` bit-perfect output (separate spec).
- Flatpak, AppImage, deb or rpm packaging.
- ARM Linux.

## Open Questions

These need the owner (workflow.md section 8, coc.md):

1. Credentials: approve the Secret Service backend for `keyring-core` (candidate: `dbus-secret-service-keyring-store`, version and license unverified)?
2. Sign-in window: `wry` and `tao` use `webkit2gtk` and `gtk3` on Linux. Google may block embedded WebKitGTK sign-in. Approve it, with cookie paste as fallback?
3. System packages needed for a build (found missing on this machine): `libasound2-dev`, `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, `libxdo-dev`, `libssl-dev`, `pkg-config`. Accept them as documented build requirements?
4. TLS: `reqwest` and `ytmapi-rs` use `native-tls`, which means OpenSSL on Linux. Accept it?
5. Is a CI job on Linux wanted?
6. Do `tech.md`, `structure.md` and `product.md` ("Windows only" lines) get updated by me in the same change?
