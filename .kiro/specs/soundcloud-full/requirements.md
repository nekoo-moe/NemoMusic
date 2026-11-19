## Goal

The listener signs in to their own SoundCloud account inside NemoMusic and uses SoundCloud the way they use YouTube Music: search, open tracks, playlists and artists, press play and hear the track within half a second, like tracks, follow artists, keep playlists, see their history, home mixes and related tracks. SoundCloud stays a second source with the same calm player. Its audio is lossy and the badge says so. The app states the unofficial-access risk in the README and in the SoundCloud settings. Signed out, SoundCloud still works for public tracks.

This spec replaces the "public tracks only, no sign-in, through `yt-dlp`" decisions of `.kiro/specs/soundcloud/` (owner, 2026-11-19). That spec's types (`Origin::SoundCloud`, `MediaUrl`, `Transport`, `UrlResolver`) stay and are reused.

## User Stories

- As a listener of remixes and independent artists, I want SoundCloud search, pages and playback to feel as quick as YouTube Music, so that I need no second app.
- As a SoundCloud user, I want to sign in once and see my likes, playlists, followed artists and history, so that my account is the same here as in the browser.
- As a listener, I want to like a track, follow an artist and fill my own playlists from NemoMusic, so that I never switch apps to do it.
- As a listener, I want one queue that mixes SoundCloud tracks and YouTube songs, with the badge showing the real codec and bitrate.
- As a careful user, I want to switch SoundCloud off, sign out and know what is wiped, so that I choose with my eyes open.

## Acceptance Criteria

Switch, honesty, limits
1. THE SYSTEM SHALL keep SoundCloud off by default and show its risk text and the hosts it contacts before the user turns it on. A user who confirmed the first SoundCloud notice (opt-in `version` 1) SHALL be asked again, because this notice adds sign-in.
2. WHILE SoundCloud is off or not confirmed, THE SYSTEM SHALL make no request to a SoundCloud host, build no SoundCloud client and show no SoundCloud entry.
3. THE SYSTEM SHALL NOT show "Lossless", "HD", "Hi-Fi" or "Hi-Res" on a SoundCloud track. WHEN a SoundCloud track plays, THE SYSTEM SHALL fill the badge from the real stream (codec and bitrate, for example "AAC 160 kbps"), else "Unknown", and show the source "SoundCloud" in the signal path.
4. THE SYSTEM SHALL keep requests human-paced: API calls start at least 250 ms apart, one at a time, no parallel bulk requests, no proxy rotation. WHEN SoundCloud answers 429, THE SYSTEM SHALL stop all SoundCloud API calls for `Retry-After` (60 s when absent, at most 15 min) and answer "try again later" without a request in that time.
5. THE SYSTEM SHALL NOT save, export or download SoundCloud audio. Audio read ahead stays in memory, at most three segments, and is dropped when the track ends or changes.
6. THE SYSTEM SHALL send the SoundCloud token, client id and every URL only to the SoundCloud hosts of the README egress list, and SHALL NOT write them to a log, file or test fixture.

Login and sign-out
7. WHEN the user chooses "Sign in to SoundCloud", THE SYSTEM SHALL open SoundCloud's own sign-in page in the sign-in window (a separate process), and SHALL NOT see or store the password.
8. WHEN the sign-in page leaves a signed-in session, THE SYSTEM SHALL read its `oauth_token` cookie, keep it only through `SecretStore` (OS credential store), check it with one `GET /me`, and show the account name and avatar.
9. WHILE signed in, THE SYSTEM SHALL send the token as `Authorization: OAuth <token>` to `api-v2.soundcloud.com` only, never to media, image or script hosts.
10. IF a signed-in call answers 401, THEN THE SYSTEM SHALL treat the session as ended, stop sending the token, show "Sign in to SoundCloud again" and keep playing public tracks.
11. IF the sign-in window is not available on this computer, or SoundCloud refuses it, THEN THE SYSTEM SHALL say so in plain words and SHALL NOT ask for a password.
12. WHEN the user signs out of SoundCloud (or of the app), THE SYSTEM SHALL delete the token from the credential store, the sign-in window's profile folder and every in-memory and on-disk copy of account data (likes, playlists, followings, history, mixes), keep going after a failed step and report it.

Playback
13. WHEN the user plays a SoundCloud track, THE SYSTEM SHALL pick the best playable stream from the track's transcodings with `rank_formats` (AAC or MP3, progressive file or HLS), ignore the adaptive `abr` list, and play it through the same output path as other sources.
14. WHEN the track's transcodings are already known (from a search, page or list) and the connection is warm, THE SYSTEM SHALL start audio within 0.5 s of the click. The stages (stream lookup, playlist, first segment, decode) are measured by an `#[ignore]` example and reported, not promised on every network.
15. WHEN a SoundCloud track plays and the queue has a next SoundCloud track, THE SYSTEM SHALL look up that track's stream and fetch its first segment ahead of time, one track ahead and no further.
16. WHEN a stream address stops working (HTTP 403 or 410), THE SYSTEM SHALL look it up again once, then show the plain reason. Addresses are kept in memory only until their signed expiry.
17. WHEN the chosen stream is HLS, THE SYSTEM SHALL play it through the same output path and, WHEN the listener seeks, start at the segment that holds the time and drop the audio before it, fetching only the segments from there on.
18. IF SoundCloud serves a track only encrypted (`cbc-encrypted-hls` or `ctr-encrypted-hls`), THEN THE SYSTEM SHALL say the track cannot play here and SHALL NOT try to bypass the protection.
19. IF SoundCloud serves only a preview (policy `SNIP`, snipped transcodings), THEN THE SYSTEM SHALL mark it "Preview", play or skip only the preview and SHALL NOT look for another source of the full track.
20. IF a track is private, removed, blocked in the region or fails to resolve, THEN THE SYSTEM SHALL show a plain reason on the track and go on to the next queue item. IF the network is down, THEN THE SYSTEM SHALL show an offline message on SoundCloud screens only.

