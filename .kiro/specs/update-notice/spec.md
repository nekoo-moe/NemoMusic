# Update Notice Specification (update-notice)

## 1. Goal
Provide an opt-in, non-intrusive update notification when a new NemoMusic version is released on GitHub.

## 2. Privacy & Network Rules
- **Host**: `api.github.com`
- **Opt-in only**: `check_for_updates` is `false` by default. No network calls take place until the user explicitly turns it on in Settings (About) or during the first-run Setup flow.
- **Payload**: Pure GET request to `https://api.github.com/repos/nekoo-moe/NemoMusic/releases/latest`. Only sends `User-Agent: NemoMusic/<version>`. No identifiers, device specs, or telemetry.
- **Frequency**: At most once every 24 hours (`last_update_check_epoch`).
- **Silent failure**: If the request fails (offline, rate limited, server error), the error is discarded silently.
- **No auto-download**: The app only provides a link to the release on GitHub.

## 3. UI Presentation
- When a newer release is detected and not yet dismissed:
  - Displays a calm notice card at the top of the Home view.
  - "What's new" button: opens the release URL in the system browser.
  - "Dismiss" button: sets `dismissed_update_version` to the tag name and dismisses the card.
