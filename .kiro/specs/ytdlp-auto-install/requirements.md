Spec approved by the owner on 2026-11-15 (design delegated; decisions below).

Owner decisions (2026-11-15):
- (b) The app installs and updates yt-dlp by itself.
- (c) New egress hosts `github.com` and `release-assets.githubusercontent.com` are approved, for yt-dlp release downloads only.
- (d) Edits to `.kiro/steering/tech.md`, `README.md`, `.github/workflows/release.yml` and this spec are approved.

## Goal

YouTube playback works on a fresh Windows x64 or Linux x86_64 machine without installing yt-dlp by hand. When no yt-dlp is found, the app downloads the official release into its own data folder in the background, checks it against the published SHA-256 list, starts using it without a restart, and keeps that copy up to date. A yt-dlp the user installed is never changed.

## User Stories

- As a new listener, I want YouTube songs to play without a separate install step, so that the app works the first time.
- As a listener who installed yt-dlp myself, I want the app to leave my copy alone, so that I keep control of it.
- As a privacy-minded listener, I want to switch the download off, so that the app contacts GitHub only if I agree.

## Acceptance Criteria

1. WHEN the app starts with `yt_dlp_auto_install` on, no `yt_dlp_path` set and no yt-dlp found by the lookup, THE SYSTEM SHALL install the app-managed copy on a background worker thread, never on the UI or audio thread, and SHALL NOT delay startup.
2. WHEN the platform is Windows x64, THE SYSTEM SHALL install the onedir `yt-dlp_win.zip`; WHEN it is Linux x86_64, THE SYSTEM SHALL install the one-file `yt-dlp_linux` to `<data>/NemoMusic/bin/yt-dlp/yt-dlp` through a temp file and a rename, mode 0o755.
3. WHEN the platform is any other, THE SYSTEM SHALL install nothing and log one warning.
4. WHEN a downloaded asset does not match its line in `SHA2-256SUMS`, THE SYSTEM SHALL discard it and keep what was there.
5. WHEN an install succeeds, THE SYSTEM SHALL use the new copy for the next YouTube resolve without an app restart.
6. WHEN the app-managed copy is the one in use and no check finished in the last 24 hours, THE SYSTEM SHALL download `SHA2-256SUMS` at startup and reinstall only if the asset's hash differs from the recorded one (or none is recorded).
7. WHEN the yt-dlp in use is the user's (settings path, `PATH`, winget), THE SYSTEM SHALL NOT download, update or change it.
8. WHEN `yt_dlp_auto_install` is off, THE SYSTEM SHALL make no download.
9. IF a download or install fails, THEN THE SYSTEM SHALL log a warning and try again only at the next start.
10. WHEN downloading, THE SYSTEM SHALL use HTTPS only, at most 3 HTTPS redirects, one download at a time, and the existing size caps.
11. WHEN the README is read, THE SYSTEM SHALL list `github.com` and `release-assets.githubusercontent.com` with when they are contacted.

## Out of Scope

- A settings UI row for the switch (follow-up, see tasks.md).
- Checking the release's GPG signature (`SHA2-256SUMS.sig`).
- Updating a yt-dlp the user installed; installs on macOS, ARM or 32-bit systems.
- Bundling yt-dlp in the release archive.

## Open Questions

None. The owner decided the design on 2026-11-15.
