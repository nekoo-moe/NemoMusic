## Goal

The user opens a device picker, sees Google Cast (Chromecast, Nest, Google TV) devices and DLNA/UPnP renderers on the local network, and plays the current song on one of them. NemoMusic stays the remote: it shows progress and offers play, pause, seek, volume and stop. The device decodes the stream itself, so the path is never bit-perfect and the app says "Casting" instead. Cast is off until the user turns it on, and it contacts nothing on the network until the picker is open.

## User Stories

- As a listener, I want to play my music on a speaker or TV, so that I can hear it across the room.
- As a listener, I want to control playback from NemoMusic, so that I need not walk to the device.
- As a local-library user, I want my own files to play on the device, so that I am not limited to streams.
- As a privacy-minded user, I want no network scanning unless I ask, so that the app stays quiet on my network.
- As an audiophile, I want the app to say plainly that casting is not bit-perfect, so that the badge never misleads me.

## Acceptance Criteria

1. WHEN the app is installed or updated, THE SYSTEM SHALL keep Cast off and SHALL send no discovery packet.
2. WHEN the user turns Cast on, THE SYSTEM SHALL first say what it does: it searches the local network when the picker is open, and it serves the chosen song, or passes the stream that is playing, to the device over the local network. THE SYSTEM SHALL also say that Windows may ask to allow NemoMusic through the firewall on the first cast. The switch stays off until the user confirms.
3. WHILE the device picker is closed, THE SYSTEM SHALL NOT run discovery or listen for devices.
4. WHEN the user opens the picker, THE SYSTEM SHALL start discovery of Cast devices and UPnP renderers, and WHEN the picker closes, THE SYSTEM SHALL stop it.
5. WHEN a device is found, THE SYSTEM SHALL list its name, its type (Cast or DLNA) and whether it is busy. THE SYSTEM SHALL list the computer's own speakers as "This computer".
6. IF no device is found, THEN THE SYSTEM SHALL say so, mention the local network and the firewall, and offer a retry.
7. WHEN the user picks a device, THE SYSTEM SHALL send the current song to it and show "Casting to <device name>" on the player bar and in the signal-path view.
8. WHILE casting, THE SYSTEM SHALL NOT show a bit-perfect badge. THE SYSTEM SHALL show "Casting", the codec and bitrate of the source when known, and "Decoded on the device" for the output.
9. WHILE casting, THE SYSTEM SHALL NOT show "Lossless", "HD" or "Hi-Res" for a YouTube or SoundCloud song. A local lossless file shows "Lossless source" only if the device plays the original file untouched. IF that is not known, THEN THE SYSTEM SHALL show "Unknown".
10. WHILE casting, THE SYSTEM SHALL show the song's position and length from the device's own reports, and let the user play, pause, seek, set volume, go to the next or previous song and stop.
11. WHEN the song ends on the device, THE SYSTEM SHALL send the next song in the queue, or stop if the queue is empty.
12. WHEN the user stops casting, THE SYSTEM SHALL stop the device's playback, stop serving files, close the connection and return playback to this computer on request.
13. WHILE casting, THE SYSTEM SHALL keep the local output idle, and SHALL NOT play the same song on both.
14. WHEN a local file is cast, THE SYSTEM SHALL serve it from a small HTTP server on this computer, bound to the local network interface only, never to the internet and never to all interfaces by default.
15. THE SYSTEM SHALL serve only the file of the song being cast, under an unguessable per-session path, and SHALL end the server when casting stops.
16. THE SYSTEM SHALL NOT serve library folders, lists of files, lyrics or account data. THE SYSTEM SHALL NOT cast a song from the offline cache, and SHALL say that it plays only inside NemoMusic.
17. IF the device cannot play the file's format, THEN THE SYSTEM SHALL say which format failed and SHALL NOT transcode unless the user turned that on. Transcoding shows in the signal path.
18. WHEN a YouTube or SoundCloud song is cast, THE SYSTEM SHALL pass the device the stream address when a test on real devices shows that the device can fetch it. OTHERWISE, or IF the device cannot fetch it, THE SYSTEM SHALL pass the playing stream through this computer to the device over the local network. THE SYSTEM SHALL NOT store the passed-through stream on disk, SHALL serve it only to the chosen device, and SHALL end it when casting stops.
19. THE SYSTEM SHALL NOT send cookies, tokens or account data to the device.
20. IF the network drops, the device goes away or the device reports an error, THEN THE SYSTEM SHALL show the reason, stop casting and offer to continue on this computer at the same position.
21. IF the device is already used by another sender, THEN THE SYSTEM SHALL ask before it takes over the device.
22. WHEN the user signs out, THE SYSTEM SHALL stop casting and end the file server.
23. THE SYSTEM SHALL work with the keyboard: a shortcut opens the picker, arrow keys move in the list, Enter selects, and focus is visible.
24. WHILE reduced motion is on, THE SYSTEM SHALL show the discovery state as text and a static indicator, not a spinner.
25. WHEN the change is merged, THE README SHALL explain the local-network traffic of Cast, the Windows firewall prompt and the rule to allow it. No internet host is added by this feature. The Cast and UPnP traffic stays on the local network.
26. THE SYSTEM SHALL answer the cross-origin requests of Cast receivers with the headers that playback needs, and only for the file or stream of the current session.
27. WHILE a room is live, THE SYSTEM SHALL NOT start casting, and WHILE casting, THE SYSTEM SHALL NOT start or join a room. THE SYSTEM SHALL say which one is active.
28. WHILE casting, THE SYSTEM SHALL set the volume on the device only, and SHALL NOT apply software volume on this computer.
29. WHILE casting, THE SYSTEM SHALL NOT claim gapless playback or show a gapless badge.
30. THE FIRST TASK of the feature SHALL be a spike on a real Chromecast and a real DLNA speaker. It SHALL record whether the device plays a `googlevideo.com` address it fetches itself, and which formats and headers work. The spike result decides which route criterion 18 uses first.

