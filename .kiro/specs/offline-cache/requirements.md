## Goal

The user saves songs, albums and playlists inside NemoMusic and plays them with no network. The app can also keep the songs the user plays most (smart downloads, capped). Saved songs live in a private cache, play only in NemoMusic, never leave it, and are deleted on sign-out. The badge shows the real codec and bitrate of the cached file. Downloads are slow and polite, like a person using the app. This is not a download manager.

## User Stories

- As a commuter, I want to save an album, so that it plays on a train with no signal.
- As a listener, I want an offline mode, so that the app shows only what can play now and never waits on the network.
- As a listener, I want the app to keep my most-played songs, so that I do not choose each one.
- As a user with a small disk, I want to set a size limit and see what uses the space, so that the cache never surprises me.
- As a Premium subscriber, I want saved songs in the quality my account has, so that offline sounds as good as streaming.
- As a privacy-minded user, I want sign-out to delete every saved song, so that nothing stays behind.

## Acceptance Criteria

1. WHEN the user chooses "Save offline" on a song, album or playlist, THE SYSTEM SHALL queue its songs for download and show per-song state: queued, saving, saved, failed.
2. WHILE a song is saved, THE SYSTEM SHALL show a saved mark on every row and page where that song appears.
3. WHEN a saved song plays, THE SYSTEM SHALL read it from the cache and make no stream-lookup or audio request for it.
4. WHEN the user turns on offline mode, THE SYSTEM SHALL list and play only saved songs, and SHALL make no request to YouTube hosts.
5. IF the network is unavailable, THEN THE SYSTEM SHALL switch to offline mode and say so once, and SHALL leave it again only when the user turns it off.
6. THE SYSTEM SHALL keep offline mode off by default and let the user switch it from the settings and from a shortcut.
7. WHEN the user opens the storage view, THE SYSTEM SHALL show total size used, free disk space, and size per album, playlist, smart downloads and artwork.
8. WHEN the user sets a size limit (default 2 GB), THE SYSTEM SHALL stop starting downloads at the limit and SHALL NOT delete songs the user saved on purpose to make room.
9. WHEN the user removes a song, album, playlist or the whole cache, THE SYSTEM SHALL delete the files, show the freed space, and keep no copy.
10. WHEN a download starts, THE SYSTEM SHALL take one song at a time, wait 10 to 30 seconds between songs, and honor backoff and 429 answers; it SHALL NOT retry in a tight loop and SHALL NOT use parallel connections to speed up.
11. WHEN the user pauses downloads, THE SYSTEM SHALL stop at once and resume from the saved position or next song when asked.
12. IF a download fails, THEN THE SYSTEM SHALL mark the song failed with a plain reason, keep the other songs going, and offer a retry by the user.
13. WHEN a song is saved, THE SYSTEM SHALL keep the stream in the form the source served, with no re-encoding and no added processing.
14. WHEN the signed-in account has Premium and the Premium-quality switch of the account-actions spec is on, THE SYSTEM SHALL save the Premium stream; otherwise it SHALL save the best stream the anonymous lookup returns.
15. WHEN a saved song plays, THE SYSTEM SHALL fill the quality badge from the cached file's real codec and bitrate, and show "Unknown" for any fact it cannot measure.
16. THE SYSTEM SHALL NOT show "Lossless", "HD" or "Hi-Res" on a cached song.
17. WHEN a song is saved, THE SYSTEM SHALL also keep its cover art, its metadata, its lyrics from the lyrics cache and its SponsorBlock segments, so that they work offline.
18. WHILE a saved song plays offline and SponsorBlock is on, THE SYSTEM SHALL skip the stored segments like it does online.
19. WHEN the user turns on smart downloads, THE SYSTEM SHALL show what it stores, which hosts it contacts, the cap and the risk text before it starts. It SHALL be off by default and need an explicit choice.
20. WHILE smart downloads are on, THE SYSTEM SHALL keep at most 50 songs and at most 25% of the size limit, replace the least played first, and never touch songs the user saved on purpose.
21. THE SYSTEM SHALL count plays for smart downloads on this computer only.
22. THE SYSTEM SHALL store cache files only in the app's private data folder, as plain files with opaque names and no encryption, and SHALL NOT offer export, copy, "show in folder", drag-out or any path to the audio files.
23. WHEN the user signs out, THE SYSTEM SHALL delete every cached song, its metadata, its play counts for smart downloads and its queue state, and SHALL show that this happened.
24. IF a saved song's file is missing or damaged, THEN THE SYSTEM SHALL mark it unsaved, stream it if online, and NOT show a silent error.
25. WHEN YouTube removes or blocks a saved song, THE SYSTEM SHALL delete its file and index entry, tell the user which song was removed and why, and SHALL NOT keep a copy.
26. WHEN the cache screen, rows and buttons are used, THE SYSTEM SHALL reach every action by keyboard with a visible focus, and SHALL honor the reduced-motion setting for progress animation.
27. WHEN the README is read, THE SYSTEM SHALL name the risk of the offline cache and list any new host (none expected: the cache reuses hosts already listed).

## Out of Scope

- Exporting, copying, converting or sharing cached audio.
- Saving SoundCloud songs (decided by the soundcloud spec, not here).
- Saving video or podcast episodes.
- Sync of the cache between computers.
- Local files (they are already on disk).
- DRM bypass of any kind.

## Open Questions

Decided by the owner (2026-11-14):

- Cache files are plain files with opaque names in the app's private folder, with no encryption (it gives no real protection). Criterion 22.
- Caps: default limit 2 GB; smart downloads at most 50 songs and at most 25% of the limit; one download at a time with a 10 to 30 second pause. Criteria 8, 10, 20.
- A song YouTube removes or blocks is deleted and the user is told. Criterion 25.
- The cache lives in `nemo-ytm` (ADR 0004) with a SQLite index. `tech.md` line "build no audio cache" changes in the implementing task.

Still open (the design settles or the owner confirms):

1. Technical: how the player reads a cached file in its source container. The design settles this (reuse of the file byte source in `nemo-audio`).
2. Which spec owns the Premium switch: criterion 14 names the account-actions spec. The design reads that switch and does not define it.
3. Smart downloads for Premium and non-Premium accounts: the same caps apply to both. Whether the pause may be shorter on Wi-Fi is unverified and stays at 10 to 30 seconds; the risk text says "may restrict the account".

<!-- Sources:
.kiro/steering/product.md (Scope, Approved additions; Risks; Open decisions 1)
.kiro/steering/coc.md (Forbidden Features; Owner Decisions 2026-11-14; User Privacy)
.kiro/steering/tech.md (offline caching line)
docs/adr/0001-youtube-music-auth.md (cookies and yt-dlp)
docs/adr/0002-lyrics-providers.md, docs/adr/0003-adblock-sponsorblock.md
crates/nemo-ytm/src/ytdlp.rs, resolver.rs, stream.rs, images.rs, crates/nemo-lyrics/src/cache.rs
-->
