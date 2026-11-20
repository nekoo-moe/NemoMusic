## Goal

When the user has switched Discord presence on, the Discord app on the same computer shows what NemoMusic is doing in every state: a song playing with a live progress bar, a paused song, idle, browsing, away, a video, a song on a cast device or in a Listen Together room, and a private form that shows no song. It is built on the integrations spec (criteria 22 to 28, which stay as they are); this spec adds every state and the worker that sends them. Nothing is sent while the switch is off, and nothing leaves the computer except through the local Discord app.

Owner request (2026-11-19, translated): "Add one agent to add ALL of Discord RPC to the app: every state, idle, listening, sleeping, etc." The Discord application id `1571523010022735972` is built in (`DISCORD_APPLICATION_ID`); it is public.

## User Stories

- As a listener, I want my Discord profile to show the song I play with a progress bar, so that friends see what I hear right now.
- As a listener, I want it to say `Paused`, `Idle` or `Away` when that is true, so that my profile never shows a song I am not hearing.
- As a private person, I want a switch that hides the song name, artist, cover and times, so that friends only see that I use NemoMusic.
- As a careful user, I want the app never to upload a picture, never to send a file path, a room name, a device name or an address, and never to log what I play, so that turning this on costs me no more privacy than the text says.
- As a user without Discord running, I want the app to say nothing and keep trying quietly, so that I get no error and no slowdown.

## Acceptance Criteria

States
1. WHILE a song plays, THE SYSTEM SHALL show the type `Listening`, the title as the first line and the artist names as the second line, and a start and an end time (from the position and the length) so that Discord draws a progress bar; WHEN the length is unknown (a live stream), THE SYSTEM SHALL send the start time only.
2. WHEN the position jumps (a seek) or a new song starts, THE SYSTEM SHALL send fresh times at the next update slot (criterion 14).
3. WHILE a song is paused, THE SYSTEM SHALL show the title and `Paused` with the artist, with the small image `paused`, and no start or end time.
4. WHILE the app is open and nothing plays (stopped, empty queue), THE SYSTEM SHALL show `Idle` and `Nothing playing`, with the time since the app became idle as the start time.
5. WHILE nothing plays and the user is in the Library or in Search, THE SYSTEM SHALL show `Browsing Library` or `Searching`, with `Nothing playing`. It SHALL NOT send what the user typed or which page, playlist or artist is open.
6. WHEN nothing has played and the window has been unfocused or minimized for 10 minutes without a break (the sleeping rule), THE SYSTEM SHALL show `Away` and `Sleeping` with no times; WHEN the window gets the focus or a song plays, THE SYSTEM SHALL leave this state at once (bypassing the 15 s slot once). A paused song counts as nothing playing for this rule. The rule reads the window only: no OS idle or lock state is read.
7. WHILE a sleep timer runs and a song plays or is paused, THE SYSTEM SHALL add `Sleep timer on` to the tooltip of the large image. It SHALL NOT show the time left.
8. WHILE playback is on a Cast device or in a Listen Together room, THE SYSTEM SHALL add `Listening on a cast device` or `Listening together` to the tooltip of the large image, and SHALL NOT send a room name, a room code, a device name or an address.
9. WHILE a video plays, THE SYSTEM SHALL show the type `Watching`, the title as the first line and `Watching a video` as the second line, with the same times and the same privacy rules as a song. (The app has no video player yet, so no code feeds this state today; the builder and its test exist so the player only has to set the context.)
10. WHILE `Hide song names on Discord` is on, THE SYSTEM SHALL show only `Listening to NemoMusic` for every state that holds a title (song, paused song, video): no title, artist, cover, source, times, device, room or sleep timer. Idle, Browsing and Away hold no title and look the same as without the switch.

Pictures
11. THE SYSTEM SHALL use as the large image only a cover address the app already has (a YouTube Music or SoundCloud thumbnail) that is `https`, on a host of the cover hosts already in the README egress list (`i.ytimg.com`, `*.ggpht.com`, `*.googleusercontent.com`, `*.sndcdn.com`) and at most 256 bytes; otherwise the asset `nemo`. It SHALL NOT upload, proxy or fetch a picture, and SHALL NOT send a local file path.
12. THE SYSTEM SHALL show the small image `youtube-music`, `soundcloud` or `local` with the tooltip `YouTube Music`, `SoundCloud` or `Local file`, and for a local file of a lossless format (FLAC, WAV, AIFF, APE, WavPack by file extension) `Local file, FLAC` (and so on). It SHALL NOT say `lossless` for YouTube or SoundCloud. It SHALL send no button and no link.

Worker and limits
13. WHILE the switch is off, THE SYSTEM SHALL start no thread, open no socket or pipe and do no work for Discord. WHEN it is switched off or the app exits, THE SYSTEM SHALL clear the presence, close the connection and end the thread.
14. THE SYSTEM SHALL send at most one update per 15 seconds, the newest state winning; the first update after a quiet time and the first after a connect SHALL be sent at once; an update equal to the last one sent SHALL NOT be sent again.
15. THE SYSTEM SHALL cut text to at most 128 bytes on a character boundary (ending in `…`) and pad text of fewer than 2 bytes with spaces, so Discord never rejects a line.
16. IF the Discord app is not running or the connection breaks, THEN THE SYSTEM SHALL retry every 30 seconds while the switch is on, with one debug line for each change of the connected state, no dialog and no effect on playback or on the UI.
17. THE SYSTEM SHALL NOT block the play hub, the UI or the audio thread: events go into a bounded mailbox that drops the oldest entry when full.
18. THE SYSTEM SHALL log no title, artist, album, cover address, id, room or device name. It SHALL emit one fixed `nemo_trace` debug event per connect, disconnect and activity change (`provider=discord`, `op=connect|set_activity|clear`, `status=ok|err`) without text.
19. THE SYSTEM SHALL talk to the local Discord app only (a Unix socket or a Windows named pipe), so the README egress list does not change.

Settings
20. WHEN the user opens Settings, Privacy, THE SYSTEM SHALL show the Discord presence row (off by default, text of criterion 22 of the integrations spec updated: what is sent is the title, artist, source, cover address and position of the song, to the Discord app on this computer, which shows it to friends and servers by the user's Discord privacy settings) and, under it, the switch `Hide song names on Discord` (off by default).
21. THE SYSTEM SHALL keep the switch and its confirmed notice version in `integrations.sqlite3`; the switch counts only while the stored confirmed version equals the version of the notice. Sign-out wipes nothing (integrations criterion 20).

## Out of Scope

- Buttons, join or listen-along invites, party size, and any link to the user's account or a track page.
- Uploading covers, proxying pictures, or any request by the app to Discord's servers (the Discord Social SDK and the Discord web API).
- Reading the OS idle or lock state (no cheap, safe way on both systems; the window rule is used).
- Showing the sleep timer's remaining time, the queue, lyrics, or the page the user is on beyond `Library` and `Search`.

## Open Questions

- Discord's local IPC update limit is unverified; the app keeps to 15 s per update. The owner can test it with a real client.
- The owner uploads the art assets named in design.md in the Discord developer portal; until then Discord shows no large picture for the `nemo` fallback and no small pictures, and the text lines still read well.
- The cover address of a YouTube or SoundCloud song is fetched by Discord's servers, not by the app. The notice says so.
