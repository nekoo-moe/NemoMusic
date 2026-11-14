## Goal

NemoMusic shows up in the operating system as a media player: media keys, the Windows media overlay and Linux media widgets show the current song and control it, even when the window is hidden or minimized. Two opt-in services are added: Last.fm scrobbling and Discord Rich Presence. Each is off by default and tells the user what it sends, and to whom, before it is turned on.

## User Stories

- As a listener, I want my keyboard's media keys to control NemoMusic, so that I do not need the window.
- As a listener, I want the OS overlay or desktop widget to show title, artist, cover and position, so that I can see and control playback anywhere.
- As a Last.fm user, I want my plays scrobbled, so that my Last.fm profile stays complete.
- As a Discord user, I want to show what I play, so that friends can see it, and only if I choose.
- As a privacy-minded listener, I want every integration off until I turn it on, so that nothing is shared by surprise.

## Acceptance Criteria

### OS media controls

1. WHEN playback starts, THE SYSTEM SHALL publish title, artist, album, cover art, duration and position to the OS media session (Windows System Media Transport Controls, Linux MPRIS).
2. WHEN the user presses play, pause, play/pause, next, previous or stop on a media key or in the OS overlay, THE SYSTEM SHALL do the same as the in-app control.
3. WHEN the OS asks to seek or set the position, THE SYSTEM SHALL seek to that position.
4. WHILE the window is unfocused or minimized, THE SYSTEM SHALL still handle media keys and OS controls.
5. WHEN the track, state or position changes, THE SYSTEM SHALL update the OS session within 1 second.
6. WHEN playback is paused or stopped, THE SYSTEM SHALL show that state in the OS session.
7. IF the OS media service is missing (for example no D-Bus session), THEN THE SYSTEM SHALL keep playing, log one warning, and SHALL NOT show an error dialog.
8. THE SYSTEM SHALL publish the real title and artist only, and SHALL NOT put lyrics, tokens or account ids in the OS session.
9. THE SYSTEM SHALL turn OS media controls on by default, because nothing leaves the machine, and Settings SHALL have a switch to turn them off.

### Last.fm

10. WHEN the user opens Settings, THE SYSTEM SHALL show Last.fm as off, with a text that says: song titles, artists, albums and play times are sent to Last.fm, and they show on your public Last.fm profile unless you set it private there.
11. WHEN the user chooses "Connect Last.fm", THE SYSTEM SHALL open Last.fm's own authorization page in the user's browser. The SYSTEM SHALL NOT ask for the Last.fm password.
12. WHEN the user authorizes, THE SYSTEM SHALL keep the Last.fm session key in the OS credential store only.
13. WHEN a track starts, THE SYSTEM SHALL send a now-playing update. IF it fails, THEN THE SYSTEM SHALL NOT retry it.
14. WHEN a track is longer than 30 seconds and has been played for half its length or 4 minutes, whichever comes first, THE SYSTEM SHALL scrobble it with the time it started.
15. THE SYSTEM SHALL NOT scrobble tracks of 30 seconds or less, and SHALL NOT count paused time.
16. IF the network or Last.fm is unavailable, THEN THE SYSTEM SHALL keep the scrobble in a local queue that survives restarts, and SHALL send queued scrobbles in order, in batches of up to 50, when Last.fm is reachable again.
17. IF Last.fm reports an invalid session, THEN THE SYSTEM SHALL ask the user to connect again and SHALL keep the queue.
18. IF Last.fm rejects a scrobble as malformed, THEN THE SYSTEM SHALL drop that scrobble and SHALL NOT retry it.
19. WHEN the user chooses "Disconnect Last.fm", THE SYSTEM SHALL delete the session key and the queue.
20. WHEN the user signs out of NemoMusic, THE SYSTEM SHALL keep the Last.fm settings, because they are not account data of the YouTube Music account.
21. THE SYSTEM SHALL NOT contain a Last.fm API secret in source code, in the repository or in a log. See Open Questions for where key and secret come from.

### Discord Rich Presence

