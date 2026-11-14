## Tasks

Cross-spec ids written `<spec>#?` name the other spec's task by the ADR 0004 Decision 4 row it delivers; the number is filled in once that spec's `tasks.md` is merged.

- [x] 1. Add `OptInInfo`, the shared opt-in data type
  - Crate and files: `nemo-core` (`src/opt_in.rs` new, `src/lib.rs`); touches nemo-core/src/opt_in.rs.
  - Does: Adds `OptInInfo { id, title, sends, hosts, risk, version, default_on }` with rustdoc that repeats the ADR 0004 opt-in rules (off by default except media controls, risk text names the real risk). No I/O.
  - _Requirements: none (groundwork)_
  - Depends on: none
  - Wave: 0
  - Checks: unit test that `default_on` is false for every `OptInInfo` built in tests except one marked media controls.
  - Steering: `structure.md`, add the `opt_in` module to the `nemo-core` entry.

- [ ] 2. Add the shared `OptInRow` settings component
  - Crate and files: `nemo-ui` (`src/gui/views/opt_in.rs` new, `src/copy.rs`).
  - Does: Draws title, switch, "What is sent", "To" and "Risk" from an `OptInInfo`. Turning the switch on opens a confirm panel with "Turn on" and "Cancel", neither pre-selected; turning off is one click with a "what stays" line and an optional "Delete now". Shows hosts in use and one status line after turn-on.
  - _Requirements: 30_
  - Depends on: integrations#1
  - Wave: 0
  - Checks: `iced_test` snapshots (off, panel open, on); keyboard order and visible focus; no animation under reduced motion; readable at 200% text scale; `design.md` checklist with a screenshot.

- [ ] 3. Add the opt-in gate and the egress-list check in `nemo-app`
  - Crate and files: `nemo-app` (`src/opt_in_gate.rs` new, `src/config.rs`).
  - Does: Stores `enabled` and `confirmed_version` per `OptInInfo.id` in the config file. Exposes one check, "build this service only when the switch is on and `confirmed_version == version`", that every opt-in service is built through. A test compares every `OptInInfo.hosts` entry with the README egress list.
  - _Requirements: none (groundwork)_
  - Depends on: integrations#1
  - Wave: 0
  - Checks: a counting fake host sees zero requests while a switch is off or unconfirmed; raising `version` makes the gate closed again; egress-list test passes.

- [x] 4. Add `PlayRule`, the shared play-count rule
  - Crate and files: `nemo-core` (`src/hooks.rs` new, `src/lib.rs`); touches nemo-core/src/hooks.rs.
  - Does: Adds `PlayRule::{MIN_TRACK, MAX_THRESHOLD, threshold, counts}`: a track over 30 s counts at half its length or 4 minutes, an unknown length at 4 minutes. Pure.
  - _Requirements: 14, 15_
  - Depends on: none
  - Wave: 0
  - Checks: table test for 29 s, 30 s, 31 s, 8 min and unknown duration.
  - Steering: `structure.md`, add the `hooks` module to the `nemo-core` entry.

- [ ] 5. Add `PlayTracker`, `PlayHookEvent`, `FinishedPlay` and `PlayObserver`
  - Crate and files: `nemo-core` (`src/hooks.rs`); touches nemo-core/src/hooks.rs.
  - Does: Turns `PlayerEvent`s and position samples into `TrackStarted`, `StateChanged`, `PositionJumped`, `DurationKnown` and `TrackEnded`. Heard time grows only while `Playing`, by `min(position delta, wall time)`; a forward jump is a seek, a backward jump resyncs; `TrackChanged` (also repeat one) closes the old play; `flush` closes the last one.
  - _Requirements: 14, 15_
  - Depends on: integrations#4
  - Wave: 0
  - Checks: table tests (pause, seek forward and back, repeat one, gapless handover, skip); `proptest` that heard time never exceeds wall time. listening-stats reuses these tests.

- [ ] 6. Add the `OsNotifier` contract
  - Crate and files: `nemo-core` (`src/notify.rs` new, `src/lib.rs`); touches nemo-core/src/notify.rs.
  - Does: Adds `NotificationGroup`, `OsNotification`, `NotificationAction`, `NotificationEvent`, `NotifyUnavailable`, `NotifierState`, `NotifyError`, `NotificationReceiver`, `OsNotifier` and `NoNotifier`. No I/O.
  - _Requirements: 31_
  - Depends on: none
  - Wave: 0
  - Checks: `NoNotifier` reports `Unavailable(NoService)` and `notify` returns an error.
  - Steering: `structure.md`, add the `notify` module to the `nemo-core` entry.

