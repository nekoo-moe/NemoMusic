## Goal

The signed-in user can shape their YouTube Music account from NemoMusic. They can dislike a song so YouTube Music recommends it less. They can edit their playlists and see the change on YouTube Music. They can sign in with several Google accounts and switch quickly, each with its own library and recommendations. Premium accounts can get Premium audio through the signed-in session, and a switch controls it. Videos with several audio tracks play in the user's preferred language. Followed artists can notify the user about new releases, off by default and calm.

## User Stories

- As a listener, I want to dislike a song, so that YouTube Music stops suggesting it.
- As a playlist owner, I want to rename, describe, reorder and delete my playlists and change their privacy, so that I do not need the website.
- As a playlist owner, I want to add and remove songs, so that my playlists stay current.
- As a person with two Google accounts, I want to switch accounts in one or two actions, so that my work and private listening stay apart.
- As a Premium subscriber, I want Premium audio quality, so that I hear the best stream my plan allows.
- As a listener of videos with dubbed tracks, I want my language chosen first, so that I do not pick it every time.
- As a fan, I want to know about new releases from artists I follow, so that I do not miss them, without being nagged.

## Acceptance Criteria

1. WHEN a signed-in user dislikes a song, THE SYSTEM SHALL send the dislike to YouTube Music and show the song as disliked.
2. WHEN the user removes a dislike, THE SYSTEM SHALL clear it on YouTube Music and in the UI.
3. WHEN a disliked song is playing, THE SYSTEM SHALL keep playing it. Dislike never skips a track by itself.
4. WHEN the user likes a song that is disliked, THE SYSTEM SHALL replace the dislike with the like. Like and dislike are never both shown.
5. WHEN the user renames a playlist, edits its description, changes its privacy (private, unlisted, public), or deletes it, THE SYSTEM SHALL apply the change on YouTube Music and then update the local view.
6. WHEN the user deletes a playlist, THE SYSTEM SHALL ask for confirmation first and name the playlist.
7. WHEN the user adds a song to a playlist, removes one, or moves one, THE SYSTEM SHALL apply it on YouTube Music and show the new order.
8. WHEN a playlist is not owned by the user, THE SYSTEM SHALL hide or disable the edit actions.
9. IF an account action fails (offline, rejected, rate-limited), THEN THE SYSTEM SHALL leave the previous state, say what failed in one plain line, and offer a retry. It SHALL NOT retry in a loop.
10. WHILE the app is offline, THE SYSTEM SHALL disable account actions and SHALL NOT queue them for later.
11. WHEN the user adds a second Google account, THE SYSTEM SHALL run the normal sign-in and keep the first account signed in.
12. WHEN the user switches account, THE SYSTEM SHALL change the library, likes, history, home feed and playlists to that account's data, and SHALL NOT show the other account's data.
13. WHEN the user switches account, THE SYSTEM SHALL keep the current track playing unless the user stops it.
14. WHEN the account switcher is used, THE SYSTEM SHALL show the active account by display name, never by e-mail or ID in logs.
15. WHEN the user signs out of one account, THE SYSTEM SHALL delete only that account's credentials and cached data. WHEN the user signs out of all accounts, THE SYSTEM SHALL delete everything local that belongs to any account.
16. WHEN the account switcher is opened, THE SYSTEM SHALL be reachable and operable by keyboard alone.
17. THE SYSTEM SHALL provide a setting "Use Premium audio quality", off by default, with one switch. It SHALL say plainly that the signed-in session is passed to `yt-dlp`, that this may restrict the account, and that it only works for accounts with Premium.
18. WHEN the setting is on and the active account has Premium, THE SYSTEM SHALL request the best audio the plan allows through the signed-in session.
19. IF the account has no Premium, THEN THE SYSTEM SHALL play as before and SHALL NOT claim a higher quality.
20. WHEN the setting is off, THE SYSTEM SHALL resolve streams without the session, as it does today.
21. WHEN the session is passed to `yt-dlp`, THE SYSTEM SHALL keep it out of logs, command lines shown to the user, and files that outlive the process.
22. IF the best stream uses a codec the decoder cannot play in this build, THEN THE SYSTEM SHALL choose the best playable stream. The badge SHALL show the codec and bitrate actually playing, or "Unknown". Until the Opus decoder of `local-library` is in the build, Premium plays AAC. Once it is, THE SYSTEM SHALL choose Opus when its bitrate is at least that of the best AAC stream.
23. WHEN a stream is YouTube, THE SYSTEM SHALL NOT label it lossless, "HD" or "Hi-Res", Premium or not.
24. THE SYSTEM SHALL provide a setting "Preferred audio language". Its default is "Automatic" (the video's original track).
25. WHEN a video offers several audio tracks and one matches the preferred language, THE SYSTEM SHALL play that track.
26. IF no track matches, THEN THE SYSTEM SHALL play the default track and show which language is playing.
27. WHEN a video has several audio tracks, THE SYSTEM SHALL let the user switch track during playback.
28. THE SYSTEM SHALL provide "Notify me about new releases from followed artists", off by default.
29. WHEN the user turns it on, THE SYSTEM SHALL first say what is sent (the followed artists' channel IDs), to whom (`music.youtube.com`), and how often it checks.
30. WHEN it is on, THE SYSTEM SHALL check no more often than once every 6 hours, only while the app is open, and with the cheapest call that finds new releases. It SHALL NOT call once per followed artist in a burst.
31. WHEN a followed artist has a new release, THE SYSTEM SHALL add it to an in-app list. It SHALL NOT show a badge count, a popup or a sound.
32. WHERE the user separately turns on OS notifications, THE SYSTEM SHALL send at most one notification per check, grouped.
33. WHEN the user signs out, THE SYSTEM SHALL delete the notification list and its settings for that account.
34. WHERE the user has reduced motion on, THE SYSTEM SHALL show account switches and list changes without animation.
35. WHEN a new network host is used, THE SYSTEM SHALL list it in the README egress list in the same change. The hosts expected here are already listed (`music.youtube.com`, `www.youtube.com`, `accounts.google.com`).

36. THE SYSTEM SHALL support up to 5 signed-in accounts at once, each with its own session, and SHALL say so when the limit is reached.
37. WHEN the Premium setting ships, THE SYSTEM SHALL update ADR 0001 in the same change: the default stays anonymous and the setting is the one switch.
38. WHILE it is not known whether the active account has Premium, THE SYSTEM SHALL learn it from the first stream resolved with the session. It SHALL NOT make an extra request to find out.

## Out of Scope

- Dislikes used to skip, hide or filter songs on this computer.
- Editing playlists that belong to other people.
- Collaborative playlists.
- Pushing notifications from a server. The project runs none, so checks happen only while the app runs.
- Premium access for accounts without Premium, and any bypass of quality limits or DRM.
- Choosing a language for lyrics (see `lyrics-plus`).
- Importing cookies from another browser's profile (ADR 0001).

## Open Questions

Decided on 2026-11-14 (owner): "Use Premium audio quality" is off by default with one switch. Premium plays AAC until the Opus decoder lands. Several accounts need a spike (one session per account) with a fallback. Raw innertube calls are used where `ytmapi-rs` 0.3.4 lacks an action. The release check uses the cheapest human-paced call, at most once every few hours, only while the app runs.

1. Several accounts: does one cookie session per account work with `ytmapi-rs` (it sends no `X-Goog-AuthUser`), and does a second sign-in in a clean webview profile leave the first session valid? The spike settles it; the design lists the fallbacks.
2. Release checks: which call is cheapest and personal enough (the new-releases page matched against the subscription list, or one browse per artist at a slow pace)? The spike settles it.
3. Which marker in the `yt-dlp` answer tells that a format is Premium-only?

<!-- Sources:
- Repo: .kiro/steering/product.md, coc.md, tech.md; docs/adr/0001-youtube-music-auth.md; crates/nemo-app/src/merged.rs (rate_song), crates/nemo-ytm/src/session.rs (rate_song), crates/nemo-ytm/src/account.rs, crates/nemo-ytm/src/ytdlp.rs ("no cookies"), crates/nemo-audio/src/decoder.rs (supported_stream_codecs), crates/nemo-ui/src/gui/app.rs (CreateMsg, on_create_playlist)
- Premium up to 256 kbps: https://support.google.com/youtubemusic/answer/9076559
-->
