## Overview

Today one Google session drives the account-aware catalog. `GoogleAccount` (`nemo-ytm/src/account.rs`) holds one cookie in the OS store through `Vault` (`credentials.rs`), builds one `SessionCatalog`, and publishes it in a `SessionSlot` that `AccountCatalog` reads. This design keeps that shape and adds five things:

1. Several accounts: one cookie session and one `SessionCatalog` per account, a small index of accounts, and a switch that swaps the `SessionSlot` without touching playback.
2. Dislike and playlist edits, as one new `Catalog` method (`set_rating`) and one new trait (`PlaylistEditor`), both implemented in `nemo-ytm`.
3. Premium audio: when the switch is on, the active account's session reaches `yt-dlp` through a short-lived Netscape cookie file (Linux: a pipe), never on the command line.
4. A preferred audio language, applied when a stream is resolved, and a track switch during playback.
5. Release notices for followed artists: one cheap check every 6 hours while the app runs, stored per account.

The shared secret-store work is part of this spec (ADR 0004): `SecretStore` moves to `nemo-core`, and the new `nemo-secrets` crate holds the Windows and Linux stores. Every other design reaches secrets through that trait.

## Affected Crates

- `nemo-core`: new `secret_store.rs` (`SecretStore`, `StoreError`, `MemoryStore` behind feature `test-util`). `account.rs` gains `AccountId`, `AccountList`, `AccountSwitcher`, `SignOutWipe`. `catalog.rs` gains `LikeStatus`, `Catalog::set_rating`, `PlaylistEditor`, `PlaylistEditInfo`, two `CatalogError` variants. `stream.rs` gains `StreamRequest`, `AudioTrackInfo`, `StreamResolver::resolve_with`. New `releases.rs` (`ReleaseFeed`). `player.rs` gains `PlayerCommand::SwitchAudioTrack`.
- `nemo-secrets` (new): `WindowsStore`, `SecretServiceStore`, `ScopedStore`, `open_platform_store`. Depends on `nemo-core` only.
- `nemo-ytm`: `credentials.rs` keeps `Vault` (now keyed per account) and loses the stores and the trait. `account.rs` becomes the multi-account `GoogleAccount`. New `ratings.rs`, `playlist_edit.rs` (typed `ytmapi-rs` queries, raw fallback in `raw.rs` style), `premium.rs` (cookie file, format ranking), `releases.rs` (watcher and store). `ytdlp.rs` takes an optional session source and a `StreamRequest`.
- `nemo-ui`: account switcher, dislike button, playlist edit menus, three settings, the release list. Talks to `nemo-core` traits only.
- `nemo-app`: opens the platform store, builds `GoogleAccount`, registers the wipe steps, injects `PlaylistEditor`, `ReleaseFeed`, `AccountSwitcher`.
- `nemo-audio`: no change here. The Opus decoder (local-library design) widens `supported_stream_codecs()` and the resolver ranking picks it up.

## Data Flow

Threads are unchanged: UI thread (iced), the app's tokio runtime for network, the audio threads. Nothing here runs on the audio callback.

- Switch: the UI sends `AccountSwitcher::switch_to(id)`. `GoogleAccount` loads that account's cookie from `Vault`, builds (or reuses) its `SessionCatalog`, writes the `SessionSlot`, and bumps `AccountList::generation`. The UI clears library, likes, history, home and playlists on `AccountSwitched`. A request that started under an older generation is dropped when it answers (criterion 12). The player is not told anything: the current track keeps its already-open stream (criterion 13). The next track resolves with the new active account's session.
- Add account: `nemo-signin` runs with a fresh, empty webview folder (`accounts/pending/`), so Google asks for a login instead of resuming the active one. The cookie is probed, saved under the next free `AccountId`, and the folder is deleted. Other accounts stay signed in.
- Actions: UI message, `Catalog::set_rating` or a `PlaylistEditor` method on the runtime, then the local view updates only after YouTube Music accepts it. A failure keeps the old state and shows one line with a retry button. No loop, no queue (criteria 9, 10).
- Premium: `YtDlpResolver::resolve_with` asks `SessionSource::cookie()` for the active account when `StreamRequest::prefer_premium` is true. The cookie becomes `run/<random>.txt` (create-new, user-only access), is passed as `--cookies <file>`, and is deleted when the child exits (a drop guard) and by a sweep at start-up. On Linux the cookies go through the child's stdin as `--cookies /dev/stdin`. `--ignore-config` stays. The child is the only reader.
- Language: `StreamRequest::audio_language` filters the `formats` that carry a `language` field. No match falls back to the default track and the answer says which language plays.
- Releases: `ReleaseWatcher` is a tokio task owned by `GoogleAccount`. It sleeps, wakes at most every 6 hours, checks only the active account, writes new `ReleaseNotice` rows and updates `last_check`. It is not started when the switch is off or nobody is signed in.

