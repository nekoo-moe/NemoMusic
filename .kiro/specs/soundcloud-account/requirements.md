## Goal

The listener signs in to their own SoundCloud account in NemoMusic and uses SoundCloud's library the way they use YouTube Music's: likes, playlists, followed artists, history, and later likes, follows and playlist edits written back. This is waves 2 and 4 of `.kiro/specs/soundcloud-full/` (its criteria 7 to 12, 21, 22, 24 to 27, 29 to 31), split into small PRs. Playback and the native client are merged and are not touched here.

Owner request (2026-11-19): "Add the whole SoundCloud music flow: login, playing music with low load delay, and features like playlists and so on working fully, like YTM."

## User Stories

- As a SoundCloud user, I want to sign in once with SoundCloud's own page, so that the app never sees my password.
- As a user whose sign-in window is blocked, I want to paste my session token in plain, guided steps, so that I can still sign in.
- As a user, I want to see my likes, playlists, followed artists and history, so that my account looks the same here and in the browser.
- As a careful user, I want sign-out to remove the token and every copy of my account data, so that nothing stays on this computer.
- As a listener, I want SoundCloud screens that look and move like the YouTube Music ones, so that the app stays one calm app.

## Acceptance Criteria

Sign-in (PR 1)
1. WHEN the user chooses sign-in, THE SYSTEM SHALL run `nemo-app --signin --site soundcloud --data-dir <folder>`, which opens `https://soundcloud.com/signin` in the webview, waits until a page of `soundcloud.com` has an `oauth_token` cookie, prints one line `{"token":"<value>"}` and exits 0 (2 closed, 3 no webview, 64 bad arguments), and SHALL return no other cookie and no password.
2. WHEN a token arrives (window or paste), THE SYSTEM SHALL check its shape, check it with one `GET /me`, and only then store it through `SecretStore` under `soundcloud/oauth-token`; it SHALL NOT write it to a file, a log, an error text or a URL.
3. WHILE signed in, THE SYSTEM SHALL send `Authorization: OAuth <token>` to `api-v2.soundcloud.com` only, never to the home page, scripts, images or media hosts.
4. IF a signed-in API call answers 401 again after a fresh client id, THEN THE SYSTEM SHALL treat the session as ended: stop sending the token, remove it and the cached account data, show `Expired` ("Sign in to SoundCloud again") and send no further retries. A 403 SHALL NOT end the session.
5. WHEN the user signs out, or the session ends, or another account signs in, THE SYSTEM SHALL delete the stored token, the cached name and avatar, the window's profile folder and the in-memory records and kept streams, run every registered sign-out step even after one fails, and name failed steps.
6. WHEN the sign-in window cannot start or is closed, THE SYSTEM SHALL say so in one plain sentence and keep the earlier state.
6a. (sign-in fix, 2026-11-20) WHEN the sign-in window shows a signed-in SoundCloud page, THE SYSTEM SHALL find the session token by the first of three readers that answers (the webview's cookie manager, `document.cookie` evaluated in the page, and the `Authorization: OAuth` header of the page's own requests to `soundcloud.com` hosts), accept a token of 8 to 2048 visible ASCII bytes (quotes removed, `%XX` decoded when the decoded form is shaped), and close the window when the page left `/signin` or a token is found.
6b. IF the window found no token 25 s after a SoundCloud page loaded, THEN THE SYSTEM SHALL say so in one plain sentence on the account card and offer `I am signed in, check again` (the window looks at once) next to `Paste a token instead`; a token pasted while the window is open SHALL close the window and take over.
6c. WHEN `NEMO_SIGNIN_DEBUG=1` is set, THE SYSTEM SHALL write one line per cookie of the sign-in window to stderr and to the app log (name, domain, secure and http-only flags, value length and shape class), and the names the `document.cookie`, `localStorage` and page-hydration probes found; it SHALL NOT write any value.

Settings and setup (PR 2)
7. THE SYSTEM SHALL show the SoundCloud opt-in row in Settings through the existing gate (On/Off, the risk text, the hosts), and the account card (Sign in with SoundCloud, name and avatar, Sign out with its confirm text) only while the switch is on and confirmed.
8. THE SYSTEM SHALL offer a guided token paste: a masked field that is never pre-filled and never echoed, plain steps for finding the `oauth_token` cookie in the user's own browser, and a sentence that the token stays on this computer.
9. THE SYSTEM SHALL describe SoundCloud honestly everywhere it is offered: unofficial access, may break SoundCloud's terms, tokens stay on this computer, audio is lossy (MP3 or AAC), plays are not counted for artists.
10. WHEN the setup flow reaches Sources, THE SYSTEM SHALL offer sign-in on the SoundCloud card (Skip stays) in place of "Public tracks only. No sign-in needed."

Library reads (PR 3)
11. WHILE signed in, THE SYSTEM SHALL read likes (tracks), the user's playlists and albums, liked playlists, followed artists, the feed and the play history on opening the library and on Refresh, page by page, following `next_href` only when it starts with `https://api-v2.soundcloud.com/`.
12. WHEN the account changes or signs out, THE SYSTEM SHALL clear what the previous account loaded before showing anything for the new one.

Screens (PRs 4 to 6)
13. THE SYSTEM SHALL offer a source switch (YouTube Music, SoundCloud, All) in the sidebar library list and on Search, and a SoundCloud library (Likes, Playlists, Following, History) and playlist, user and track pages built from the existing components (Sort, track rows, skeleton kit, motion module, Material look).
14. WHEN a SoundCloud screen loads, THE SYSTEM SHALL show the shared skeleton; WHEN it fails, a plain sentence with Retry; WHEN it is empty, an empty state. Play and queue SHALL work from every row.
15. THE SYSTEM SHALL show the badge as `MP3 128 kbps` or `AAC 160 kbps` style text from the real stream and SHALL NOT show "Lossless", "HD", "Hi-Fi" or "Hi-Res" on a SoundCloud track.

Writes (PR 7)
16. WHEN the user likes or unlikes a track, follows or unfollows an artist, creates, renames or deletes a playlist, or adds or removes a track, THE SYSTEM SHALL send it only after reading how the web app sends it, show the new state only after the server answers, say in one sentence when it failed, never retry a write, and never write silently. (2026-11-20: built are like, unlike, unfollow, create, add, remove and rename; follow waits for the owner, because the web app signs that one request with a secret of its own script, and deleting a playlist is not in the owner's list.)

Limits (all PRs)
17. THE SYSTEM SHALL keep the limits of `common3.md`: tokens only in the credential store, no token, URL, id or title in a log, no bulk export, requests human-paced with `Retry-After` honoured.

## Out of Scope

- Playback, HLS seek, fallback, Home and mixes (the other SoundCloud agent).
- Reposts, comments, messages, uploads, notifications.
- A password form, any sign-in that skips SoundCloud's own page, shared tokens.
- Telemetry and play-count events.

## Open Questions

Answered by the owner on 2026-11-19: (1) a guided paste of `oauth_token` is allowed if the window is blocked; (2) history is sent to the user's own history only when switched on, default off; (3) plays not counting for artists is acceptable, the README says so; (4) the risk text draft is accepted; (5) the setup-flow line and one `product.md` line may be edited; (6) a cold first play of 0.5 to 0.7 s is acceptable.

Open for the owner:
1. Turning SoundCloud off keeps the stored token until the user signs out (the account card is hidden while off). Should Off also sign out?
2. Social sign-in (Google, Apple, Facebook) uses pop-up windows, which the sign-in window does not open (not yet tested on a real account). The e-mail form should work in the window; the others may need the paste. Acceptable?
