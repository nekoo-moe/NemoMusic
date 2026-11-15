Spec approved by the owner on 2026-11-15 (design delegated; decisions below).

Owner decisions (2026-11-15):
- (a) Ship ONE program: the sign-in window moves into the main app.
- (d) Edits to `.kiro/steering/structure.md` and `tech.md`, `README.md`, `.github/workflows/release.yml` and this spec are approved.

## Goal

NemoMusic ships as one program. The Google sign-in window is built into `nemo-app` instead of a second `nemo-signin` executable that had to sit next to it. Sign-in looks and behaves the same: the window still runs in its own process, started from the same executable.

## User Stories

- As a listener, I want to download and run one program, so that sign-in cannot break because a helper file is missing or was left behind.
- As a maintainer, I want the release to build and pack one executable, so that packaging has fewer steps.

## Acceptance Criteria

1. WHEN `nemo-app` is started with `--signin` as its first argument, THE SYSTEM SHALL run the sign-in window with the remaining arguments and exit with its code, before data paths, logging, the tokio runtime or iced start.
2. WHEN the remaining arguments are not exactly `--data-dir <folder>`, THE SYSTEM SHALL exit with code 64.
3. WHEN the webview or the window cannot start (for example no display), THE SYSTEM SHALL exit with code 3 and print nothing.
4. WHEN sign-in succeeds or the user closes the window, THE SYSTEM SHALL keep the existing protocol: exit 0 with one JSON line `{"cookie":"..."}` on stdout, or exit 2 with nothing printed.
5. WHEN the user starts sign-in, THE SYSTEM SHALL start the running executable as `<exe> --signin --data-dir <webview folder>`.
6. WHEN the release workflow runs, THE SYSTEM SHALL build and package only `nemo-app`.
7. WHEN the README, `structure.md` and `tech.md` are read, THE SYSTEM SHALL describe `nemo-signin` as a library linked into `nemo-app`, run as `nemo-app --signin`.

## Out of Scope

- Any change to the sign-in page, the cookie filter or the credential store.
- The lyrics-plus `--site spotify` argument (that spec's task 22 now reads `nemo-app --signin --site spotify`).
- Installers, code signing.

## Open Questions

None. The owner decided the design on 2026-11-15.