- [ ] 7. Add `IntegrationsService` and the `Backend` fields
  - Crate and files: `nemo-core` (`src/integrations.rs` new, `src/services.rs`, `src/lib.rs`); touches nemo-core/src/integrations.rs and nemo-core/src/services.rs.
  - Does: Adds `IntegrationsService`, `LastFmStatus`, `IntegrationError`, `NativeWindow`, `NoIntegrations`, and `Backend.integrations` and `Backend.notifier` defaulting to `NoIntegrations` and `NoNotifier`.
  - _Requirements: none (groundwork)_
  - Depends on: integrations#1, integrations#6
  - Wave: 0
  - Checks: existing `Backend` tests build with the defaults. Only one task at a time edits `services.rs` (listening-stats#1 and ai-features#2 also touch it).
  - Steering: `structure.md`, add the `integrations` module and `Backend.{integrations, notifier}` to the `nemo-core` entry.

- [ ] 8. Create the `nemo-integrations` crate skeleton
  - Crate and files: `nemo-integrations` (new: `Cargo.toml`, `src/lib.rs`, `src/service.rs`, `src/info.rs`); workspace `Cargo.toml` members.
  - Does: Adds the crate with `build(..)` returning an `Integrations` that implements `IntegrationsService` and a no-op `PlayObserver` fan-out. `info.rs` holds the `OptInInfo` values for media controls (default on), Last.fm and Discord.
  - _Requirements: 10, 22_
  - Depends on: integrations#5, integrations#7
  - Wave: 0
  - Checks: unit test that only the media-controls info is `default_on`, and the Last.fm and Discord texts match criteria 10 and 22.
  - Steering: `structure.md`, new crate `nemo-integrations` (depends on `nemo-core` only) and edge `nemo-app -> nemo-integrations`.

- [ ] 9. Add the play hub thread in `nemo-app`
  - Crate and files: `nemo-app` (`src/play_hub.rs` new, `src/main.rs`, `src/backend.rs`).
  - Does: One thread drains `PlayerControl::subscribe`, samples `position()` every 500 ms, feeds `PlayTracker` and hands each event to registered `PlayObserver`s with a non-blocking send; a full channel drops the event. Calls `flush` at exit. Nothing runs in the audio callback.
  - _Requirements: 5_
  - Depends on: integrations#5
  - Wave: 0
  - Checks: test with a fake `PlayerControl` and a fake observer: events in order, a slow observer never blocks the hub, `flush` emits the last `TrackEnded`. `nemo-audio` unchanged; bit-exact test keeps passing.
  - Steering: `structure.md`, add `nemo-app/src/play_hub.rs`.

- [ ] 10. Spike: OS media controls with `souvlaki`
  - Crate and files: scratch branch only; result into `.kiro/specs/integrations/design.md` (Risks).
  - Does: Proves that `souvlaki` =0.9.3 with `default-features = false` and `use_zbus` passes `cargo deny check`, builds on 1.97, shows title and cover in the Windows overlay from a `file://` PNG, and that iced 0.14 hands over the HWND (`window::run`). Pass: all four hold. Fail: `cargo deny` fails, so the owner picks `use_dbus` or own MPRIS on `zbus` (changes tasks 14 and 16); `file://` rejected, so Windows gets no cover (changes task 15); no HWND, so task 16 finds another hand-over.
  - _Requirements: none (groundwork)_
  - Depends on: integrations#9
  - Wave: 1
  - Checks: needs a Windows machine and a Linux desktop session; `playerctl metadata` output recorded.

- [ ] 11. Spike: Discord IPC crate
  - Crate and files: scratch branch only; result into `.kiro/specs/integrations/design.md` (Risks).
  - Does: Proves that `discord-rich-presence` =1.1.0 passes `cargo deny check` (second `uuid` 0.8), builds on 1.97, and sets and clears a listening activity on a running Discord client. Notes how often local IPC accepts updates. Pass: all hold. Fail: task 24 uses about 150 lines of own IPC over the named pipe or Unix socket instead.
  - _Requirements: none (groundwork)_
  - Depends on: none
  - Wave: 1
  - Checks: needs the owner's Discord application id (or a temporary test id) and a Discord client.

- [ ] 12. Spike: Windows toast from an unpackaged build
  - Crate and files: scratch branch only; result into `.kiro/specs/integrations/design.md` (Risks).
  - Does: Proves that `tauri-winrt-notification` =0.8.1 shows a toast for app id `NemoMusic.NemoMusic` on Windows 10 and 11 with only a Start Menu shortcut carrying that id (made by hand), that `on_activated` fires, and whether the crate escapes `Tom & Jerry <3`. Pass: toast shows verbatim and the click arrives. Fail on escaping: `windows.rs` escapes (task 27). Fail on display: the owner decides between packaging (MSIX) and no Windows toasts; tasks 27 and 31 change.
  - _Requirements: none (groundwork)_
  - Depends on: none
  - Wave: 1
  - Checks: needs Windows 10 and Windows 11 machines.

- [ ] 13. Add the integrations database
  - Crate and files: `nemo-integrations` (`src/store.rs` new, `src/lastfm/queue.rs` new); `nemo-app` (`src/paths.rs`, `paths.integrations`).
  - Does: Opens `integrations.sqlite3` (WAL, `user_version` migrations) with `scrobbles` (capped at 5,000 rows, oldest dropped with one warning) and `meta` for the three switches and the Last.fm user name. Media controls default on in `meta`.
  - _Requirements: 9, 16_
  - Depends on: integrations#8
  - Wave: 1
  - Checks: migrations from empty and twice; queue survives reopen; order kept; cap drops oldest; an unopenable file leaves media controls and Discord working.

- [ ] 14. Publish playback to the OS media session and handle media keys
  - Crate and files: `nemo-integrations` (`src/media.rs` new, `src/service.rs`); `nemo-app` (`src/backend.rs` wiring `build` and `Backend.integrations`, hub registration).
  - Does: The media worker owns `souvlaki::MediaControls`, sets title, artist, album and duration on `TrackStarted`, and `set_playback` with position on state and seek events. `map_media_event` maps play, pause, toggle, next, previous, stop, set position and relative seek to `PlayerCommand`s. A missing OS service logs one warning and playback goes on.
  - _Requirements: 1, 2, 3, 5, 6, 7, 8, 9_
  - Depends on: integrations#10, integrations#13
  - Wave: 2
  - Checks: pure tests for `map_media_event` (every event of criteria 2 and 3) and for the metadata builder (only title, artist, album, duration, cover URL; criterion 8); test that a failing controls constructor gives one warning and no error. Bit-exact test keeps passing.
  - Steering: `tech.md` row `souvlaki` =0.9.3 (MIT, `default-features = false`, `use_zbus` on Linux); Linux package list unchanged if the spike passed.

- [ ] 15. Give the OS media session the cover art
  - Crate and files: `nemo-integrations` (`src/media.rs`, `src/media/cover.rs` new).
  - Does: Asks `ImageSource::image` for 512 px, re-encodes to PNG with `image`, writes `cover-0.png` or `cover-1.png` alternately under `paths.integrations/media` and passes a `file://` URL. No remote cover URL reaches the OS.
  - _Requirements: 1, 8_
  - Depends on: integrations#14
  - Wave: 2
  - Checks: test that files alternate and the URL is `file://`; on Windows without `file://` support (spike result) no cover is set.

- [ ] 16. Hand the native window to the media controls
  - Crate and files: `nemo-ui` (`UiOptions::on_window_ready`); `nemo-app` (window id to `IntegrationsService::window_ready`); `nemo-integrations` (`src/media.rs`).
  - Does: Reports the HWND once iced has the window, so SMTC can be created on Windows; the worker retries creating the controls when the id arrives or the switch flips. Media keys keep working while the window is minimized or unfocused.
  - _Requirements: 4, 7_
  - Depends on: integrations#14
  - Wave: 2
  - Checks: unit test that the worker creates controls once after `window_ready`; manual check with the window minimized (task 33).

- [ ] 17. Report the remote position while parked or casting
  - Crate and files: `nemo-app` (`src/play_hub.rs`).
  - Does: When `RemotePlayback::remote_status().mode` is not `Local`, the hub samples the remote position and state instead of `position()`, so the OS session and counted plays follow the cast device.
  - _Requirements: 5_
  - Depends on: integrations#9, cast#? (row 41, `RemotePlayback`), cast#? (row 7, `Park`)
  - Wave: 2
  - Checks: test with a fake `RemotePlayback` in `Cast` mode: position follows the remote, heard time grows only while the remote plays.

- [ ] 18. Add the OS media controls row in Settings
  - Crate and files: `nemo-ui` (Settings section "Integrations", `src/copy.rs`); `nemo-app` (switch wiring).
  - Does: Adds the "Integrations" section with the media controls `OptInRow`, on by default and turned off in one click (no confirm panel, nothing leaves the computer).
  - _Requirements: 9, 30_
  - Depends on: integrations#2, integrations#14
  - Wave: 3
  - Checks: `iced_test` snapshot; keyboard reach and focus; switching off removes the OS session within 1 s; `design.md` checklist with a screenshot.

- [ ] 19. Add the signed Last.fm API client
  - Crate and files: `nemo-integrations` (`src/lastfm/api.rs` new).
  - Does: `sign` builds `api_sig` (MD5 over sorted `name+value` pairs without `format`, plus the secret, via `md-5`). Calls `track.updateNowPlaying` and `track.scrobble` over HTTPS only, reads JSON whatever the status, and takes the base URL as a constructor argument (plain HTTP only under `#[cfg(test)]`). Key, secret, session key and request bodies are `Secret` or never logged.
  - _Requirements: 13, 21, 29_
  - Depends on: integrations#8
  - Wave: 2
  - Checks: `sign` vector checked against an independent MD5 tool, with non-ASCII values and `artist[10]` sorting before `artist[2]`; local test server; a `tracing` capture holds no key, secret, session key or title.
  - Steering: `tech.md` row `md-5` =0.11.0; README egress line `ws.audioscrobbler.com` (only when Last.fm is on).

- [ ] 20. Connect Last.fm through the browser
  - Crate and files: `nemo-integrations` (`src/lastfm/auth.rs` new); `nemo-app` (browser callback that opens only `https://www.last.fm/` URLs with `open`).
  - Does: `lastfm_save_keys` stores `lastfm.api_key` and `lastfm.api_secret` in `SecretStore`; `lastfm_begin` gets a token and returns the authorization URL; `lastfm_finish` gets the session key into `SecretStore` (`lastfm.session_key`) and the public user name into `meta`. No password is asked for; no key ships with the app.
  - _Requirements: 11, 11a, 12, 21_
  - Depends on: integrations#19, account-actions#? (row 24, `SecretStore` and `nemo-secrets`)
  - Wave: 2
  - Checks: test server for `auth.getToken` and `auth.getSession`; fake `SecretStore` holds the three keys and no file does; the callback refuses any URL not on `https://www.last.fm/`; a repo grep test finds no Last.fm key.
  - Steering: `tech.md` row `open` =5.4.5; README egress line `www.last.fm` (opened in the user's browser, not contacted by the app).

- [ ] 21. Scrobble finished plays through the queue
  - Crate and files: `nemo-integrations` (`src/lastfm/worker.rs` new, `src/service.rs`).
  - Does: On `TrackStarted` sends now-playing once, no retry. On `TrackEnded` with `counts` (the shared `PlayRule`) queues a row with the start time; rows without artist or title are not queued. A tokio task sends the oldest 50 in order and retries network errors and codes 11 and 16 with backoff from 30 s, doubling, up to 30 min.
  - _Requirements: 13, 14, 15, 16_
  - Depends on: integrations#13, integrations#20
  - Wave: 2
  - Checks: test server: now-playing once; batch of 50 in order; restart keeps the queue; offline then online sends all; tracks of 30 s or less and paused time never scrobble.

- [ ] 22. Handle Last.fm rejections and disconnect
  - Crate and files: `nemo-integrations` (`src/lastfm/worker.rs`, `src/lastfm/auth.rs`).
  - Does: Error 9 stops sending, sets `NeedsReconnect` and keeps the queue. Any other batch error splits the batch until the single bad row is found and dropped; ignored scrobbles count as sent. `lastfm_disconnect` deletes the session key and the queue. Sign-out registers no wipe step, so Last.fm settings stay.
  - _Requirements: 17, 18, 19, 20_
  - Depends on: integrations#21
  - Wave: 2
  - Checks: test server: error 9 keeps rows; one malformed row in 50 is dropped and 49 sent; disconnect empties key and queue; sign-out flow leaves the keys (no `SignOutWipe` impl).

- [ ] 23. Add the Last.fm row in Settings
  - Crate and files: `nemo-ui` (Settings "Integrations", `src/copy.rs`); `nemo-app` (wiring).
  - Does: `OptInRow` for Last.fm, off by default, with the criterion 10 text. Without keys: a short guide with a link to Last.fm's API account page and two fields. With keys: "Connect Last.fm", "I have approved", connected user, queue length, "Disconnect Last.fm", the host line, and "cannot store the key on this computer" when the store is missing.
  - _Requirements: 10, 11, 11a, 17, 19, 29, 30_
  - Depends on: integrations#2, integrations#3, integrations#22
  - Wave: 3
  - Checks: `iced_test` snapshots per `LastFmStatus`; key fields never echo the key; keyboard order and focus; `design.md` checklist with a screenshot.

- [ ] 24. Show the playing song as Discord presence
  - Crate and files: `nemo-integrations` (`src/discord.rs` new, `src/service.rs`).
  - Does: One thread owns the IPC client. Track start, resume and seek set the activity (title, artist, start time, listening type); pause, stop, switch off and exit clear it. At most one update per 15 s, newest wins; retries connect every 30 s while on. `DISCORD_APPLICATION_ID` holds the owner's id; with it empty, `discord()` is false and the row is not offered. No image and no other host.
  - _Requirements: 23, 24, 25, 26, 27, 28_
  - Depends on: integrations#11, integrations#13
  - Wave: 2
  - Checks: pure presence builder (padding under 2 and cutting over 128 bytes on a char boundary); rate limiter with a fake clock; fake IPC server for connect, retry and clear. Needs the owner's Discord application id.
  - Steering: `tech.md` row `discord-rich-presence` =1.1.0 (or a note that own IPC is used, per the spike); README: Discord presence adds no host.

- [ ] 25. Add the Discord presence row in Settings
  - Crate and files: `nemo-ui` (Settings "Integrations", `src/copy.rs`).
  - Does: `OptInRow` for Discord, off by default, with the criterion 22 text and "none on the internet" as the host line. Shown only when an application id is built in.
  - _Requirements: 22, 29, 30_
  - Depends on: integrations#2, integrations#3, integrations#24
  - Wave: 3
  - Checks: `iced_test` snapshot; hidden with an empty id; keyboard order and focus.

- [ ] 26. Compose release notifications as plain text
  - Crate and files: `nemo-integrations` (`src/notify/mod.rs`, `src/notify/compose.rs` new).
  - Does: `release_notification` gives "Artist - Title" for one release, "N new releases" with up to 3 artist names for several, and `None` for none. `sanitize` removes control characters, cuts the title at 100 and the body at 200 characters and escapes `&`, `<`, `>`. No cover, lyrics, token or account id.
  - _Requirements: 33, 36_
  - Depends on: integrations#6, account-actions#? (row 28, `ReleaseNotice`)
  - Wave: 2
  - Checks: pure tables (one, several, empty, 100 and 200 cuts, control characters, a title that looks like HTML).

- [ ] 27. Show notifications on Windows
  - Crate and files: `nemo-integrations` (`src/notify/windows.rs` new).
  - Does: A `nemo-notify` worker thread shows toasts with `APP_USER_MODEL_ID` `NemoMusic.NemoMusic`, a fixed tag and group so a newer toast replaces an older one, and normal urgency with no own sound. Missing Start Menu shortcut gives `Unavailable(NotInstalled)`. `on_activated` only sends `Activated`; `clear` removes the toast from history. A failed `notify` is dropped.
  - _Requirements: 31, 35, 37, 38, 39, 40_
  - Depends on: integrations#12, integrations#26
  - Wave: 2
  - Checks: shortcut check against a temp folder; `#[ignore]` manual test from an installed build (task 33). No host contacted.
  - Steering: `tech.md` row `tauri-winrt-notification` =0.8.1 (Windows only).

- [ ] 28. Show notifications on Linux
  - Crate and files: `nemo-integrations` (`src/notify/linux.rs` new).
  - Does: Talks to `org.freedesktop.Notifications` with blocking `zbus` on one thread: `GetCapabilities` once, `Notify` with `replaces_id`, `desktop-entry` `nemomusic`, urgency normal, `default` action only when listed, body always escaped. `ActionInvoked` becomes `Activated`; `clear` calls `CloseNotification`. No session bus gives `Unavailable(NoService)`.
  - _Requirements: 31, 35, 37, 38, 39, 40_
  - Depends on: integrations#26
  - Wave: 2
  - Checks: unit tests on the argument builder; `#[ignore]` test on GNOME and KDE daemons (task 33).
  - Steering: `tech.md` `zbus` =5.19.0 row names `nemo-integrations` as a user (Linux only).

- [ ] 29. Notify new releases from the release check
  - Crate and files: `nemo-app` (release flow in `src/backend.rs` or `src/releases.rs`); `nemo-ui` (focus query, `NotificationEvent` read per frame, open the release list).
  - Does: When a `ReleaseFeed` check ends and "OS notifications for new releases" is on and confirmed, asks whether the window is focused and visible; if not, takes the unannounced notices and calls `OsNotifier::notify` once. Activation raises the window and opens the release list. Switching off and sign-out call `clear(Releases)`.
  - _Requirements: 32, 33, 34, 35, 39, 40_
  - Depends on: integrations#3, integrations#27, integrations#28, account-actions#? (row 28, `ReleaseFeed` and the criterion 32 switch)
  - Wave: 2
  - Checks: fake notifier: one call per check, none while focused (notices stay unannounced), none when off, `clear` on switch-off and sign-out.

- [ ] 30. Add the notification opt-in text and status line
  - Crate and files: `nemo-integrations` (`src/info.rs`); `nemo-ui` (status line under the release-notification row, `src/copy.rs`).
  - Does: Adds the `OptInInfo` for OS release notifications: shown on the desktop, nothing sent to a server, risk on locked screens, nearby readers and OS history. Settings shows one line with the reason when `NotifierState` is `Unavailable`.
  - _Requirements: 32, 38, 40_
  - Depends on: integrations#2, integrations#29
  - Wave: 3
  - Checks: `iced_test` snapshot for each `NotifyUnavailable`; hosts list empty, egress-list test unchanged.

- [ ] 31. Package the notification entry points
  - Crate and files: installer definition (`workflow.md` section 7) and Linux packaging (`nemomusic.desktop`, icon `nemomusic`).
  - Does: The installer writes `NemoMusic.lnk` in the Start Menu with `System.AppUserModel.ID` = `NemoMusic.NemoMusic`; the Linux package ships `nemomusic.desktop` and the icon.
  - _Requirements: 38_
  - Depends on: integrations#27, integrations#28; the installer task of the release plan (not in these specs)
  - Wave: 3
  - Checks: clean VM install shows a toast; uninstall removes the shortcut. Needs the owner's signing setup for the installer.

- [ ] 32. Document integrations in the README
  - Crate and files: `README.md`.
  - Does: Describes media keys and the OS overlay, the Last.fm guide (own API account, public profile risk), Discord presence (visible to friends), and that notifications send nothing. Checks the egress lines added by tasks 19 and 20.
  - _Requirements: 10, 22, 29, 32_
  - Depends on: integrations#23, integrations#25, integrations#30
  - Wave: 3
  - Checks: egress-list test passes.
  - Steering: README risk texts for Last.fm and Discord.

- [ ] 33. Run the manual checks
  - Crate and files: none (results in the PR and the design's Risks).
  - Does: Windows overlay and media keys with the window minimized; `playerctl` and a desktop widget on GNOME and KDE; no D-Bus session; a real Last.fm scrobble; a real Discord client; toasts from an installed build (text `Tom & Jerry <3`, click, replace) on Windows 10 and 11, Linux daemons.
  - _Requirements: 1, 2, 3, 4, 5, 6, 7, 14, 23, 24, 26, 33, 34, 35, 37_
  - Depends on: integrations#18, integrations#23, integrations#25, integrations#31
  - Wave: 3
  - Checks: needs the owner: Last.fm API test account, the Discord application id, Windows and Linux machines.

## Definition of Done

See [workflow.md section 4](../../steering/workflow.md#4-definition-of-done). Also for this spec:
- `nemo-audio` is not changed; `cargo test -p nemo-audio --test bit_exact` keeps passing. The hub only reads position atomics, so no counting-allocator test is added.
- No Last.fm key, secret or session key, and no notification text above `debug`, appears in source, settings files or logs (the `tracing` capture tests pass).
- Every new host is on the README egress list and the `nemo-app` egress-list test passes.
