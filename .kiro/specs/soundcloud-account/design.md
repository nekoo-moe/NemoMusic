## Overview

The account is a second, small sibling of the Google account: one token in the OS credential store, a cached display name and avatar, one `GET /me` to check. It lives in `nemo-soundcloud` (`account.rs`, `auth.rs`, `signin.rs`, `wipe.rs`) behind a `nemo-core` trait, so the UI never sees a token. Library reads (`library.rs`) and the screens follow in later PRs.

## Affected Crates

- `nemo-signin`: `--site soundcloud` (default stays Google). Cookie rules in `cookies.rs` (pure, tested): only the `oauth_token` of `soundcloud.com`, shaped like a token, leaves the window. The SoundCloud window looks for the token every second at a fixed deadline (`Poller`; a busy page cannot push the deadline back), by three readers and the first answer wins: the cookie manager (`cookies_for_url` of `https://soundcloud.com`, then all cookies, filtered to `oauth_token` on a `soundcloud.com` host), `document.cookie` evaluated in the page (`token.rs`, pure and tested: quoted values, several cookies, percent encoding), and a small page hook (`hook.js`, injected for SoundCloud only) that reports the token of an `Authorization: OAuth` header the page sends to a `soundcloud.com` host. It also finishes when the page left `/signin` and a token is found. 25 s after a SoundCloud page loaded with no token it prints `{"hint":"no_token"}` once and keeps looking; the app writes `check` to its stdin to make it look at once. With `NEMO_SIGNIN_DEBUG=1` it writes `signin <line>` lines (names, domains, flags, lengths, shape classes, never values) to stderr, which the app copies to its log.
- `nemo-core`: `source_account.rs` (`SourceAccount`, `SourceAccountState`, `SourceAccountInfo`, `SourceAccountError`) and `Backend.soundcloud_account: Option<Arc<dyn SourceAccount>>`, as `soundcloud-full` design listed. It reuses `SignOutReport`, `SignOutWipe` and `WipeScope` of `account.rs`.
- `nemo-soundcloud`: `Auth` (token slot with an epoch, a hook run once when the session ends), `ScClient::get` (sends the token to the API host only; a second 401 with a token after a fresh client id is `ClientError::SessionEnded`), `ScClient::get_as` (explicit token, for the check before keeping it), `ScAccount`, `SignInWindow` (child process runner and `TokenSource` trait), `SessionCaches` (sign-out step for track records, kept streams and prefetch). `ScRequest` gets `authorization`. Small additive edits in `records.rs`, `opener.rs`, `stream.rs`, `catalog.rs`.
- `nemo-app`: builds the account only when the SoundCloud gate is open and a credential store exists; its own webview profile folder `webview-soundcloud` (apart from Google's) and cache file `account/soundcloud.json`.
- `nemo-ui` (PR 2 onwards): the settings row and account card, setup Sources step, then the screens.

## Data Flow

Sign-in: the UI calls `sign_in()` on the app runtime. The runner starts the app executable as `--signin --site soundcloud --data-dir <profile>`, reads at most 64 KB of stdout and the exit code (10 minute limit, child killed on drop). The token is checked (`normalize_token`: 8 to 2048 bytes of visible ASCII without space, quote, comma, semicolon, backslash and angle brackets, after removing `oauth_token=`, `OAuth ` and quotes, and decoding `%XX` when the decoded form is shaped), then `GET /me` with it. Only on a 200 is it written to the store (`soundcloud/oauth-token`), set on the client and the name and avatar cached (`id`, `username`, `avatar_url` only). A rejected token also deletes the window's profile folder, so a stale cookie cannot make the next window close at once.

Paste while the window is open: `sign_in_with_token` stops the window's call (the child process ends with it), puts the earlier state back, and checks the pasted token; the window's own call ends as `Cancelled`, which the page drops. `SourceAccount::sign_in_hint` and `recheck_sign_in` (defaulted) give the page the hint and the "check again" request.

Session end: `ScClient::get` sees 401, drops the client id, looks it up again, repeats once; a second 401 with a token raises the hook. The account then runs the same removal as sign-out and sets `Expired`. The token is gone, so the next call is anonymous and starts no new round. Public tracks keep playing; the call that met the end fails with "Sign in to SoundCloud again".

Start: with a stored token and a cache file the state is `SignedIn` at once, no request. Without the cache one `GET /me` runs in the background.

Account switch: signing in with another user id runs the registered steps first, so nothing of the first account shows under the second.

## Interfaces

```rust
// nemo-core::source_account
pub trait SourceAccount: Send + Sync {
    fn state(&self) -> SourceAccountState;
    fn sign_in(&self) -> BoxFuture<'_, Result<SourceAccountInfo, SourceAccountError>>;
    fn sign_in_with_token(&self, token: Secret<String>) -> BoxFuture<'_, Result<SourceAccountInfo, SourceAccountError>>;
    fn sign_out(&self) -> BoxFuture<'_, Result<SignOutReport, SourceAccountError>>;
}
// nemo-soundcloud
pub struct ScAccount;   // impl SourceAccount; new(client, store, window, files, handle); register_wipe; user_id
pub trait TokenSource { fn token(&self) -> BoxFuture<'_, Result<Secret<String>, SourceAccountError>>; }
pub struct SignInWindow; // impl TokenSource
```

## Error Handling

`SourceAccountError` texts are fixed and hold no token, name or id. 401 twice with a token is `Rejected` at sign-in and `Expired` for a saved session; 429 is "try again later"; offline keeps the earlier state. A store that refuses a write keeps no session at all; a store that refuses the delete at sign-out still signs out in this process and reports the failure. 403 is a refusal, not an ended session.

## Testing Strategy

Fake `SecretStore` (`MemoryStore`), fake `TokenSource`, fake `ScHttp` with recorded `Authorization` values: sign-in by window and by paste, shape refusal with zero requests, rejected token (two checks, nothing left, profile removed), storage failure, restart with and without cache, sign-out with a failing step, a store that cannot delete, session end (one round, token gone, later calls anonymous), 403, account switch, no token in `Debug`, URLs or the cache file. The runner is tested against fake helper scripts (token line, exit codes 2, 3, 64, junk, empty, timeout, missing program, the arguments it passes). `nemo-signin`: cookie filter (domain, name, shape), page recognition, argument parsing, the stdout line. Loopback server: the header is sent as `OAuth <token>` only when asked, marked sensitive, a redirect off https is not followed. Real service: none without an owner-supplied token (an `#[ignore]` example later).

## Risks and Alternatives

- SoundCloud may change the cookie name or block the embedded window: the paste fallback covers it (owner answer 1). The cookie shape check could refuse a future token format: the paste field says why.
- A 401 caused by a stale client id and a dead token look the same; the fresh-id repeat tells them apart at the cost of one 3 MB script read when the token is really dead.
- Polling the cookie jar every second while the window is open costs little and runs only in the window process.
- Alternatives rejected: reading the cookie from the user's browser profile (reads other sites' data); asking for the password (the app must never see it).
