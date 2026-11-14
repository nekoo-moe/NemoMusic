## Goal

The user can add their own timed lyrics to songs that have none, read a romanized line under lyrics in non-Latin scripts, share up to 4 lines of one song as an image, and read lyrics full screen in a landscape layout. Three new sources fill gaps: the user's own Spotify lyrics, YouTube captions as a last fallback, and an AI translation (BETA). The timing offset already exists; only its gaps are specified. Lyrics stay on this computer, except the image the user makes (`coc.md`).

## User Stories

- As a listener of a rare song, I want to paste lyrics and tap to time each line, so that the song gets synced lyrics.
- As a perfectionist, I want to refine the timing word by word, so that the highlight follows the singing.
- As a listener who cannot read Japanese, Korean or Chinese, I want a romanized line under each lyric line, so that I can sing along.
- As a listener, I want to share a short excerpt as an image with credit, so that I can show a line I love.
- As a listener at a desk, I want lyrics full screen in landscape, so that I can read them from a distance.
- As a listener, I want more sources, so that fewer songs show "No lyrics".
- As a listener, I want a translation of the lyrics, so that I can follow songs in other languages, knowing what is sent and where.

## Acceptance Criteria

### Own lyrics (editor)
1. WHEN a song has no lyrics, or the user opens "Edit lyrics", THE SYSTEM SHALL offer an editor with a text area for pasted plain lyrics.
2. WHILE the song plays in the editor, THE SYSTEM SHALL record the playback position as the start of the current line each time the user presses a tap key or button, then move to the next line.
3. THE SYSTEM SHALL let the user step back one line, re-tap a line, and nudge a line start by 50 ms, all by keyboard.
4. WHEN line timing is done, THE SYSTEM SHALL offer optional word timing as a second pass over the same text, line by line, with the same tap control.
5. WHEN the user opens the preview, THE SYSTEM SHALL play the song with the lyrics highlighted exactly as the lyrics view will show them.
6. IF a line has no start time, THEN THE SYSTEM SHALL refuse to save and name the first line that is missing. It SHALL NOT spread lines evenly over the song (ADR 0002).
7. WHEN the user saves, THE SYSTEM SHALL store the lyrics in the local lyrics database under the track key, labelled "Your lyrics", with the granularity "Synced by line" or "Synced by word".
8. WHILE a track has saved own lyrics, THE SYSTEM SHALL use them before any other provider and SHALL send no lyrics request for that song. The user can delete them to return to the normal chain.
9. THE SYSTEM SHALL keep own lyrics out of "Clear lyrics cache", off every network call and out of every export. Only the user removes them. Sign-out keeps them, since they are not account data.
10. IF the user edits lyrics fetched from a provider, THEN THE SYSTEM SHALL save a separate copy as own lyrics and leave the provider's cached text as it is.

### Romanization
11. THE SYSTEM SHALL offer romanization for these 12 languages: Japanese (Hepburn), Korean (Revised Romanization), Mandarin Chinese (Pinyin), Cantonese (Jyutping), Russian, Ukrainian, Greek, Arabic, Hebrew, Hindi, Thai and Persian. Each has a script that many listeners cannot read and a song catalog on YouTube Music.
12. THE SYSTEM SHALL do romanization on this computer and send no lyrics to any host for it.
13. WHEN romanization is on and a lyric line is in one of those languages, THE SYSTEM SHALL show the romanized text under that line in a smaller, quieter style that keeps 4.5:1 contrast.
14. THE SYSTEM SHALL default romanization to off, with a switch in Settings > Lyrics and a toggle in the lyrics tools. WHEN a provider already sends a romanization line, THE SYSTEM SHALL use it and label it as the provider's.
15. THE SYSTEM SHALL time a romanized line with its source line, including the user's offset, and SHALL NOT time it separately.
16. IF a line mixes scripts, THEN THE SYSTEM SHALL romanize only the runs it can and leave the rest as written.
17. THE SYSTEM SHALL label the scheme used ("Romanized: Hepburn") in the lyrics footer. It SHALL NOT claim the result is exact.

### Share as image
18. WHEN the user selects lines of the shown lyrics, THE SYSTEM SHALL allow at most 4 lines of one song, and SHALL block a 5th with a short message.
19. WHEN the user chooses "Share as image", THE SYSTEM SHALL show a preview with the selected lines, the song title, the artist and the lyrics provider credit (and the Unison submitter or "Lyrics via Better Lyrics" when the source asks for it).
20. THE SYSTEM SHALL save the image to a file the user picks or copy it to the clipboard, only after an explicit action, and SHALL send nothing to any host.
21. THE SYSTEM SHALL NOT offer to export full lyrics, copy all lyrics or share own lyrics as text. The image is the only export.
22. THE SYSTEM SHALL keep the image text readable at 4.5:1 on any cover, and every control reachable by keyboard.

### Full screen, landscape
23. WHEN the window is wider than tall and the user opens full-screen lyrics, THE SYSTEM SHALL show the lyrics large, with cover, title and artist beside them, and the offset control and provider footer visible.
24. THE SYSTEM SHALL keep the active line anchored as in `design.md` section 10 and scale with the window size and the OS text scale up to 200%.
25. WHEN reduced motion is on, THE SYSTEM SHALL switch lines without scrolling animation.
26. WHEN the user presses `Esc`, THE SYSTEM SHALL leave full screen and keep playback and the lyrics position.

