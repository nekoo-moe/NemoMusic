## Overview

A new crate, `nemo-integrations`, holds three services that watch the player and talk to the outside: OS media controls (Windows SMTC, Linux MPRIS) through `souvlaki`, Last.fm scrobbling, and Discord Rich Presence. They share one source of facts, the player-event hooks, which live in `nemo-core` and are also used by listening-stats. A fourth piece, `OsNotifier` (Windows toast, Linux desktop notification), is not a watcher: other features call it. Its first caller is the release notice of account-actions (design in the paragraph "OS notifications" below).

`nemo-core` gets a pure `PlayTracker` that turns `PlayerEvent`s and position samples into higher-level `PlayHookEvent`s: a track started, the state changed, the position jumped (a seek), a track ended with the real heard time and whether it counts. `nemo-app` runs one small hub thread (`play_hub.rs`) that feeds the tracker and hands each event to the registered observers. Observers never block: each one pushes into its own channel and does its work on its own thread or tokio task. Nothing runs in the audio callback and `nemo-audio` is not changed.

Only OS media controls are on by default (criterion 9), since nothing leaves the machine. Last.fm and Discord are off, and each has one settings entry that shows what is sent, to which host and the risk text before the first request.

## Affected Crates

- `nemo-core`: `hooks.rs` (`PlayRule`, `PlayTracker`, `PlayHookEvent`, `FinishedPlay`, `PlayObserver`), `integrations.rs` (`IntegrationsService`, `LastFmStatus`, `NativeWindow`, `NoIntegrations`), `opt_in.rs` (`OptInInfo`, shared by every opt-in feature, ADR 0004), `notify.rs` (`OsNotifier` and its types, `NoNotifier`), and `Backend.integrations` and `Backend.notifier`.
- `nemo-integrations` (new): `media.rs` (souvlaki wrapper, event mapping, cover files), `lastfm/` (`api.rs` signing and calls, `auth.rs`, `queue.rs` SQLite, `worker.rs`), `discord.rs`, `notify/` (`compose.rs` pure text rules, `windows.rs`, `linux.rs`, `mod.rs`), `info.rs` (texts, hosts), `service.rs`.
- `nemo-app`: `play_hub.rs` (hub thread, observer list, flush at exit), wiring in `backend.rs`, `paths.integrations`, a callback that opens only `https://www.last.fm/` URLs in the default browser (`open` crate), and the window handle hand-over.
- `nemo-ui`: Settings section "Integrations" (three rows in the shared opt-in pattern, key and secret fields, connect, disconnect, queue length); `UiOptions::on_window_ready` that reports the native window id once iced has the window (iced `window::run`; the exact call is unverified for 0.14).
- `nemo-secrets`/`SecretStore` (ADR 0004): needed from that design. Keys used: `lastfm.api_key`, `lastfm.api_secret`, `lastfm.session_key`.
- `nemo-audio`, `nemo-ytm`, `nemo-library`, `nemo-lyrics`: no change. The mini player (`M`) is reused unchanged.

## Data Flow

```text
PlayerControl (events, position) -> hub thread (PlayTracker, 250 ms loop) -> observers (try_send)
   media worker thread  <- PlayHookEvent -> souvlaki -> OS            (OS key -> PlayerControl::send)
   lastfm tokio task    <- PlayHookEvent -> queue.sqlite3 -> ws.audioscrobbler.com (HTTPS)
   discord thread       <- PlayHookEvent -> local IPC socket or named pipe -> Discord app
```