## Out of Scope

- AirPlay, Bluetooth and Spotify Connect.
- NemoMusic as a receiver (a device others cast to).
- Multi-room groups and synchronized playback on several devices.
- Video, subtitles and photos on the device.
- Casting through the internet or to devices on another network.
- Transcoding by default.
- A custom Cast receiver app, which needs a Google developer registration.
- Casting songs from the offline cache.
- Gapless playback on the device.
- Volume on this computer while casting.
- Windows "Play To" and a Linux DLNA push.

## Open Questions

Decided (owner, 2026-11-14): offline-cache songs are never cast (criterion 16). If `googlevideo.com` addresses do not play when the device fetches them, the stream passes through this computer over the local network, and `coc.md` allows it (criterion 18). The spike on real devices comes first (criterion 30). CORS and the Windows firewall prompt are covered by the design (criteria 2, 25 and 26). Device volume only (criterion 28). No gapless claim (criterion 29). No "Play To" (Out of Scope).

1. Which crates carry Cast and UPnP? The design checks licence, upkeep and MSRV against `tech.md`.
2. Which container and headers do the Default Media Receiver and DLNA speakers accept for a YouTube stream? The spike answers it.

<!-- Sources:
Google Cast supported media (containers, audio codecs, CORS note): https://developers.google.com/cast/docs/media
Cast Web Receiver core features: https://developers.google.com/cast/docs/web_receiver/core_features
UPnP AVTransport (SSDP search, SetAVTransportURI): https://github.com/sonydevworld/audio_control_api_examples/blob/master/DLNA/AVTransport/play_file.adoc
How DLNA casting works (the device fetches the URL itself): https://dev.to/gtoxlili/i-needed-the-stream-url-when-casting-video-so-i-built-a-fake-tv-4ag4
yt-dlp FAQ (same IP and headers): https://github.com/yt-dlp/yt-dlp/wiki/FAQ
Invidious note on the ip parameter: https://github.com/iv-org/invidious/issues/92
Crate versions and licences: https://crates.io/api/v1/crates/rust-cast, /mdns-sd, /rupnp, /ssdp-client
Repo steering: .kiro/steering/code.md (Bit-perfect), .kiro/steering/coc.md (Forbidden Features, offline cache), .kiro/steering/product.md (Quality badge rule)
-->
