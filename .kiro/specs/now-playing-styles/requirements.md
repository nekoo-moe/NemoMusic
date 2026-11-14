## Goal

The user picks one of three Now Playing styles in settings: Classic, Material 3 Expressive, or a redesigned Apple Music-style player with word-by-word lyrics. Where an animated cover exists (Spotify Canvas), the user can turn it on and see it as a short silent loop in place of the still cover. Animated covers need the video decoder of the `video-playback` spec, so the first release ships still covers and the animated-cover interface only. Apple Music animated covers are Later. The look changes. Playback, the quality badge and the signal path do not. Reduced motion and the contrast rules stay.

## User Stories

- As a listener, I want to choose how Now Playing looks, so that the screen fits my taste.
- As a listener who likes Apple Music, I want big lyrics that light up word by word over the cover, so that I can follow the singing.
- As a listener, I want a moving cover when the song has one, so that the screen feels alive.
- As a listener sensitive to motion, I want everything to stay still, so that the screen stays comfortable.
- As a privacy-minded listener, I want to know what each animated-cover source sends before I turn it on.

## Acceptance Criteria

### Styles
1. THE SYSTEM SHALL offer three styles: "Classic", "Material 3 Expressive" and "Apple Music".
2. WHEN the app starts for the first time, THE SYSTEM SHALL use "Material 3 Expressive", the style of today's Now Playing pane and full-screen view (`docs/ui-redesign.md`). Existing users see no change after the update.
3. THE SYSTEM SHALL show the style choice in Settings with a small preview of each, and apply it at once without a restart. It SHALL keep the choice across restarts.
4. THE SYSTEM SHALL apply the chosen style to the right Now Playing pane, the full-screen Now Playing and the compact layout. The player bar, queue and navigation are unchanged.
5. THE SYSTEM SHALL show the same facts in every style: title, artist, cover, seek bar, controls, the honest quality badge, the signal-path entry, the lyrics provider footer and credit, the lyrics offset control and the "Back to current line" pill.
6. THE SYSTEM SHALL NOT change playback, gapless, the output path or any sample because of the style or an animated cover.
7. WHEN the style changes during playback, THE SYSTEM SHALL keep playing and keep the lyrics position without a gap.

### Classic
8. THE SYSTEM SHALL show Classic as a calm column: large still cover, title, artist, seek bar and controls, with lyrics one tap away on a tab. It uses the existing tokens and no new color roles.

### Material 3 Expressive
9. THE SYSTEM SHALL keep today's layout, canvas, per-piece lyric highlight and all of `design.md`, with no rule changed.