Keyring layout, all through `SecretStore` (values at most 2000 bytes, as today): `ytm/index` (JSON: version, next id, active id, ids), `ytm/acct-<n>/manifest`, `ytm/acct-<n>/part-<i>`. Start-up migration: when `ytm/index` is missing and the old `session-manifest` exists, copy its parts to account 1, write the index last, then delete the old keys. A crash before the index write re-runs the copy.

## Interfaces

```rust
// nemo-core/src/secret_store.rs  (moved from nemo-ytm/src/credentials.rs; no I/O, no runtime)
/// Most bytes one value may hold. Windows allows 2560 per blob; this leaves a margin.
pub const MAX_SECRET_BYTES: usize = 2000;
/// A store failed. The text is fixed: platform errors can name the credential.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct StoreError(pub &'static str);
/// Keys are ASCII `[a-z0-9/_-]`, 1 to 96 bytes, as `<crate>/<purpose>[/<account>]`.
#[must_use] pub fn is_valid_key(key: &str) -> bool;
pub trait SecretStore: Send + Sync {
    fn set(&self, key: &str, value: &[u8]) -> Result<(), StoreError>; // value > MAX_SECRET_BYTES: Err
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError>;
    fn delete(&self, key: &str) -> Result<bool, StoreError>;          // true when it existed
}
/// Map-backed store for tests (feature `test-util`). Enforces the same limits.
#[cfg(feature = "test-util")] #[derive(Default)] pub struct MemoryStore { /* Mutex<BTreeMap<String, Vec<u8>>> */ }
#[cfg(feature = "test-util")] impl SecretStore for MemoryStore { /* set, get, delete */ }

// nemo-secrets/src/lib.rs
pub const SERVICE: &str = "NemoMusic";
#[cfg(windows)] pub struct WindowsStore { /* Arc<windows_native_keyring_store::Store> */ }
#[cfg(target_os = "linux")] pub struct SecretServiceStore { /* Arc<zbus_secret_service_keyring_store::Store> */ }
#[cfg(windows)] impl WindowsStore { pub fn new() -> Result<Self, StoreError>; }
#[cfg(target_os = "linux")] impl SecretServiceStore { pub fn new() -> Result<Self, StoreError>; }
pub fn open_platform_store() -> Result<std::sync::Arc<dyn SecretStore>, StoreError>;
/// Prefixes every key, so a crate cannot touch another crate's entries by mistake.
pub struct ScopedStore { /* inner: Arc<dyn SecretStore>, prefix: String */ }
impl ScopedStore { pub fn new(inner: std::sync::Arc<dyn SecretStore>, prefix: &str) -> Self; }
impl SecretStore for ScopedStore { /* set, get, delete */ }

// nemo-core/src/account.rs (additions)
pub const MAX_ACCOUNTS: usize = 5;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)] pub struct AccountId(pub u8); // local slot, not a Google id
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)] pub enum PremiumStatus { #[default] Unknown, Yes, No }
#[derive(Debug, Clone, PartialEq, Eq)] pub struct AccountEntry { pub id: AccountId, pub info: AccountInfo, pub premium: PremiumStatus }
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AccountList { pub accounts: Vec<AccountEntry>, pub active: Option<AccountId>, pub generation: u64 }
pub trait AccountSwitcher: Send + Sync {
    fn list(&self) -> AccountList; // cheap, safe every frame
    fn add_account(&self) -> BoxFuture<'_, Result<AccountEntry, AccountError>>;
    fn switch_to(&self, id: AccountId) -> BoxFuture<'_, Result<AccountList, AccountError>>;
    fn sign_out_account(&self, id: AccountId) -> BoxFuture<'_, Result<SignOutReport, AccountError>>;
}
// AccountError gains: LimitReached, UnknownAccount. `AccountService::sign_out` signs out every account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum WipeScope { Account(AccountId), All }
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)] #[error("{0}")] pub struct WipeError(pub &'static str);
/// One step of sign-out. Each crate that keeps account data registers one with `GoogleAccount`.
pub trait SignOutWipe: Send + Sync {
    fn name(&self) -> &'static str;
    fn wipe(&self, scope: WipeScope) -> Result<usize, WipeError>; // items removed
}

// nemo-core/src/catalog.rs (additions)
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum LikeStatus { Liked, Disliked, Indifferent }
// Catalog: new default method; `rate_song` keeps its signature and defaults to calling `set_rating`.
fn set_rating<'a>(&'a self, video_id: &'a VideoId, status: LikeStatus) -> BoxFuture<'a, Result<(), CatalogError>>;
// CatalogError gains: Rejected(String), RateLimited.
#[derive(Debug, Clone, PartialEq, Eq)] pub struct SetVideoId(pub String);
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum Privacy { Private, Unlisted, Public }
/// Present on `PlaylistDetail` only when the signed-in user owns the playlist (criterion 8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistEditInfo { pub privacy: Privacy, pub description: Option<String>, pub set_video_ids: Vec<SetVideoId> /* parallel to `tracks` */ }
// PlaylistDetail gains: `pub edit: Option<PlaylistEditInfo>`.
pub trait PlaylistEditor: Send + Sync {
    fn rename<'a>(&'a self, id: &'a BrowseId, title: &'a str) -> BoxFuture<'a, Result<(), CatalogError>>;
    fn set_description<'a>(&'a self, id: &'a BrowseId, text: &'a str) -> BoxFuture<'a, Result<(), CatalogError>>;
    fn set_privacy<'a>(&'a self, id: &'a BrowseId, privacy: Privacy) -> BoxFuture<'a, Result<(), CatalogError>>;
    fn delete<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<(), CatalogError>>;
    fn add_tracks<'a>(&'a self, id: &'a BrowseId, videos: &'a [VideoId]) -> BoxFuture<'a, Result<(), CatalogError>>;
    fn remove_track<'a>(&'a self, id: &'a BrowseId, video: &'a VideoId, entry: &'a SetVideoId) -> BoxFuture<'a, Result<(), CatalogError>>;
    fn move_track<'a>(&'a self, id: &'a BrowseId, entry: &'a SetVideoId, before: Option<&'a SetVideoId>) -> BoxFuture<'a, Result<(), CatalogError>>;
}

// nemo-core/src/stream.rs (additions; `ResolvedStream` gains `audio_language: Option<String>`, `audio_tracks: Vec<AudioTrackInfo>`)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StreamRequest { pub audio_language: Option<String> /* BCP 47, None = automatic */, pub prefer_premium: bool }
#[derive(Debug, Clone, PartialEq, Eq)] pub struct AudioTrackInfo { pub language: Option<String>, pub label: String, pub is_default: bool }
// StreamResolver: `fn resolve_with<'a>(&'a self, video: &'a VideoId, request: &'a StreamRequest) -> BoxFuture<'a, Result<ResolvedStream, StreamError>>;` default calls `resolve`.
// nemo-core/src/player.rs: `PlayerCommand::SwitchAudioTrack(Option<String>)`: resolve again with that language, reopen at the current position.

