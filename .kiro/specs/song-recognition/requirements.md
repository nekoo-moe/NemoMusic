## Goal

The user taps "Recognize", NemoMusic listens for a few seconds, and it shows which song is playing near them. One more tap finds that song in NemoMusic and plays it. The microphone is used only while recognition runs. Only a fingerprint of the sound, never a recording, leaves the computer. Recognized songs are kept in a history on this computer. The feature is off until the user turns it on.

## User Stories

- As a listener, I want to identify a song playing in the room, so that I can play it in NemoMusic.
- As a privacy-minded user, I want to know when the microphone is open and what is sent, so that I stay in control.
- As a listener, I want a list of songs I recognized, so that I can find them again later.
- As a listener, I want to identify a song playing in another app on this computer, so that I need no microphone for it.

## Acceptance Criteria

1. WHEN the app is installed or updated, THE SYSTEM SHALL keep song recognition off, and SHALL NOT open the microphone or contact the recognition service.
2. WHEN the user turns the feature on in settings, THE SYSTEM SHALL first show what is sent (an audio fingerprint, the time of day and the app version, never the recording), to whom (the recognition host), and the risk (unofficial service, terms may be breached). The switch stays off until the user confirms.
3. WHEN the user starts recognition, THE SYSTEM SHALL show a visible "Listening" state with a Cancel button, and SHALL open the input device only from that moment.
4. WHEN recognition ends, fails or is cancelled, THE SYSTEM SHALL close the input device at once.
5. THE SYSTEM SHALL NOT write captured audio to disk, to logs or to memory beyond the capture window, and SHALL NOT send it anywhere.
6. WHEN a fingerprint is sent, THE SYSTEM SHALL send only the fingerprint and the minimum request fields the service needs, with no account data, cookies or user identifiers.
7. THE SYSTEM SHALL NOT send a made-up location or device identity. Open Question 3 covers the request fields.
8. WHEN the service returns a match, THE SYSTEM SHALL show title, artist and cover, and a "Find in NemoMusic" action.
9. WHEN the user chooses "Find in NemoMusic", THE SYSTEM SHALL search the YouTube Music catalog and the local library by title and artist, list the matches, and play the one the user picks. THE SYSTEM SHALL NOT play a match automatically.
10. IF no catalog entry matches, THEN THE SYSTEM SHALL say so and keep the recognized song in history.
11. IF the service finds no match, THEN THE SYSTEM SHALL say "No match" and offer to try again.
12. IF the computer is offline, the service is unreachable or rate limits the request (HTTP 429), THEN THE SYSTEM SHALL show a short message with the cause, and SHALL NOT retry in a loop.
13. IF no input device exists or the OS denies microphone access, THEN THE SYSTEM SHALL say so and point to the OS setting.
14. WHEN the user picks an input source, THE SYSTEM SHALL list microphones, and SHALL offer "This computer's sound" only on systems where the OS can capture the output (loopback).
15. WHILE "This computer's sound" is used, THE SYSTEM SHALL show the same "Listening" state, and SHALL capture only until the result or Cancel.
16. WHEN a recognition succeeds, THE SYSTEM SHALL add an entry (title, artist, time, source: microphone or computer) to a history stored on this computer.
17. WHEN the user opens the history, THE SYSTEM SHALL list entries newest first, and offer "Find in NemoMusic", delete one entry and clear all.
18. WHEN the user turns the feature off, THE SYSTEM SHALL stop any capture and keep the history until the user clears it.
19. WHEN the user signs out, THE SYSTEM SHALL keep the history, because it holds no account data. IF a later design adds account data to an entry, THEN THE SYSTEM SHALL delete that data on sign-out.
20. THE SYSTEM SHALL work with the keyboard: a shortcut starts and cancels recognition, and focus is visible on every control.
21. WHILE reduced motion is on, THE SYSTEM SHALL replace the listening animation with a static indicator and a text label.
22. WHEN playback is running, THE SYSTEM SHALL NOT change the audio output path, and the signal-path view SHALL stay accurate. Capture uses a separate input stream.
23. WHEN the change is merged, THE README egress list SHALL name the recognition host, and the settings screen SHALL show the same host.

## Out of Scope

- Recognizing from an audio file the user picks.
- Humming or singing recognition.
- Uploading history, sharing it, or showing it to other people.
- Our own fingerprint database or server (the project runs no server).
- Recognition on a schedule or in the background.
- Scrobbling recognized songs (see `integrations`).

## Open Questions

1. Which host? SongRec sends a POST to `amp.shazam.com` (path `/discovery/v5/en/US/android/-/tag/<uuid>/<uuid>`). This is an unofficial endpoint, not a public API, and it can change or block clients. Approve the host `amp.shazam.com` for the README egress list?
2. Algorithm and code: SongRec is GPL-3.0, like this repo (verified). Its README says it is built for interoperability and warns that software patents may apply outside the EU. Does the owner accept the patent note? Candidates: port the algorithm, or depend on a crate. A crate's licence and upkeep are unverified.
3. What SongRec sends beyond the fingerprint: a fixed fake geolocation (latitude 45, longitude 2), a fixed time zone ("Europe/Paris"), a random user agent, and random UUIDs in the path. NemoMusic should send no false location. Does the service still answer without those fields? Unverified, the design must test it.
4. Loopback: in scope as written (criteria 14 and 15). It needs WASAPI loopback on Windows and a monitor source on Linux (PipeWire or PulseAudio). Keep it in this wave, or defer?
5. What does "Terms" say for the service? No public terms for third-party clients were found. Treated as unverified, covered by the owner's 2026-11-14 risk decision. The notice text still needs the owner.
6. Capture length and number of attempts per run (SongRec listens in short growing windows). The design decides, within a human-paced request rate.
7. History size limit, and whether entries store a cover URL (a request to a cover host) or only text.

<!-- Sources:
SongRec README (privacy, GPL-3.0, speakers and microphone, legal note): https://github.com/marin-m/SongRec/blob/main/README.md
SongRec request code (host, path, body fields, HTTP 429): https://raw.githubusercontent.com/marin-m/SongRec/main/src/core/fingerprinting/communication.rs
SongRec licence via GitHub API: https://api.github.com/repos/marin-m/SongRec
Repo steering: .kiro/steering/product.md (Approved additions, Risks), .kiro/steering/coc.md (User Privacy)
-->
