# ADR 0001: YouTube Music sign-in with a Google account

Status: proposed - owner to confirm
Date: 2026-11-10
Related: product.md (Local only), tech.md (Google login, Credential store), coc.md (Privacy), ADR 0003.

## Context
The user signs in with a Google account. All data comes from YouTube Music (YTM) and that account. Everything runs locally: no project server, no relay, no telemetry, no shipped secret, no invented client ID. YTM has no public API, so every method here is unofficial and may breach YouTube's Terms of Service. The owner accepted that risk on 2026-11-10.

## Options compared
| | 1 WebView2 sign-in + cookies | 2 OAuth device flow (TV client) | 3 OAuth installed-app (PKCE, loopback) | 4 Cookie/header paste |
|---|---|---|---|---|
| 2FA and passkeys | Should work in a real Chromium page; unverified for passkeys and security keys | Yes, user signs in at google.com/device in a normal browser | Yes, system browser | Yes, done in the user's own browser |
| Google blocks embedded browsers | Risk. Google blocks OAuth in webviews (2021) and sign-in from CEF-type frameworks (2021). No rule found for plain WebView2 login; unverified, test it | No, system browser | No, system browser | No |
| YTM innertube accepts the credential | Yes. Same cookies and SAPISIDHASH as the web client (ytmapi-rs `BrowserToken`) | Doubtful. ytmusicapi issues #813 (Aug 2025) and #921 report HTTP 400 "invalid argument"; maintainers advise browser auth. Current state unverified | Unverified, likely no. A non-TV client getting innertube access is not documented (ytmusicapi #708 reports the Web client failing) | Yes, same as 1 |
| Lifetime and refresh | ytmusicapi docs: about 2 years unless the user logs out. YouTube rotates cookies of sessions still open in a browser (yt-dlp wiki), so a copied jar can die early. Use a private profile, close it after capture | ytmapi-rs notes access tokens last minutes; refresh token handled by `refresh()`. Refresh-token limits for unverified apps: unverified | Same as 2, plus Google consent-screen and scope review | Same as 1; worse if copied from a live tab |
| Needs a client secret | No | Yes. User must create their own Cloud project; ytmapi-rs takes `client_id` and `client_secret` (ytmusicapi docs since Nov 2024) | Yes, a client ID is needed. Shipping ours means the project owns a Google Cloud identity (conflicts with local-only); asking each user is heavy | No |
| Account-restriction risk | Request pattern equals a web client. No ban reports found for ytmapi-rs or ytmusicapi; unverified | Unknown | Unknown | Same as 1 |
| ytmapi-rs 0.3.4 today | No login UI. Feed the cookie string to `BrowserToken::from_str` | `OAuthTokenGenerator`, `OAuthToken::from_code`, `refresh` | None | `BrowserToken::from_str`, `from_cookie_file` |
| Build effort in Rust | Medium: second window (`wry` or `webview2-com`, unverified fit with iced), cookie read via CookieManager (HttpOnly included, unverified) | Low code, high setup burden for users | High: PKCE, loopback server, review | Very low code, poor UX |

Notes from the ytmapi-rs source: `BrowserToken` keeps the cookie string and SAPISID, reads the client version from music.youtube.com, and signs `SAPISIDHASH`. It takes the first `SAPISID=` followed by `;`, so the cookie string must not end with it. It sends no `X-Goog-AuthUser`, so multi-account users may need a fix upstream or a local patch.

## Credential storage
- Windows Credential Manager. `keyring` 4.x is now a sample/CLI crate; the library is `keyring-core` plus `windows-native-keyring-store` (both Apache-2.0/MIT, active in 2026). tech.md names `keyring`; it needs updating, and the exact crate pair needs owner approval (versions unverified).
- A generic credential blob holds at most 2560 bytes. A full cookie jar can exceed that, so store one entry per cookie (SAPISID, `__Secure-1PSID`, `__Secure-3PSID` and the rest) plus a manifest entry listing the names. No new crypto crate needed.
- Wrap values in `Secret<T>` (code.md). Never log them, never put them in fixtures.
- Do not pass the account cookies to `yt-dlp` by default. yt-dlp and downstream projects report that cookies can lead to account or IP restrictions at volume. Anonymous resolution stays the default; a cookie option, if needed for Premium quality, is a later owner decision.

