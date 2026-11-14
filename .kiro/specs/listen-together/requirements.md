## Goal

The user can start or join a shared room and hear the same song at the same position as friends, including friends on Metrolist. The host controls playback. The room service is a third-party server the user chooses by typing its address (a self-hosted metroserver or any compatible one). NemoMusic ships with no default server. The project runs no server. Listen Together is off until the user turns it on, and it says what other members and the server see before the first room.

## User Stories

- As a listener, I want to create a room and share a code, so that friends can listen with me.
- As a listener, I want to join a friend's room with a code, so that we hear the same song together.
- As a Metrolist user's friend, I want to join their room from NemoMusic, so that we need not use the same app.
- As a privacy-minded user, I want to know what the room and the server see, so that I can decide.
- As a user, I want to leave a room cleanly, so that my player returns to normal.

## Acceptance Criteria

1. WHEN the app is installed or updated, THE SYSTEM SHALL keep Listen Together off, and SHALL NOT open any connection to a room server.
2. WHEN the user turns Listen Together on, THE SYSTEM SHALL first show the server address the user typed, what it receives (display name, room code, track ids and titles, play state and position, IP address) and the risk (unofficial, no service terms known). The switch stays off until the user confirms.
3. THE SYSTEM SHALL ship with no server address. THE SYSTEM SHALL ask the user to type one: a `wss://` address, or a `ws://` address only when it points to this computer or a private network. THE SYSTEM SHALL label it "the server you set", and SHALL NOT open a connection until the user confirms. A public server becomes a listed choice only after its operator has agreed to NemoMusic using it.
4. WHEN the user creates a room, THE SYSTEM SHALL show the room code, a Copy button and a Leave button, and the user becomes host.
5. WHEN the user joins with a room code, THE SYSTEM SHALL ask for a display name, and SHALL NOT use the Google account name or email as a default.
6. IF the room code is wrong or the room is full, THEN THE SYSTEM SHALL show the server's reason and stay out of any room.
7. WHILE the host has not approved a join request, THE SYSTEM SHALL show "Waiting for the host" with a Cancel button.
8. WHEN a guest asks to join, THE SYSTEM SHALL show the host the name and Approve and Reject buttons, unless the host turned auto-approve on. Auto-approve is off by default.
9. WHILE in a room, THE SYSTEM SHALL let only the host change the track, play, pause, seek and the queue order.
10. WHEN the host plays, pauses, seeks or changes the track, THE SYSTEM SHALL apply the same action on every member's player.
11. WHEN a guest joins, THE SYSTEM SHALL start the room's current track at the room's current position.
12. WHEN a guest pauses locally, THE SYSTEM SHALL let the room play on, and WHEN the guest presses play, THE SYSTEM SHALL rejoin the room at its current position.
13. WHILE in a room, THE SYSTEM SHALL measure each member's offset from the room's position and SHALL correct drift above a set threshold by a seek. The design sets the threshold and checks it with a test.
14. WHILE in a room, THE SYSTEM SHALL correct drift only by a seek, and SHALL NOT stretch or resample the audio to catch up.
15. WHILE in a room, THE SYSTEM SHALL suspend crossfade and any other effect that overlaps two tracks. WHEN the user leaves, THE SYSTEM SHALL restore the setting unchanged.
16. WHILE in a room, THE SYSTEM SHALL keep the quality badge and signal-path view true to what plays on this computer.
17. THE SYSTEM SHALL share only the track id, title, artist, album, duration and cover link of a YouTube Music song. THE SYSTEM SHALL NOT share lyrics, local file names, local file tags, library data, the Google account, cookies or stream URLs.
18. WHEN the current track is a local file or a SoundCloud track, THE SYSTEM SHALL NOT send it to the room, and SHALL tell the host the room cannot play it.
19. WHILE in a room, THE SYSTEM SHALL show each member only as a display name with a host mark and a connected state.
20. WHEN the host removes a member, THE SYSTEM SHALL end that member's session with a short reason, and the host may block the display name on this computer.
21. WHEN the server hands the host role to another member (metroserver does so after the host is gone for 30 seconds), THE SYSTEM SHALL tell every member who the new host is, and SHALL update who may control playback.
22. WHEN the user leaves a room, THE SYSTEM SHALL close the connection, forget the session token, stop following the room and keep the local queue.
23. WHEN the user turns the feature off or signs out, THE SYSTEM SHALL leave the room and close the connection.
24. IF the connection drops, THEN THE SYSTEM SHALL keep playing the current song locally, retry with growing delays up to a fixed number of attempts, and show the state. IF all attempts fail, THEN THE SYSTEM SHALL leave the room and say so.
25. IF the server is unreachable, THEN THE SYSTEM SHALL show a short message with the host name and SHALL NOT retry in a tight loop.
26. THE SYSTEM SHALL send "NemoMusic/<version>" as the User-Agent. IF the server refuses this client or limits what it may do, THEN THE SYSTEM SHALL show the server's message as received and SHALL NOT change its client name to hide what it is.
27. THE SYSTEM SHALL show a dot or label on the player bar while a room is live, and every Listen Together action SHALL work with the keyboard and have a visible focus.
28. WHILE reduced motion is on, THE SYSTEM SHALL show member joins and leaves without movement.
29. WHEN the change is merged, THE README egress list SHALL say that the room server is the one the user types and that no default exists, and SHALL name a server host only once it is a listed choice. The settings screen SHALL show the address in use.
30. WHILE Cast is playing, THE SYSTEM SHALL NOT start or join a room, and WHILE a room is live, THE SYSTEM SHALL NOT start casting. THE SYSTEM SHALL say which one is active.
31. THE SYSTEM SHALL speak the protocol of the published `listentogether.proto` (protobuf envelope, optional gzip, WebSocket) and SHALL ignore messages and features it does not know.

