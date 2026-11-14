## Goal

The user searches SoundCloud next to YouTube Music, opens track, playlist and user pages, plays them, and mixes SoundCloud tracks with YouTube songs in one queue. SoundCloud is a second source with the same calm player. Its audio is lossy and the badge says so. The app states the Terms of Service risk in the README and in the SoundCloud settings. Tracks that SoundCloud serves only as a preview to non-subscribers stay previews.

## User Stories

- As a listener of remixes and independent artists, I want to search SoundCloud inside NemoMusic, so that I need no second app.
- As a listener, I want to put SoundCloud tracks and YouTube songs in one queue, so that one session plays both.
- As a listener, I want the badge to show the real codec and bitrate, so that I know what I hear.
- As a careful user, I want to switch SoundCloud off and read its risk, so that I choose with my eyes open.

## Acceptance Criteria

1. THE SYSTEM SHALL keep SoundCloud off by default and show its risk text and the hosts it contacts before the user turns it on.
2. WHILE SoundCloud is off, THE SYSTEM SHALL make no request to a SoundCloud host and show no SoundCloud result or entry.
3. WHEN SoundCloud is on and the user searches, THE SYSTEM SHALL show SoundCloud tracks, playlists and users in their own group, apart from YouTube Music results.
4. WHEN the user opens a SoundCloud track, playlist or user page, THE SYSTEM SHALL show title, uploader, artwork, duration and the uploader's name as the source gives them, with no attribution removed.
5. WHEN the user plays a SoundCloud track, THE SYSTEM SHALL resolve a stream and play it through the same output path as other sources.
6. WHEN the user adds SoundCloud tracks and YouTube songs to the queue, THE SYSTEM SHALL play them in order, with the same controls, history and keyboard shortcuts.
7. WHEN a SoundCloud track plays, THE SYSTEM SHALL fill the quality badge from the real stream: codec, bitrate if known, otherwise "Unknown".
8. THE SYSTEM SHALL NOT show "Lossless", "HD", "Hi-Fi" or "Hi-Res" on a SoundCloud track.
9. WHEN a SoundCloud track plays, THE SYSTEM SHALL show the source name "SoundCloud" in the signal-path view.
10. IF SoundCloud serves a track only as a preview to this user (for example a Go+ track without a subscription), THEN THE SYSTEM SHALL mark it "Preview" and play or skip only the preview, and SHALL NOT search for another source of the full track.
11. IF SoundCloud serves a track only in an encrypted form that the app's tools cannot read, THEN THE SYSTEM SHALL say the track cannot play here, and SHALL NOT try to bypass the protection.
12. IF a track is private, removed, blocked in the region or fails to resolve, THEN THE SYSTEM SHALL show a plain reason on the track, and go on to the next queue item.
13. IF the network is down, THEN THE SYSTEM SHALL show an offline message on SoundCloud screens and leave the YouTube Music screens working as before.
14. WHEN requests are made, THE SYSTEM SHALL keep them human-paced, honor 429 and backoff answers, and use no proxy rotation or parallel bulk requests.
15. THE SYSTEM SHALL pass no SoundCloud credential, cookie or token to any host other than SoundCloud, and SHALL write none to a file or a log.
16. WHEN the SoundCloud settings page is open, THE SYSTEM SHALL show the on/off switch, the risk text, the host list and the build's `yt-dlp` state.
17. WHEN a user uses the keyboard only, THE SYSTEM SHALL reach search, results, pages and the settings switch with a visible focus.
18. WHEN the reduced-motion setting is on, THE SYSTEM SHALL show SoundCloud screens with no added motion.
19. WHEN the README is read, THE SYSTEM SHALL list each SoundCloud host, why it is contacted, and that it is contacted only when SoundCloud is on.
20. THE SYSTEM SHALL NOT save SoundCloud tracks for offline use.

## Out of Scope

- SoundCloud sign-in, likes, reposts, followed artists, the user's own playlists and the SoundCloud home feed (see Open Questions 1).
- Saving SoundCloud audio offline, uploading, commenting and following.
- Any bypass of previews, Go+ limits or encrypted streams.
- Lyrics for SoundCloud tracks, unless a provider matches by title and artist as it does today.

## Open Questions

1. Sign-in. Proposal: out of scope for the first version. Only public content plays. Reason: the official route needs an app registration (see 3), and the unofficial route (a user's cookie or OAuth token passed to `yt-dlp`) adds a credential and an account-restriction risk. Owner decides whether likes and playlists come later.
2. `yt-dlp` has a SoundCloud extractor. Its source lists the formats `http_aac`, `hls_aac`, `http_opus`, `hls_opus`, `http_mp3` and `hls_mp3`, and it scrapes a web `client_id` and caches it (a `yt-dlp` debug log shows "Loading soundcloud.client_id from cache"). Unverified here: that the pinned `yt-dlp` plays these in this app's decoder, since HLS is not a single file and `stream.rs` reads one ranged HTTPS file. The design needs a spike.
3. Official API. SoundCloud's developer page says credentials need an Artist Pro subscription, so a project with no server and many users cannot hold one shared key. A per-user key is unusual. Unverified: whether the API terms allow a third-party desktop player. Candidate: do not use the official API.
4. Formats today. SoundCloud's developer blog (version "Deadline Extended") plans to drop progressive MP3, HLS MP3 and HLS Opus on 2025-12-31 and keep HLS AAC at 160 and 96 kbps (the blog gives the plan, not a confirmed result). Unverified: what the public web serves today. The badge reads the real stream, so it needs no fixed list.
5. Hosts, from `yt-dlp` and the SoundCloud blog, all unverified for the final list: `soundcloud.com`, `api-v2.soundcloud.com`, `*.sndcdn.com` (media and artwork; named in the blog's example URLs as `cf-media.sndcdn.com` and `cf-hls-media.sndcdn.com`) and `playback.media-streaming.soundcloud.cloud`. A network capture must settle the list. The owner approves it.
6. Structure. `structure.md` has no crate for a second service and `nemo-core` has a `Source` of Local or Ytm only. Is the new code a new crate (candidate `nemo-soundcloud`) and does `Source` gain a variant? The `Catalog` trait in `nemo-core/src/catalog.rs` is built for YouTube Music pages; does SoundCloud implement it or get its own trait? `structure.md` dependency rules need an edit.
7. The `yt-dlp` resolver today is anonymous (`--ignore-config`, no cookies) and picks `bestaudio` by `abr`. A 64 kbps Opus stream can sound better than 128 kbps MP3. Which stream does the app choose?
8. Name and wording of the risk text. The owner has not approved the text.

<!-- Sources:
.kiro/steering/product.md (Scope; Risks), .kiro/steering/coc.md (Honest Context; Owner Decisions 2026-11-14), .kiro/steering/structure.md
crates/nemo-core/src/catalog.rs, crates/nemo-ytm/src/ytdlp.rs, stream.rs
https://raw.githubusercontent.com/yt-dlp/yt-dlp/master/yt_dlp/extractor/soundcloud.py (format list, api-v2 base URL)
https://github.com/yt-dlp/yt-dlp/issues/17335 (encrypted HLS tracks give a DRM error; yt-dlp nightly 2026.07.23 log)
https://developers.soundcloud.com/blog/api-streaming-urls (format plan, example hosts)
https://developers.soundcloud.com/docs/api/register-app (Artist Pro needed for API credentials)
-->
