## Goal

The user taps "Recognize", NemoMusic listens for a few seconds, and it shows which song is playing near them. One more tap finds that song in NemoMusic and plays it. The microphone is used only while recognition runs. Only a fingerprint of the sound, never a recording, leaves the computer. Recognized songs are kept in a history on this computer. The feature is off until the user turns it on.

## User Stories

- As a listener, I want to identify a song playing in the room, so that I can play it in NemoMusic.
- As a privacy-minded user, I want to know when the microphone is open and what is sent, so that I stay in control.
- As a listener, I want a list of songs I recognized, so that I can find them again later.
- As a listener, I want to identify a song playing in another app on this computer, so that I need no microphone for it.

## Acceptance Criteria

1. WHEN the app is installed or updated, THE SYSTEM SHALL keep song recognition off, and SHALL NOT open the microphone or contact the recognition service.
2. WHEN the user turns the feature on in settings, THE SYSTEM SHALL first show what is sent (an audio fingerprint, its length, the time of the request, the app name and version, and the IP address, never the recording), to whom (`amp.shazam.com`), and the risk (unofficial service, terms may be breached, software patents may apply outside the EU). The switch stays off until the user confirms.
3. WHEN the user starts recognition, THE SYSTEM SHALL show a visible "Listening" state with a Cancel button, and SHALL open the input device only from that moment.
4. WHEN recognition ends, fails or is cancelled, THE SYSTEM SHALL close the input device at once.
5. THE SYSTEM SHALL NOT write captured audio to disk, to logs or to memory beyond the capture window, and SHALL NOT send it anywhere.
6. WHEN a fingerprint is sent, THE SYSTEM SHALL send only the fingerprint and the minimum request fields the service needs, with no account data, cookies or user identifiers.
7. THE SYSTEM SHALL NOT send a made-up location, time zone or device identity. THE SYSTEM SHALL send the real time of the request and its real name and version as the User-Agent. IF the service answers only when a location or time zone is present, THEN THE SYSTEM SHALL keep recognition unavailable, say why, and send nothing false.
8. WHEN the service returns a match, THE SYSTEM SHALL show title, artist and the YouTube Music cover of the same song, and a "Find in NemoMusic" action. IF YouTube Music has no matching song, THEN THE SYSTEM SHALL show a placeholder instead of a cover.
9. WHEN the user chooses "Find in NemoMusic", THE SYSTEM SHALL search the YouTube Music catalog and the local library by title and artist, list the matches, and play the one the user picks. THE SYSTEM SHALL NOT play a match automatically.
10. IF no catalog entry matches, THEN THE SYSTEM SHALL say so and keep the recognized song in history.
11. IF the service finds no match, THEN THE SYSTEM SHALL say "No match" and offer to try again.
12. IF the computer is offline, the service is unreachable or rate limits the request (HTTP 429), THEN THE SYSTEM SHALL show a short message with the cause, and SHALL NOT retry in a loop.
13. IF no input device exists or the OS denies microphone access, THEN THE SYSTEM SHALL say so and point to the OS setting.
14. WHEN the user picks an input source, THE SYSTEM SHALL list microphones first, and SHALL offer "This computer's sound" only on systems where the OS can capture the output (loopback). WHEN the user picks "This computer's sound" for the first time, THE SYSTEM SHALL say that it listens to everything this computer plays until the result, and ask to confirm.
15. WHILE "This computer's sound" is used, THE SYSTEM SHALL show the same "Listening" state, and SHALL capture only until the result or Cancel.
16. WHEN a recognition succeeds, THE SYSTEM SHALL add an entry (title, artist, time, source: microphone or computer) to a history stored on this computer. An entry holds text only, no cover. THE SYSTEM SHALL keep at most 500 entries and drop the oldest.
17. WHEN the user opens the history, THE SYSTEM SHALL list entries newest first, and offer "Find in NemoMusic", delete one entry and clear all.
18. WHEN the user turns the feature off, THE SYSTEM SHALL stop any capture and keep the history until the user clears it.
19. WHEN the user signs out, THE SYSTEM SHALL keep the history, because it holds no account data. IF a later design adds account data to an entry, THEN THE SYSTEM SHALL delete that data on sign-out.
20. THE SYSTEM SHALL work with the keyboard: a shortcut starts and cancels recognition, and focus is visible on every control.
21. WHILE reduced motion is on, THE SYSTEM SHALL replace the listening animation with a static indicator and a text label.
22. WHEN playback is running, THE SYSTEM SHALL NOT change the audio output path, and the signal-path view SHALL stay accurate. Capture uses a separate input stream.
23. WHEN the change is merged, THE README egress list SHALL name the recognition host, and the settings screen SHALL show the same host.
24. WHILE listening, THE SYSTEM SHALL capture at most 12 seconds of audio in total, SHALL send at most 3 requests per run with at least 4 seconds between them, and SHALL NOT start a new run within 10 seconds of the end of the last one.
25. WHEN the user turns the feature on, THE SYSTEM SHALL NOT send any request until the user starts a recognition. The first task of the feature SHALL check whether the service answers without a location and time zone (criterion 7) and record the result before any other code ships.
26. THE SYSTEM SHALL NOT fetch any image from the recognition service or from a cover host of that service. WHEN a result shows a cover, THE SYSTEM SHALL take it from the YouTube Music search result for the matched title and artist (hosts the app already lists for covers), and SHALL say in the settings text of criterion 2 that the matched title and artist are searched on YouTube Music.

## Out of Scope

- Recognizing from an audio file the user picks.
- Humming or singing recognition.
- Uploading history, sharing it, or showing it to other people.
- Our own fingerprint database or server (the project runs no server).
- Recognition on a schedule or in the background.
- Scrobbling recognized songs (see `integrations`).
- Storing covers or recordings in the history.
- Sending a fake location, time zone or device identity to get an answer.

## Open Questions

Decided (owner, 2026-11-14): the host `amp.shazam.com` is approved. The owner accepts the patent and Terms of Service risk (`coc.md`). The design picks between a port of the algorithm and a crate. Loopback capture stays in, as opt-in. Capture length and attempt rate are set by the design (criterion 24). History keeps text only, 500 entries.

1. Terms notice text: no public terms for third-party clients were found. The owner approves the wording of the risk text shown before the switch turns on.
2. Cover host: decided (owner, 2026-11-14). The cover comes from YouTube Music; no recognition-service cover host is used or listed (criteria 8 and 26).
3. Does the service answer without a location and time zone? Unverified. The first task tests it (criterion 25). If it does not, the design raises it to the owner and the feature stays unavailable.

<!-- Sources:
SongRec README (privacy, GPL-3.0, speakers and microphone, legal note): https://github.com/marin-m/SongRec/blob/main/README.md
SongRec request code (host, path, body fields, HTTP 429): https://raw.githubusercontent.com/marin-m/SongRec/main/src/core/fingerprinting/communication.rs
SongRec licence via GitHub API: https://api.github.com/repos/marin-m/SongRec
Repo steering: .kiro/steering/product.md (Approved additions, Risks), .kiro/steering/coc.md (User Privacy)
-->