## Out of Scope

- Running a room server (the project runs none). Documenting how to run one is allowed.
- Voice, text chat and reactions.
- Rooms that play local files or SoundCloud tracks.
- Sharing the account, library or history with members.
- Track suggestions from guests, and queue editing by guests. They are Later, unless the design finds them trivial.
- Copying code from Metrolist or metroserver. The client is written from the published `.proto`.
- A built-in default server.
- Android notification actions.

## Open Questions

Decided (owner, 2026-11-14): no default server; the user enters the address (criterion 3). The public Metrolist server, "The Meowery" (`wss://metroserverx.meowery.eu/ws`, run by Nyx), becomes a listed choice only after its operator agrees. The client is written from the published `listentogether.proto`. NemoMusic sends its real name as the User-Agent (criterion 26). Guest suggestions and queue editing are Later. The display-name blocklist is a convenience, not security.

1. Who asks the operator of The Meowery whether NemoMusic is welcome, and with which text? The owner, with the User-Agent string from criterion 26.
2. Whether a leave fully removes the user's data on the server. Unverified; the settings text says so.
3. Does a track id in a room equal a YouTube video id, and does every member need a working YouTube Music stream? The design checks it against the `.proto` and a self-hosted metroserver.
4. Which protocol features NemoMusic asks for at launch. The design settles this.

<!-- Sources:
Metrolist org (metroserver, metroproto, KMP app private): https://github.com/MetrolistGroup
metroserver README (features, UA policy, host handover, Docker, GPL-3.0): https://github.com/MetrolistGroup/metroserver
Protocol file: https://raw.githubusercontent.com/MetrolistGroup/metroproto/master/listentogether.proto
Default server and client constants (reconnect attempts, ping, User-Agent): https://github.com/MetrolistGroup/Metrolist/blob/main/app/src/main/kotlin/com/metrolist/music/listentogether/ListenTogetherServers.kt and .../ListenTogetherClient.kt
Licences via GitHub API: https://api.github.com/repos/MetrolistGroup/metroserver, https://api.github.com/repos/MetrolistGroup/metroproto
Behaviour of another client with the same rooms (host control, auto-approve, blocklist, crossfade suspended): https://www.simpmusic.org/docs/guide/listen-together
Repo steering: .kiro/steering/product.md (Local only, Approved additions), .kiro/steering/coc.md (User Privacy)
-->