### Timing offset (gaps only)
27. THE SYSTEM SHALL keep the existing per-track offset: -10 to +10 s in 50 ms steps, `[` and `]`, reset, saved in `lyrics_offset`.
28. WHEN the user changes the source of a song's lyrics, THE SYSTEM SHALL keep the offset and show it, since the offset belongs to the track.
29. THE SYSTEM SHALL apply the offset to word timing, romanization and translation lines, and to the editor preview.
30. WHEN the offset is not 0, THE SYSTEM SHALL show it in the full-screen landscape layout as `+0.30 s`.

### New sources
31. THE SYSTEM SHALL place the new sources in the existing provider chain and the settings list, each with a switch, its hosts, its risk note and its finest timing level. All three default to off.
32. WHEN the user turns on Spotify lyrics, THE SYSTEM SHALL first say that it uses the user's own Spotify sign-in, that it is unofficial, and that Spotify may restrict the account. It SHALL NOT use another person's session or a shared key.
33. WHEN the user signs in to Spotify, THE SYSTEM SHALL keep the credential in the OS credential store only and never in a file or log. WHEN the user signs out of Spotify, or out of NemoMusic, THE SYSTEM SHALL delete it and every cached Spotify lyric.
34. WHEN Spotify lyrics are shown, THE SYSTEM SHALL name "Spotify" as the provider and the third-party source it reports, when it reports one.
35. WHEN no other source has timed lyrics, THE SYSTEM SHALL try YouTube captions of the track's video as the last fallback, and label them "YouTube captions" and "Not lyrics-grade" when they are auto-generated.
36. THE SYSTEM SHALL NOT choose captions over any lyrics provider, and SHALL NOT make up word timing that the captions lack.
37. WHEN the user turns on AI translation, THE SYSTEM SHALL say what is sent (the lyric text of the shown song and the target language), to whom (the endpoint the user set under `ai-features`), and that the feature is BETA, before the first request.
38. WHILE no AI endpoint is set up, THE SYSTEM SHALL show the translation toggle as unavailable with the reason, and SHALL send nothing.
39. WHEN a translation arrives, THE SYSTEM SHALL show it under each line, labelled "AI translation (BETA)" with the language, store it only on this computer, and never export it.
40. IF a request fails, times out or the user is offline, THEN THE SYSTEM SHALL keep showing the lyrics already on screen, show a short message, and retry only on user action or the next song.
41. WHEN a new host is added by one of these sources, THE SYSTEM SHALL list it in the README egress list in the same change. The AI endpoint is listed as "the endpoint you set".

## Out of Scope

- Publishing lyrics to LRCLIB or any other service.
- Exporting or sharing own lyrics as text or an `.lrc` file.
- Import of lyrics files into the editor (the existing local `.lrc` sidecar stays as is).
- Machine alignment of lyrics to audio.
- Romanization through an online service.
- Android lyrics widgets.

## Open Questions

1. Spotify lyrics: which host and request, and how is the sign-in done (webview cookie, as for Google)? Public write-ups show `spclient.wg.spotify.com` with a web token, but the request is unverified, and Spotify lyrics come from a licensed third party. Does the owner accept the licence risk beyond the ToS risk already accepted?
2. AI translation sends copyrighted lyric text to the endpoint the user sets. `coc.md` says lyrics are never exported, except the image. Does the owner approve this as a second exception, opt-in and BETA? The spec only assumes the endpoint of the `ai-features` spec, which does not exist yet.
3. YouTube captions: which host serves them (`www.youtube.com` is on the egress list; a caption host is unverified), and is `yt-dlp` allowed to fetch them?
4. Romanization: is the list of 12 right? Which Rust crates cover them (candidates unverified), and which languages have no good offline option?
5. Should the offset be saved per source as well as per track, since two sources of one song can differ in timing?
6. Image look: one fixed style, or a few? Cover-derived colors must keep 4.5:1.
7. Is the AI translation language the UI language, or a setting?

<!-- Sources:
.kiro/steering/coc.md (Owner Decisions 2026-11-14, User Privacy), .kiro/steering/product.md (Approved additions), .kiro/steering/design.md section 10
docs/adr/0002-lyrics-providers.md (chain, cache, offset, granularity, credit)
crates/nemo-lyrics/src/cache.rs (lyrics_offset), crates/nemo-ui/src/state.rs (offset clamp -10 to +10 s), crates/nemo-ui/src/keys.rs ([ and ]), crates/nemo-ui/src/gui/views/now_playing.rs (offset buttons)
https://github.com/Paxsenix0/Spotify-Lyrics-API (unofficial Spotify lyrics, internal endpoint)
https://gist.github.com/gunsha (spclient.wg.spotify.com/color-lyrics/v2/track/{id}, unverified)
https://stackoverflow.com/questions/73704499/get-lyrics-data-from-spotify (LINE_SYNCED response shape)
-->