// nemo-core/src/releases.rs
#[derive(Debug, Clone, PartialEq, Eq)] pub struct ReleaseNotice { pub album: AlbumSummary, pub found_at: std::time::SystemTime, pub seen: bool }
pub trait ReleaseFeed: Send + Sync {
    fn list(&self) -> BoxFuture<'_, Result<Vec<ReleaseNotice>, CatalogError>>;
    fn mark_seen(&self, ids: &[BrowseId]) -> BoxFuture<'_, Result<(), CatalogError>>;
    /// New notices not yet sent to the OS; marks them as sent. For one grouped OS notification.
    fn take_unannounced(&self) -> BoxFuture<'_, Result<Vec<ReleaseNotice>, CatalogError>>;
}

// nemo-ytm (internal)
pub trait SessionSource: Send + Sync { fn cookie(&self) -> Option<nemo_core::secret::Secret<String>>; }
pub const MIN_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(6 * 3600);
pub fn rank_audio(codec: nemo_core::audio::Codec, kbps: Option<u32>) -> u32; // higher abr wins; Opus wins a tie
```

Traits I need from other designs: `OsNotifier` (integrations design) for criterion 32, a `Codec::Opus` entry in `supported_stream_codecs()` (local-library design), the captions parser is not needed here. Settings in `UiSettings`: `premium_audio: bool`, `audio_language: Option<String>`, `release_notices: bool`, `os_release_notices: bool`, with matching `SettingChange` variants.

## Error Handling

- Typed errors: `CatalogError::{Network, Rejected, RateLimited, Unavailable}` for actions, `AccountError::{LimitReached, UnknownAccount, Storage}` for accounts, `StoreError` and `WipeError` with fixed texts. The UI maps each to one plain line and a retry button. No retry loop.
- A raw call that YouTube answers with an unknown shape is `Rejected` with a fixed text. The response body is never put in an error (it can hold ids).
- Offline: actions return `Network` before sending when the app knows it is offline, and the UI disables them (criterion 10).
- Cookie file: if it cannot be created, Premium resolution fails closed to anonymous resolution and the UI says "Premium audio could not start". A failed delete is logged by path-free text and the start-up sweep retries.
- Premium rejected by YouTube (account restricted, sign-in required): `StreamError::NotAvailable`, the setting stays on, a banner suggests turning it off.
- Wipe: `sign_out_account` runs every registered `SignOutWipe`, keeps going after one fails, and reports the failures. The Vault entries go last, so a failed wipe can be retried while the account still exists.
- Switch to an account whose session no longer works: the entry stays, shows "sign in again", and the active account does not change.
- No `unwrap`; no cookie, name or id in logs (`Secret<T>`, criterion 14).

## Testing Strategy

- `nemo-core`: `is_valid_key`, `MemoryStore` limits, `AccountList` generation.
- `nemo-secrets`: ignored round-trip tests per platform (moved from `credentials.rs`), `ScopedStore` prefix tests with `MemoryStore`.
- `nemo-ytm`: `Vault` per-account keys; migration from `session-*` with a crash between steps; index never lists a missing account; a switch bumps `generation` and the old answer is dropped; the cookie file is absent after success, failure and timeout; a fake `yt-dlp` script checks that no argument or environment variable holds the cookie (extends the tests near `ytdlp.rs:598`); `rank_audio` picks AAC until Opus is allowed, then Opus at equal bitrate; edit and rating requests against the existing `test_server.rs` with recorded bodies; watcher timing with `tokio::time::pause`.
- Spike (first task, throwaway accounts, human-paced): two accounts signed in through fresh profiles; library differs; both work after 24 h and after one signs out. Pass: criteria 11, 12, 15. A second spike step checks one Premium account against `yt-dlp` and records which format marker means Premium.
- `nemo-ui`: headless tests for keyboard-only switcher, reduced motion, delete confirmation naming the playlist.
- Audio is not touched. The final task still runs `cargo test -p nemo-audio --test bit_exact`. No real-time code changes, so no counting-allocator test is needed.

## Risks and Alternatives

- Several accounts may fail the spike: `ytmapi-rs` 0.3.4 sends no `X-Goog-AuthUser`, and Google may share state between webview logins. Fallback 1: one multi-login cookie plus an `X-Goog-AuthUser` index per account, with our own signed requests (`sha2` is already a dependency) for the account-aware calls. Fallback 2: one account at a time; "switch" signs out and opens the Google chooser. Criteria 12 to 15 hold either way; criterion 36 drops to 1.
- Passing the session to `yt-dlp` can lead to account or IP limits (ADR 0001). The setting stays off by default and says so. The temp file is the weakest point: another process of the same user could read it for a few seconds. The spike checks the Windows file ACL and whether `yt-dlp` re-reads a deleted-on-close file; if not, the Premium option stays Linux-pipe only and Windows is blocked until fixed.
- Raw innertube calls (rename, privacy, move) can break when YouTube changes. `ytmapi-rs` 0.3.4 lists `EditPlaylistQuery` (title, description, privacy, swap), `AddPlaylistItemsQuery`, `RemovePlaylistItemsQuery`, `DeletePlaylistQuery` and `RateSongQuery` with `LikeStatus::Disliked`. The owner chose raw calls where the crate lacks an action; the first task checks each typed query field by field, uses it where it works, and writes raw calls (as in `raw.rs`) for the rest. A move is a swap in the crate, so `move_track` is raw.
- Release check: the cheapest plan is two calls (the subscription list, cached for a day, and the new-releases page matched against it). The page may not be personal. Fallback: one browse per followed artist, one every 90 seconds, oldest first, at most 40 per check.
- Premium status is learned from the first stream, so the UI says "active when your plan has it" until then.
- Departures from ADR 0004: none. Two shared traits beyond its expected list (`AccountSwitcher`, `SignOutWipe`) and `ReleaseFeed` are added to `nemo-core`; the ADR lists the expected set as open. `nemo-ytm` keeps no dependency on `nemo-secrets`: it receives the store as `Arc<dyn SecretStore>`.
- Alternative for the temp file: `--add-header "Cookie: ..."`. Rejected: it shows in the process list.

## Steering and README changes

- `tech.md`: replace rows "Credential store crates" and "Credential store" with: crates `keyring-core` =1.0.1 (MIT OR Apache-2.0, MSRV 1.85), `windows-native-keyring-store` =1.1.0 (MIT OR Apache-2.0, MSRV 1.88, released 2026-05-24), `zbus-secret-service-keyring-store` =1.0.1 (MIT OR Apache-2.0, MSRV 1.88, released 2026-08-15), now owned by `nemo-secrets`. Opus row: note that Premium plays AAC until the decoder lands.
- `structure.md`: add `nemo-secrets` (depends on `nemo-core` only); `nemo-app` depends on it; `nemo-ytm` loses its keyring dependencies; `nemo-core` gets feature `test-util`.
- README egress list: no new host (`music.youtube.com`, `www.youtube.com`, `accounts.google.com` are listed).
- ADR 0001: record the Premium switch (off by default, one setting), the multi-account result from the spike, the keyring layout, and the temp cookie file. ADR 0004: mark the `SecretStore` move done in the task that lands it.
- `nemo-ytm/Cargo.toml`: drop `keyring-core` and both store crates; add `rusqlite` =0.41.1 only if the offline-cache design has not already.

<!-- Sources:
- Repo: crates/nemo-ytm/src/{credentials,account,session,ytdlp,raw}.rs, crates/nemo-core/src/{account,catalog,stream,player,secret}.rs, crates/nemo-audio/src/decoder.rs, docs/adr/0001-youtube-music-auth.md, 0004-feature-wave-crates.md, .kiro/specs/linux-port/design.md
- ytmapi-rs 0.3.4 queries: https://docs.rs/ytmapi-rs/0.3.4/ytmapi_rs/query/index.html , EditPlaylistQuery, LikeStatus (docs.rs)
- Crates: https://crates.io/crates/keyring-core , windows-native-keyring-store , zbus-secret-service-keyring-store (queried 2026-11-14)
- Unverified: Premium format marker in yt-dlp output, personalised new-releases page, Windows delete-on-close with yt-dlp, `next` response `likeStatus`.
-->