Browse
21. WHEN the user searches with SoundCloud on, THE SYSTEM SHALL show SoundCloud tracks, playlists (albums included) and users in their own group apart from YouTube Music results, and load the next page only on request.
22. WHEN the user opens a SoundCloud track, playlist or user page, THE SYSTEM SHALL show title, uploader, artwork, duration, release date and description as the source gives them, with no attribution removed, and play, queue and radio actions like the YouTube pages.
23. WHEN the user starts a radio from a SoundCloud track, THE SYSTEM SHALL queue the related tracks SoundCloud returns for it, a page at a time.

Account library (signed in)
24. WHILE signed in, THE SYSTEM SHALL show the user's liked tracks, own and liked playlists, followed artists and play history in the library, fetched on opening and on refresh, not in the background.
25. WHEN the user likes or unlikes a track, THE SYSTEM SHALL change it on the account, show the new state at once and undo it with a plain message if SoundCloud refuses.
26. WHEN the user follows or unfollows an artist, THE SYSTEM SHALL do the same.
27. WHEN the user creates, renames, deletes or edits one of their own SoundCloud playlists, or adds or removes a track, THE SYSTEM SHALL write the change to the account and refresh that playlist.
28. WHILE signed in, THE SYSTEM SHALL show the SoundCloud home (the mixes and selections SoundCloud offers) and the charts, fetched when the home opens.
29. WHERE the user turned on "Add plays to my SoundCloud history" (off by default), THE SYSTEM SHALL add each SoundCloud track played to the account history once, and SHALL NOT send any other play or analytics event.

Interface
30. WHEN a user uses the keyboard only, THE SYSTEM SHALL reach search, results, pages, the sign-in button and the SoundCloud settings with a visible focus.
31. WHEN the reduced-motion setting is on, THE SYSTEM SHALL show SoundCloud screens with no added motion, and show loading as the skeleton of the shared kit.
32. WHEN the README is read, THE SYSTEM SHALL list each SoundCloud host, why it is contacted and that it is contacted only when SoundCloud is on, and state the unofficial-access risk, that plays through NemoMusic may not count in the artist's SoundCloud statistics, and that SoundCloud audio is lossy.

## Out of Scope

- Uploading, comments, reposts, messages, notifications, SoundCloud Go+ full tracks and any bypass of previews, limits or encryption.
- The official SoundCloud API and any shared or per-user API key. The `client_id` is the one the SoundCloud web app itself uses.
- Saving SoundCloud audio offline, cast of SoundCloud streams and any export.
- Lyrics for SoundCloud tracks, unless a provider matches by title and artist as it does today.
- Telemetry of any kind, including the play-count events of SoundCloud's own player.

## Open Questions

For the owner (each has a default the design uses):
1. Token fallback. If SoundCloud blocks sign-in in the embedded window, may the app offer a guided paste of the `oauth_token` cookie as the YouTube flow does? Default: no paste until the owner says so; the window failure shows plain text (criterion 11).
2. History. Criterion 29 sends the listener's plays to their own SoundCloud history only when switched on. Default: off.
3. Plays do not count for artists. The app sends no analytics events (`coc.md`), so SoundCloud probably does not count these plays in artist statistics; the README says so. Is that acceptable, or should the app stay unsigned for plays? Default: accept and say it.
4. Wording of the risk text and of the README row (draft in the ADR). Default: the ADR draft.
5. Steering text this spec may not edit still says SoundCloud is public tracks only: `design.md` (setup flow: "Public tracks only. No sign-in needed.") and the `soundcloud` line of `product.md` Scope. Default: the first UI PR edits the `design.md` line; the owner edits `product.md`.

Decided by the owner on 2026-11-19: login, a native client in `nemo-soundcloud`, `yt-dlp` only as a fallback, the crate-boundary change, new SoundCloud hosts, unofficial-access risk accepted.

<!-- Sources:
.kiro/steering/{coc,product,tech,structure}.md, docs/adr/0004-feature-wave-crates.md, .kiro/specs/soundcloud/
Probed 2026-11-19 (anonymous, a few requests): https://soundcloud.com/, https://api-v2.soundcloud.com/{search,search/tracks,search/playlists,resolve,tracks,tracks/<id>/related,users/<id>/tracks,playlists/<id>,me (401)}, the media/.../stream/hls call, a playlist.m3u8 and its init.mp4 and first segment
https://raw.githubusercontent.com/yt-dlp/yt-dlp/master/yt_dlp/extractor/soundcloud.py (Unlicense; client id regex, transcoding handling, DRM protocols, login header)
The web app's route table in a-v2.sndcdn.com/assets/*.js (me/*, users/:id/*, playlists, mixed-selections, system-playlists, play-history)
-->