## Local data store
- SQLite via `rusqlite` at `%APPDATA%\NemoMusic\account.db` (WAL). Roaming profile syncs this folder in some domains; prefer `%LOCALAPPDATA%` for cache tables. Owner to confirm.
- Tables: `library_song`, `library_album`, `library_artist`, `liked`, `playlist`, `playlist_item`, `history`, `sync_state`. Data is fetched from the account with continuation pages, human-paced, incremental, and on user action or app start only.
- The file is not encrypted (no SQLCipher in tech.md). It holds the user's own listening data; say so in Settings. Encryption is an open decision.
- No account e-mail or ID in logs. Thumbnails cache lives under `%LOCALAPPDATA%\NemoMusic\thumbs`.

## Sign-out (wipes everything local)
1. For cookie methods, first load the Google sign-out URL in the app's WebView so the session is ended server-side. Without that, local deletion leaves the session valid until the user logs out elsewhere. For OAuth, call Google's revoke endpoint. Both are unverified in practice; test in the spike.
2. Delete every Credential Manager entry in the manifest, then the manifest.
3. Close the DB, delete `account.db`, `-wal`, `-shm`, the thumbnails cache and the WebView2 user-data folder.
4. Drop in-memory tokens (zeroize). Show a summary of what was removed. On SSD, deletion is not secure erase; say so.
5. Test: after sign-out, `cmdkey /list` shows no NemoMusic entries and a file search finds no cookie names under the app folders.

## Network egress (auth and account only; add to the README list)
`accounts.google.com` and `*.gstatic.com` (sign-in page, methods 1 and 4 helper), `music.youtube.com` (innertube, via ytmapi-rs), `www.youtube.com` (yt-dlp), `i.ytimg.com` and `lh3.googleusercontent.com` (thumbnails), `oauth2.googleapis.com` and `www.google.com/device` (methods 2 and 3 only). The exact set is unverified; capture it in the spike.

## First-run notice (draft, owner to approve; shown before sign-in, no pre-checked boxes)
> NemoMusic is not made, approved or supported by Google or YouTube. It uses YouTube Music through unofficial means, which may break the YouTube Terms of Service. Google could limit or suspend the account you sign in with, and access can stop working without notice. Use at your own risk, and consider an account you can afford to lose. NemoMusic runs only on your computer and has no servers: your sign-in is kept in Windows Credential Manager and your library is stored on this PC. Signing out deletes both. Ad blocking and SponsorBlock can reduce income for artists; both can be switched off in Settings. YouTube Music audio is lossy. [Continue] [Cancel]