### Apple Music style
10. THE SYSTEM SHALL show cover, title, artist and controls on one side and large lyrics on the other in wide windows, and a single column with a lyrics tab in compact windows (`ui-reference.md`).
11. WHEN the lyrics are timed by word or syllable, THE SYSTEM SHALL highlight the active word as it is sung, with the sung part of the line in full ink and the rest dimmed. The highlight SHALL follow the timing in the lyrics (including the user's offset) and SHALL NOT invent word timing for line-timed lyrics.
12. WHEN lyrics are timed by line only, THE SYSTEM SHALL highlight the whole active line in the same style and say "Synced by line" in the footer.
13. THE SYSTEM SHALL show background-vocal lines (Unison and Better Lyrics provide them) smaller and quieter, and show duet singers on opposite sides when the lyrics carry a singer.
14. THE SYSTEM SHALL show lines that are not active smaller and dimmed, and SHALL keep every line at 4.5:1 contrast over the animated or still backdrop.
15. THE SYSTEM SHALL draw a backdrop from the cover colors. WHEN animated covers are off, the backdrop is static, as in `design.md` section 9.
16. THE SYSTEM SHALL keep the lyrics provider name, credit, granularity label and offset control visible and keyboard reachable.
17. THE SYSTEM SHALL keep these `design.md` rules in the Apple Music style: no glass behind lyric text (a scrim is used), contrast of 4.5:1 and 3:1, tokens only, spacing and shape scales, targets of at least 32 px, visible focus ring, honest quality labels, reduce-transparency fallback, 60 fps budget, no work on the audio thread, and lyrics never exported.
18. THE SYSTEM SHALL change only these rules for the Apple Music style: section 10 sizes and left alignment (larger lines, scale of inactive lines), section 10 "no karaoke wipes" (a smooth fill inside the active word is allowed), section 8 "no loops" and the 400 ms cap (a lyric fill lasts as long as its word), and section 9 "static" backdrop (it may move slowly, never while reduced motion is on).

### Animated covers
19. THE SYSTEM SHALL default animated covers to off, with one switch per source in Settings > Now Playing. A still cover always shows when the switch is off, no animation exists, or the video is not loaded yet.
19a. UNTIL the video decoder of `video-playback` is part of the app, THE SYSTEM SHALL show the animated-cover switches as unavailable with the reason, and SHALL NOT look up or download any animated cover.
20. WHEN the user turns on Spotify Canvas, THE SYSTEM SHALL first say that it uses the user's own Spotify sign-in, is unofficial, may restrict that account, and sends the song's Spotify track ID to Spotify.
21. THE SYSTEM SHALL keep the Spotify credential in the OS credential store only, never in a file or log, and SHALL delete it, and every cached Canvas, on Spotify sign-out and on NemoMusic sign-out.
22. THE SYSTEM SHALL NOT offer Apple Music animated covers (Later). The official Apple Music API needs a developer token the project cannot ship and does not return them, so only unofficial page scraping would work (see Decided).
23. WHEN a song plays, THE SYSTEM SHALL look for an animated cover only if its switch is on, and only for the song now playing, at human pace and without retry loops. A miss is cached locally as "none" for 7 days.
24. WHEN an animated cover exists, THE SYSTEM SHALL play it silently in the cover slot in a loop, with the still cover under it and a cross-fade of at most 400 ms. THE SYSTEM SHALL NOT play its audio.
25. WHILE the window is minimized, hidden, not on the Now Playing screen, or playback is paused, THE SYSTEM SHALL pause the loop and show the still frame.
26. WHILE reduced motion is on (the setting or the OS), THE SYSTEM SHALL show the still cover only and SHALL NOT download the video.
27. THE SYSTEM SHALL NOT use more than one video decoder at a time, SHALL stop it on style change, and SHALL NOT let a decoder stall delay audio.
28. THE SYSTEM SHALL keep animated covers in the app's private cache only, never export them, cap their size, and delete them on sign-out of the source and on "Clear cache".
29. THE SYSTEM SHALL label an animated cover as such ("Animated cover: Spotify") and SHALL NOT use it to claim any audio quality.
30. IF the network is down, the host refuses, or the video cannot be decoded, THEN THE SYSTEM SHALL keep the still cover and show no error on the Now Playing screen, only a line in Settings.
31. WHEN a new host is used, THE SYSTEM SHALL list it in the README egress list in the same change.

## Out of Scope

- Playing music videos (the `video-playback` spec).
- Animated covers for local files, SoundCloud songs and artist pages.
- User-made animated covers or uploads.
- Sound from an animated cover.
- Apple Music animated covers (Later, see Decided).
- Lyrics editing (the `lyrics-plus` spec) and the lyrics image.
- Android Auto and phone lock-screen layouts.

## Open Questions

Decided by the owner (2026-11-14):
- "Classic" is the calm column with lyrics on a tab, as criterion 8 says. There is no older design to match.
- Decoder: animated covers wait for `video-playback` (`nemo-video`). This spec defines the animated-art interface now and ships still covers first (criterion 19a).
- Spotify Canvas goes through `nemo-spotify` and uses the user's own Spotify sign-in (criteria 20 and 21). The design picks the host, the request and the sign-in route.
- Animated covers are off by default, with one switch per source (criterion 19).
- Apple Music animated covers: feasibility checked on 2026-11-14. The public Apple Music API has an `editorialVideo` attribute, but it needs a developer token (a paid Apple developer account and a private key the project cannot ship), and an Apple forum thread reports it is not returned. The only working route in public tools is reading the HLS address from the public album page, which is unofficial scraping, covers albums only, and needs HLS playback. They are marked Later (criterion 22).

Settled by the design, no owner decision needed:
- The Apple Music style uses an animated cover only in the cover slot, never as a full-window backdrop. The backdrop may drift slowly (criterion 18).
- Font sizes and spring values of the Apple Music style are listed against `design.md` sections 8 to 10 in the design.
<!-- Sources:
.kiro/steering/design.md sections 1, 6 to 10, 13, 15; .kiro/steering/product.md (principle 4, Approved additions); .kiro/steering/coc.md (Owner Decisions 2026-11-14)
docs/ui-redesign.md sections 6c and 7; docs/ui-reference.md; crates/nemo-ui/src/gui/views/now_playing.rs, backdrop.rs, lyrics.rs; crates/nemo-ui/src/gui/app.rs (ToggleFullScreen); crates/nemo-ui/src/canvas.rs
docs/adr/0002-lyrics-providers.md (word and syllable timing, TTML, background vocals)
https://github.com/m8tec/apple-music-animated-artworks (Apple animated covers served as HLS m3u8, read from public pages)
https://developer.apple.com/forums/thread/696843 (editorialVideo not returned by the API)
https://github.com/ungive/discord-music-presence/issues/123 (Apple developer account needed for the API)
https://github.com/itsmeow/Spicetify-Canvas/blob/master/README.md (Spotify Canvas through protobuf)
https://github.com/Paxsenix0/Spotify-Canvas-API and https://github.com/squeeeezy/Wolframe-spotify-canvas (unofficial Canvas fetch, Pathfinder; unverified)
https://support.apple.com/en-za/HT204459 (Apple Music word-by-word lyrics)
https://github.com/amll-dev/applemusic-like-lyrics (word-by-word lyrics rendering reference)
-->