- The hub drains one `EventReceiver` (`PlayerControl::subscribe`), reads `snapshot()` after an event to get the `Track`, and samples `position()` every 500 ms. While Cast holds the player parked (`RemotePlayback::remote_status().mode` is not `Local`), the local position does not move, so the hub samples `remote_status().position` and `state` instead; a room guest plays locally and needs no change. Heard time grows only while `Playing`, by `min(position delta, wall time elapsed)`; a forward jump beyond elapsed time is a seek (`PositionJumped`, no heard time); a backward jump resyncs. `TrackChanged` closes the old play and opens a new one, also for repeat one. `flush` at exit closes the last play.
- Observers get owned clones (`Track` is `Clone`); a full channel drops the event, never blocks the hub.
- OS media controls. The media worker owns `souvlaki::MediaControls`. On `TrackStarted` it sets metadata (title, artist, album, duration, cover) and on state or seek events it calls `set_playback` with the position, so the OS extrapolates and no per-second update is needed (within 1 s, criterion 5). Cover: the worker asks `ImageSource::image` for 512 px (already cached on disk by `nemo-ytm`'s image cache), re-encodes WebP to PNG with `image`, writes `cover-0.png` or `cover-1.png` (alternating, so the OS never reads a half-written file) in `paths.integrations/media`, and passes a `file://` URL built with `url::Url::from_file_path`. No remote cover URL goes to the OS, so no extra host (Open Question 4). Incoming `MediaControlEvent`s arrive on souvlaki's thread and map to `PlayerControl::send(..)`, which never blocks: Play, Pause, Toggle (reads `snapshot().state`), Next, Previous, Stop, `SetPosition` and `Seek` (relative, from `position()`, clamped). `OpenUri`, `Raise`, `Quit` are ignored. On Linux souvlaki uses the `use_zbus` backend: no `libdbus`.
- Last.fm. `TrackStarted` sends `track.updateNowPlaying` once, no retry. `TrackEnded` with `counts` (the same `PlayRule` as listening-stats) appends a row to `scrobbles` in the queue, timestamp = start time. The task sends the oldest 50 in order. Params are signed: `api_sig` = MD5 over the sorted `name+value` pairs (without `format`) plus the secret, via `md-5`. Responses are read as JSON (`format=json`), and the body is inspected whatever the HTTP status. Auth: `auth.getToken`, the UI opens `https://www.last.fm/api/auth/?api_key=..&token=..` in the browser, the user presses "I have approved", `auth.getSession` returns the session key, which goes to `SecretStore`. Last.fm's own page is the only place a password is typed.
- Discord. One thread owns `discord_rich_presence::DiscordIpcClient`. Track start, resume and a seek set the activity (details: title, state: artist, start timestamp = now minus position, listening type); pause, stop, switch off and exit clear it. At most one update per 15 s, the newest wins. When Discord is not running, `connect` fails and the thread retries every 30 s while the switch is on. No image and no external host are sent.

OS notifications. Nothing watches the player here. `nemo-app` runs the release flow of account-actions: when a `ReleaseFeed` check ends and `os_release_notices` is on, it asks the UI whether the window is focused and visible (criterion 34). If not, it calls `ReleaseFeed::take_unannounced`, `compose::release_notification` (pure: one release gives "Artist - Title", several give "N new releases" and up to 3 artist names; `sanitize` removes control characters, cuts the title at 100 and the body at 200 characters, and escapes `&`, `<`, `>`), and `OsNotifier::notify`. If the window is focused, nothing is taken, so the releases stay unannounced and unseen in the list. The notifier owns one worker thread (`nemo-notify`) and one channel of requests; callers never block.
- Windows: `tauri-winrt-notification` shows a toast with the app id `NemoMusic.NemoMusic` (constant `APP_USER_MODEL_ID`). An unpackaged desktop app needs a Start Menu shortcut that carries this id, so the installer (`workflow.md` section 7) writes `NemoMusic.lnk` with `System.AppUserModel.ID`. The notifier checks that the shortcut exists in `%APPDATA%\Microsoft\Windows\Start Menu\Programs`; if not (a `cargo run` build), `state()` is `Unavailable(NotInstalled)` and nothing is shown. The crate's `POWERSHELL_APP_ID` fallback is not used, because the toast would say it comes from PowerShell. The `on_activated` handler runs on a COM thread and only sends `NotificationEvent::Activated` through an `mpsc` channel; the UI reads it per frame and opens the release list. The toast tag and group are fixed, so a newer toast replaces an older one.
- Linux: the notifier talks to `org.freedesktop.Notifications` on the session bus with `zbus` (blocking API, one thread). It calls `GetCapabilities` once, then `Notify(app_name "NemoMusic", replaces_id = last id, icon "nemomusic", summary, body, actions ["default", "Open"] only if the server lists `actions`, hints {`desktop-entry` "nemomusic", `urgency` 1 (normal)}, expire_timeout -1)`. The body is escaped always, whether or not the server lists `body-markup`. The signals `ActionInvoked` and `NotificationClosed` are read on the same thread; "default" becomes `NotificationEvent::Activated`. The `.desktop` file (name `nemomusic`) is part of the Linux packaging task.
- `clear(group)` calls `CloseNotification(id)` on Linux and removes the toast from the history by tag and group on Windows. Sign-out and switching the setting off call it.

Queue storage `integrations.sqlite3` (WAL): `scrobbles(id INTEGER PRIMARY KEY, ts INTEGER, artist TEXT, track TEXT, album TEXT, duration_s INTEGER, attempts INTEGER)`, capped at 5,000 rows (oldest dropped, one warning), and `meta(key, value)` for the three switches and the Last.fm user name (public, not a secret).

## Interfaces

```rust
// nemo-core/src/hooks.rs  (pure, no I/O)
pub struct PlayRule;
impl PlayRule {
    pub const MIN_TRACK: Duration;            // 30 s: a track must be longer
    pub const MAX_THRESHOLD: Duration;        // 4 min
    pub fn threshold(duration: Option<Duration>) -> Option<Duration>;   // None when duration <= 30 s
    pub fn counts(duration: Option<Duration>, heard: Duration) -> bool;
}
pub struct FinishedPlay { pub track: Track, pub started_at: SystemTime, pub heard: Duration,
    pub duration: Option<Duration>, pub counts: bool }
pub enum PlayHookEvent {
    TrackStarted { track: Track, started_at: SystemTime },
    StateChanged { state: PlayState, position: Duration },
    PositionJumped { position: Duration },
    DurationKnown(Duration),
    TrackEnded(FinishedPlay),
}
pub trait PlayObserver: Send + Sync { fn on_event(&self, event: &PlayHookEvent); } // must not block
pub struct PlayTracker { /* private */ }
impl PlayTracker {
    pub fn new() -> Self;
    pub fn on_player_event(&mut self, ev: &PlayerEvent, snap: &PlayerSnapshot, now: Instant, wall: SystemTime, out: &mut Vec<PlayHookEvent>);
    pub fn tick(&mut self, position: Duration, now: Instant, out: &mut Vec<PlayHookEvent>);
    pub fn flush(&mut self, now: Instant, out: &mut Vec<PlayHookEvent>);
}

// nemo-core/src/integrations.rs
pub struct NativeWindow(pub usize);          // HWND on Windows, unused elsewhere
// nemo-core/src/opt_in.rs: the one type behind every opt-in row (ADR 0004, "Opt-in settings pattern")
pub struct OptInInfo { pub id: &'static str, pub title: &'static str, pub sends: &'static str,
    pub hosts: &'static [&'static str], pub risk: &'static str, pub version: u32, pub default_on: bool }
pub enum LastFmStatus { NoKeys, Ready, AwaitingApproval, Connected { user: String }, NeedsReconnect }
pub enum IntegrationError { Unavailable(String), Network(String), Rejected(String), Storage(String) }
pub trait IntegrationsService: Send + Sync {
    fn infos(&self) -> Vec<OptInInfo>;
    fn media_controls(&self) -> bool;
    fn set_media_controls(&self, on: bool);
    fn discord(&self) -> bool;                 // false and not offered while no application id is built in
    fn set_discord(&self, on: bool);
    fn lastfm_status(&self) -> LastFmStatus;
    fn lastfm_queue_len(&self) -> usize;
    fn lastfm_save_keys(&self, key: Secret<String>, secret: Secret<String>) -> Result<(), IntegrationError>;
    fn lastfm_begin(&self) -> BoxFuture<'_, Result<String, IntegrationError>>;      // the authorization URL
    fn lastfm_finish(&self) -> BoxFuture<'_, Result<String, IntegrationError>>;     // the user name
    fn lastfm_disconnect(&self) -> BoxFuture<'_, Result<(), IntegrationError>>;     // session key and queue
    fn window_ready(&self, window: NativeWindow);
}
// nemo-core/src/notify.rs  (no I/O)
pub enum NotificationGroup { Releases }                        // one live notification per group; a newer one replaces it
pub struct OsNotification { pub group: NotificationGroup, pub title: String, pub body: String }   // plain text, already sanitized
pub enum NotificationAction { OpenReleases }
pub enum NotificationEvent { Activated(NotificationAction), Closed(NotificationGroup) }
pub enum NotifyUnavailable { NoService, NotInstalled, Refused }
pub enum NotifierState { Ready, Unavailable(NotifyUnavailable) }
#[derive(Debug, thiserror::Error)]
pub enum NotifyError { #[error("notifications are not available")] Unavailable(NotifyUnavailable), #[error("the notification service failed")] Service }
pub trait NotificationReceiver: Send { fn try_next(&mut self) -> Option<NotificationEvent>; }
pub trait OsNotifier: Send + Sync {
    fn state(&self) -> NotifierState;                                  // cheap, safe every frame
    fn notify(&self, n: OsNotification) -> Result<(), NotifyError>;    // queues for the worker thread; never blocks
    fn clear(&self, group: NotificationGroup);
    fn subscribe(&self) -> Box<dyn NotificationReceiver>;
}
pub struct NoNotifier;                                            // state() Unavailable(NoService), notify() Err

// nemo-integrations/src/notify
pub const APP_USER_MODEL_ID: &str = "NemoMusic.NemoMusic";
pub fn release_notification(notices: &[ReleaseNotice]) -> Option<OsNotification>;   // pure; None for an empty list
pub fn sanitize(text: &str, max_chars: usize) -> String;                             // pure
// nemo-integrations
pub fn build(paths: &Path, secrets: Arc<dyn SecretStore>, player: Arc<dyn PlayerControl>,
    images: Arc<dyn ImageSource>, handle: tokio::runtime::Handle) -> Result<Integrations, IntegrationError>;
impl IntegrationsService for Integrations { /* ... */ }
impl PlayObserver for Integrations { /* fan-out to the three workers */ }
pub const DISCORD_APPLICATION_ID: &str;   // "" until the owner registers it
pub fn map_media_event(ev: souvlaki::MediaControlEvent, snap: &PlayerSnapshot, pos: Duration) -> Option<PlayerCommand>;
pub fn sign(params: &[(&str, &str)], secret: &str) -> String;
```

## Error Handling

- No OS media service (no D-Bus session, no HWND yet, SMTC refused): one `tracing::warn!` without metadata, playback continues, no dialog (criterion 7). The worker retries creating the controls only when the switch is flipped or a window id arrives.
- Last.fm: network error, error 11 or 16: keep the rows, retry with backoff 30 s, doubling, up to 30 min. Error 9: stop sending, status `NeedsReconnect`, keep the queue (criterion 17). Any other error on a batch: split in halves and resend until the single bad row is found and dropped (criterion 18), so one bad row does not discard 49 good ones. An `ignored` scrobble (code 1 to 5) counts as sent. Rows without an artist or title are not queued and are never guessed from file names. Now-playing failures are dropped.
- Responses and keys never reach logs: `Secret<String>` for key, secret and session key, and request bodies, which hold the signed fields and the session key, are never logged.
- Discord: connect, write and read errors are silent (one debug line without text). A rejected activity (text under 2 or over 128 bytes) is padded or cut on a character boundary before sending.
- Notifications: no service, a missing shortcut or a refusal gives `NotifierState::Unavailable(..)`; Settings shows one line with the reason (criterion 38) and the release check goes on, with the releases left unannounced. A failed `notify` is dropped, not retried, because a late toast is worse than none. Notification text, titles and artist names are never logged above `debug`, and then only their length.
- Credential store missing: Last.fm shows "cannot store the key on this computer" and stays off. Queue database cannot open: Last.fm is unavailable, the other two still work.
- Sign-out: nothing is wiped (criterion 20). Last.fm and Discord settings are not data of the YouTube Music account, so no wipe step is registered.

## Testing Strategy

- `PlayRule` and `PlayTracker`: tables for 29 s, 30 s, 31 s and 8-minute tracks, unknown duration, pause, seek forward and back, repeat one, gapless handover, skip; a `proptest` that heard time never exceeds wall time. (listening-stats uses the same tests.)
- `sign`: a fixed vector checked against an independent MD5 tool, including non-ASCII values and indexed batch names (`artist[10]` sorts before `artist[2]`).
- Last.fm against a local test server (the base URL is a constructor argument, plain HTTP only under `#[cfg(test)]`): now-playing once; batch of 50; order; restart keeps the queue; offline then online; error 9 keeps the queue; one bad row is isolated and dropped; disconnect deletes the session key and the queue; a capture of `tracing` output holds no key, secret, session key or title.
- `map_media_event` and the metadata builder are pure: every event maps as in criterion 2 and 3; metadata holds only title, artist, album, duration and the cover URL (criterion 8).
- Discord: a pure presence builder (padding, cutting, timestamps), a rate-limiter test with a fake clock (15 s, newest wins), a fake IPC server for connect, retry and clear.
- Notifications: `release_notification` and `sanitize` are pure tables (one release, several, empty, 100 and 200 character cuts, control characters, `&` and `<` escaped, a title that looks like HTML). A fake backend records calls: one call per check, none while the window is focused (and the notices stay unannounced), none when the switch is off, `clear` on switch-off and sign-out. The Windows check for the Start Menu shortcut runs against a temp folder. `#[ignore]` tests, run by hand: a toast with the text `Tom & Jerry <3` shows verbatim from an installed build (if the crate does not escape, `windows.rs` does), the click reaches `Activated`, and a Linux toast appears and replaces itself on a real notification daemon (GNOME and KDE).
- Manual: Windows overlay and media keys with the window minimized; `playerctl` and a desktop widget on Linux; a real Discord client.
- Audio is untouched; `cargo test -p nemo-audio --test bit_exact` must keep passing. The hub only reads `position()` atomics, so no counting-allocator test is added.

## Risks and Alternatives

- `souvlaki` 0.9.3 with `use_zbus` on Linux must pass `cargo deny` (unverified, not run here). If it fails, the owner decides between `use_dbus` (needs `libdbus-1-dev`, which `tech.md` avoids) and a small own MPRIS service on `zbus`.
- `souvlaki` needs a window handle on Windows and `file://` cover URLs there are unverified; if SMTC rejects them, fall back to no cover on Windows. Fetching the iced handle through `window::run` is also unverified.
- `discord-rich-presence` 1.1.0 (MIT, 2026-01-26) has no listed `rust-version` and depends on `uuid` 0.8, a second `uuid` next to 1.29.2 (allowed by the workspace lint, still worth a `deny` check). Discord's docs steer new work to the Social SDK, and local-IPC rate limits are unverified. Alternative: about 150 lines of own IPC over a named pipe or Unix socket; kept as the fallback if the crate fails `deny` or is abandoned.
- Last.fm needs the user to make an API account. Terms and rate limits are unverified. HTTPS on `ws.audioscrobbler.com` is not stated in the scrobbling page (it shows `http`); the client refuses plain HTTP because the session key travels in the body, so if HTTPS fails the feature reports an error instead of downgrading.
- A scrobble is queued when the track ends, so a crash loses the current one. Last.fm drops scrobbles older than about 2 weeks (ignored code 3); the user sees the count of ignored ones.
- Alternative for media keys without OS sessions: global hotkeys; rejected, they collide with other players and miss the overlay.
- No tray icon (owner decision), so media keys while the window is closed work only while the app runs. For the same reason a notification can only appear while the app runs; there is no startup from a click.
- Windows toasts need the installer to write the Start Menu shortcut with the app id, and whether Windows 10 and 11 show a toast from an unpackaged app with only that shortcut is unverified here (documented behaviour of the toast API; the manual test checks it). `tauri-winrt-notification` 0.8.1 builds toast XML from the text; whether it escapes markup is unverified, so the test above decides who escapes. The crate states tests on Windows 10 and 8.1 only.
- Alternative rejected: `notify-rust` 4.19.0 (MIT OR Apache-2.0, MSRV 1.89, released 2026-11-08, maintained). Its Windows side is the same crate we use, but its default Linux backend needs `zbus` ^6 beside the locked `zbus` 5.19.0 (two major versions of one stack), and its other Linux backend needs `libdbus`, which `tech.md` avoids. About 80 lines of `zbus` proxy code replace it. If the owner prefers fewer own lines, `notify-rust` is the swap and the second `zbus` is the cost. Also rejected: `winrt-notification` (last release 2022, replaced by the Tauri fork), `win-toast-notify` 0.1.6 (2024, one maintainer, no click events), a PowerShell subprocess (shows as PowerShell).
- Open Question 6: the Last.fm switch is independent of listening-stats, as proposed.

## Steering and README changes

- Opt-in: Last.fm and Discord presence use the shared `OptInRow` and `OptInInfo` (ADR 0004, "Opt-in settings pattern"); this design owns `OptInInfo` (`nemo-core/src/opt_in.rs`). OS media controls are the one default-on row (nothing leaves the computer). OS notifications are called by features that have their own switch.
- `tech.md`: `souvlaki` =0.9.3 (MIT, 2026-10-27, MSRV 1.87, `default-features = false`, `use_zbus` on Linux); `md-5` =0.11.0 (MIT OR Apache-2.0, 2026-03-27, MSRV 1.85); `discord-rich-presence` =1.1.0 (MIT, 2026-01-26, MSRV unlisted); `open` =5.4.5 (MIT, 2026-10-22, MSRV 1.62); existing pins: `reqwest`, `rusqlite`, `url`, `image`, `tokio`. Linux package list: no change if `use_zbus` passes `deny`.
- `structure.md`: new crate `nemo-integrations`, edge `nemo-app -> nemo-integrations`, depends on `nemo-core` only; new `nemo-core` modules `hooks` and `integrations`; `nemo-app/src/play_hub.rs`.
- `tech.md` rows for notifications: `tauri-winrt-notification` =0.8.1 (MIT OR Apache-2.0, released 2026-07-17, MSRV 1.82, Windows only; depends on `windows` ^0.62 which `Cargo.lock` has as 0.62.2, `windows-version` 0.1 and `thiserror`; no C toolchain expected, unverified); `zbus` =5.19.0 (MIT, released 2026-08-09, MSRV 1.87, Linux only; already built for the Secret Service store, same feature set `async-io` and `blocking-api`).
- README egress list: `ws.audioscrobbler.com` (Last.fm API, only when connected); `www.last.fm` (opened in the user's own browser for authorization, not contacted by the app). Discord presence adds no host (local IPC only). README risk text for Last.fm (public profile) and Discord (visible to friends).
- ADR 0004: add the hooks, the hub, the `nemo-core` `integrations` trait and `notify.rs` to Decision 2 (done in the Decision 4 table, rows 29 to 31). The README egress list gets no entry for notifications (local only). The installer plan adds the Start Menu shortcut with `NemoMusic.NemoMusic`; the Linux packaging adds `nemomusic.desktop` and an icon named `nemomusic`.
- The owner registers the Discord application, picks its name and gives the id to the implementing task.

<!-- Sources:
- docs/adr/0004-feature-wave-crates.md
- crates/nemo-core/src/player.rs, services.rs (PlayerControl, EventReceiver, ImageSource, Backend), track.rs, secret.rs
- crates/nemo-audio/src/control.rs, control_tick.rs (TrackChanged on every start, repeat one included)
- crates/nemo-app/src/backend.rs, main.rs, paths.rs (wiring); crates/nemo-ui/src/route.rs
- https://www.last.fm/api/scrobbling (rule, batch of 50, errors 9, 11, 16, ignored codes, retry)
- https://www.last.fm/api/desktopauth (token, browser approval, signed getSession)
- https://crates.io/api/v1/crates/{souvlaki,discord-rich-presence,md-5,open} and their dependency lists (queried 2026-11-14)
- https://docs.discord.com/developers/discord-social-sdk/development-guides/setting-rich-presence (Social SDK is the recommended path)
- https://crates.io/api/v1/crates/{tauri-winrt-notification,notify-rust,zbus,winrt-notification,win-toast-notify} and their dependency lists (queried 2026-11-14); https://raw.githubusercontent.com/tauri-apps/winrt-notification/dev/src/lib.rs (Toast API, app id, on_activated, POWERSHELL_APP_ID)
- https://specifications.freedesktop.org/notification/latest/protocol.html (Notify, GetCapabilities, ActionInvoked, NotificationClosed)
-->