## Prior art: SimpMusic and a fork (checked 2026-11-10)
SimpMusic ([repo](https://github.com/maxrave-dev/SimpMusic), Kotlin Multiplatform) is a YouTube Music client with a desktop build. Its docs say plainly: "Desktop still has no embedded login browser." Android signs in through an in-app browser that captures the session. Desktop uses three other ways:
| SimpMusic desktop way | What it is | For NemoMusic |
|---|---|---|
| QR pairing from the Android app | Desktop runs a one-shot local server for 5 minutes. The phone scans a QR code that carries an AES-256-GCM key, then sends the selected sign-ins over the LAN. The key never travels over the network. | Not usable. NemoMusic has no phone app. Good pattern to remember: local only, short-lived, key in the QR. |
| Manual cookie paste with the "SimpMusic Utils" Chrome extension (MIT) | The user signs in to music.youtube.com in their own browser, the extension shows the cookies, the user pastes them in the app. | Same as our method 4. Confirms the fallback is accepted in practice. |
| Import from an installed browser profile | `BrowserLoginRepository.profiles()` and `cookies()` read cookies from browser profiles. macOS asks for file access first. | Rejected, see below. |
Details I could not verify: how SimpMusic decrypts browser cookies on Windows. SimpMusic also writes the used account's cookie to `ytdlp-cookie.txt` for `yt-dlp`. That is the open decision 10 in `product.md`.

Method 5, import from another browser's profile: rejected. It reads another app's cookie store. Chrome on Windows protects that store with App-Bound Encryption (named in the BitChord issue below), and working around it is what credential-stealing malware does. That conflicts with `coc.md`. A Firefox-only import is a possible later, owner-approved extra.

Evidence for method 1 (embedded sign-in): an issue in the BitChord project (a different app) reports a test with the JavaFX WebView, its default user agent, no spoofing. Google's sign-in to music.youtube.com worked and the cookie jar held the full session (SAPISID, `__Secure-1PAPISID`, SID, LOGIN_INFO). Tested on macOS only. It also found no passkeys and no browser password manager inside the window, and Google could reject that WebView later. This is one report, not our setup: WebView2 on Windows is still untested, so the spike below stays required.

SimpMusic's license: the GitHub API says GPL-3.0, its website says MIT. Read its `LICENSE` before copying any code. This repo is GPL-3.0. We take the ideas only.

## Recommendation
Choose method 1 (WebView2 sign-in capturing session cookies) as the primary path, with method 4 (guided cookie paste) built first and kept as the permanent fallback if Google blocks the embedded page. Reject methods 2 and 3: they need a Google Cloud client ID and secret (the project must not ship one, and users should not need their own), and innertube acceptance of OAuth tokens is currently reported broken. Reuse ytmapi-rs `BrowserToken` for both, so the WebView is only a cookie collector. If the spike shows the embedded page is blocked, ship method 4 only, with a clear step-by-step guide.

## Spike plan (do in this order; use the owner's own throwaway account)
1. Method 4, half a day. `BrowserToken::from_str` with a cookie string from a private browser window, then fetch library playlists and liked songs. Pass: data returns after an app restart (token reloaded from storage) and 24 h later, on an account with 2-step verification and a passkey. Fail: HTTP 400/401/403 or a login challenge.
2. Method 1, 2 days. Separate WebView2 window with its own user-data folder, loading the Google sign-in with `continue=https://music.youtube.com`. Test password + 2-step prompt, passkey, and security key. Pass: no "browser or app may not be secure" page, cookies read including HttpOnly ones, same fetch as step 1 works, still works 7 days later with the WebView closed. Fail: any block, or a forced re-login within 7 days.
3. Storage, half a day. Round-trip the per-cookie entries in Credential Manager and run the full sign-out including server-side logout. Pass: the sign-out test above is clean and the old cookies are rejected by YTM afterwards.
4. Observation, 7 days of normal use. Pass: no captcha, no unusual-activity prompt, no forced sign-out. Record the real host list for the README.

<!-- Sources (checked 2026-11-10):
https://docs.rs/ytmapi-rs/0.3.4/ytmapi_rs/auth/index.html
https://raw.githubusercontent.com/nick42d/youtui/main/ytmapi-rs/src/auth/browser.rs
https://raw.githubusercontent.com/nick42d/youtui/main/ytmapi-rs/src/auth/oauth.rs
https://ytmusicapi.readthedocs.io/en/stable/setup/oauth.html
https://ytmusicapi.readthedocs.io/en/stable/setup/browser.html
https://github.com/sigma67/ytmusicapi/issues/679
https://github.com/sigma67/ytmusicapi/issues/708
https://github.com/sigma67/ytmusicapi/issues/813
https://github.com/sigma67/ytmusicapi/issues/921
https://developers.googleblog.com/en/upcoming-security-changes-to-googles-oauth-20-authorization-endpoint-in-embedded-webviews/
https://developers.googleblog.com/en/guidance-to-developers-affected-by-our-effort-to-block-less-secure-browsers-and-applications/
https://github.com/yt-dlp/yt-dlp/wiki/extractors
https://github.com/kieraneglin/pinchflat/wiki/YouTube-Cookies
https://github.com/open-source-cooperative/keyring-rs
https://github.com/open-source-cooperative/windows-native-keyring-store
https://github.com/queueb-org/keyring-winbridge/blob/master/docs/protocol/v1alpha1.md
https://www.simpmusic.org/docs/guide/accounts-login
https://www.simpmusic.org/blogs/en/how-to-log-in-on-desktop-app
https://github.com/maxrave-dev/SimpMusic (BrowserLoginViewModel.kt, CLAUDE.md)
https://github.com/maxrave-dev/utils
https://github.com/kushagrasinghx/BitChord/issues/690
Not verified: WebView2 Google sign-in behavior, passkeys in WebView2, OAuth refresh-token limits, server-side logout effect, current ytmapi-rs OAuth success, keyring-core versions.
-->