22. WHEN the user opens Settings, THE SYSTEM SHALL show Discord presence as off, with a text that says: the song title and artist you play are shown on your Discord profile, and your Discord friends and server members can see them.
23. WHEN the user turns Discord presence on and the Discord app is running, THE SYSTEM SHALL show the current song title and artist, and the elapsed time, through the local Discord app.
24. WHILE playback is paused or stopped, THE SYSTEM SHALL clear the presence.
25. WHEN the user turns Discord presence off or quits the app, THE SYSTEM SHALL clear the presence.
26. IF the Discord app is not running, THEN THE SYSTEM SHALL do nothing and retry quietly when it starts, with no error dialog.
27. THE SYSTEM SHALL update the presence at most once per track change or seek, and SHALL NOT flood Discord (the IPC limit is unverified, see Open Questions).
28. THE SYSTEM SHALL talk to the local Discord app only, and SHALL NOT send data to any other host for this feature.

### Common

29. WHEN a service is turned on, THE SYSTEM SHALL list the host it contacts in Settings. The README egress list SHALL name `ws.audioscrobbler.com` and `www.last.fm` (Last.fm). Discord presence adds no host.
30. WHEN the user uses only the keyboard, THE SYSTEM SHALL let them reach every switch and button of this feature with a visible focus.

## Out of Scope

- A tray icon for background play is not part of this spec (see Open Questions).
- Other scrobbling services (ListenBrainz and others).
- Discord buttons, "join" or "listen along" invites (see the listen-together spec).
- Last.fm love and unlove, recommendations and profile views.
- OS media controls on macOS.
- Android Auto and phone remote control.

## Open Questions

1. Last.fm key and secret. Last.fm signs every call with an API key and a shared secret, and the key comes from an API account (it is unverified whether Last.fm allows an open-source app to ship one). No secret may live in code. Options: (a) the owner registers one account and the secret is injected at build time (it still ends up in the binary, so it is not secret); (b) the user registers their own API account and pastes key and secret into Settings, which go into the OS credential store. Which one? The Last.fm API terms are unverified.
2. Discord. Presence needs a Discord application id, which is public, not secret. Who registers it, and under which name? Whether Discord allows third-party apps to use local IPC for this is unverified (the Rich Presence docs name only the Social SDK and the Embedded App SDK). Candidate crate: `discord-rich-presence` (licence and maintenance unverified).
3. OS media controls crate. Candidate: `souvlaki` 0.9.3, MIT, published 2026-10-27, Rust 1.87, 24 releases since 2021, covers Windows, Linux (MPRIS) and macOS. Its default Linux backend uses libdbus, which differs from the pure-Rust zbus choice in `tech.md`; a `use_zbus` feature exists. On Windows it needs a window handle. Which backend, and does `cargo deny` pass?
4. Cover art for the OS session: Windows and MPRIS take a URL or file. Sending a YouTube thumbnail URL to the OS is local; confirm no extra host is needed.
5. Tray icon: add it (hide to tray, keep playing) or leave it out? It needs the owner's call because it changes what closing the window does. Proposal: leave it out now.
6. Is the Last.fm switch tied to listening-stats history, or independent? Proposal: independent.
7. Exists today: no media-key, MPRIS, SMTC, Last.fm or Discord code. A mini player exists (`crates/nemo-ui/src/gui/app.rs`, key `M` in `crates/nemo-ui/src/keys.rs`) and is reused unchanged.

<!-- Sources:
- .kiro/steering/product.md (MVP: media-key support; Approved additions: integrations), coc.md (User Privacy)
- https://crates.io/api/v1/crates/souvlaki and https://github.com/Sinono3/souvlaki (version, licence MIT, Rust 1.87, features)
- https://www.last.fm/api/scrobbling (now-playing, scrobble rule, retry, batch of 50, error codes, endpoint)
- https://www.last.fm/api/desktopauth (browser authorization, signed calls, session key)
- https://docs.discord.com/developers/rich-presence/overview (presence is visible on the profile)
- https://docs.discord.com/developers/events/gateway-events (gateway status limit of 5 per 20 seconds; it may not apply to local IPC)
-->
