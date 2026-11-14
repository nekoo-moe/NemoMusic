## Goal

NemoMusic can suggest songs and write a short reading of the user's music taste, using an AI provider the user sets up. The provider is an endpoint the user chooses (an OpenAI-compatible API or a local model server) with the user's own key. Nothing is sent until the user turns the feature on and has seen exactly what is sent and to which host. With no provider set, no AI feature appears anywhere. This spec also defines the provider settings that the lyrics-plus spec uses for lyrics translation.

## User Stories

- As a listener, I want song suggestions based on songs I pick or play, so that I find music I may like.
- As a listener, I want a short reading of my taste, so that I can see patterns in what I play.
- As a privacy-minded listener, I want to use a model on my own computer, so that nothing leaves it.
- As a user of a cloud model, I want to see the exact text sent and the host, so that I know what I share.
- As a listener who does not want AI, I want to see no AI buttons at all, so that the app stays calm.

## Acceptance Criteria

### Provider settings

1. WHEN no AI provider is saved, THE SYSTEM SHALL hide every AI feature, button and menu entry, including the AI translation toggle in the lyrics view. Settings SHALL show one "AI provider" entry to set it up, and Settings > Lyrics SHALL show the translation entry as unavailable with the text "Set up an AI endpoint first" (lyrics-plus criterion 38).
2. WHEN the user sets up a provider, THE SYSTEM SHALL ask for an endpoint URL, a model name and an API key (the key is optional for a local server).
3. THE SYSTEM SHALL talk to the endpoint with the OpenAI-style chat completions request shape (`/v1/chat/completions`, as local servers such as Ollama accept it).
4. THE SYSTEM SHALL store the API key in the OS credential store only, and SHALL NOT write it to a file, a log, an error message or the clipboard.
5. THE SYSTEM SHALL require HTTPS for an endpoint. Plain HTTP SHALL be allowed for a loopback address (`localhost`, `127.0.0.1`, `::1`), and for a private LAN address written as an IP number (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16, `fc00::/7`, `fe80::/10`), where THE SYSTEM SHALL show a warning that the text crosses the local network unencrypted and SHALL ask the user to confirm before it saves the provider. A host name other than `localhost` SHALL need HTTPS.
6. WHEN the user saves a provider, THE SYSTEM SHALL show the host name it will send data to, and a short text: what you send leaves this computer for that host, and its owner's terms apply.
7. WHEN the user chooses "Test connection", THE SYSTEM SHALL send a fixed test message with no user data and show success or the short error.
8. WHEN the user chooses "Remove provider", THE SYSTEM SHALL delete the endpoint, the model name and the key, and every AI feature SHALL disappear again.
9. THE SYSTEM SHALL NOT ship with a default endpoint, a preset provider list, a default key or a hosted proxy. The user may set any endpoint. The project runs no server.
10. WHEN the user signs out of YouTube Music, THE SYSTEM SHALL keep the provider settings, because they are not account data. AI results derived from account data (for example a taste reading) SHALL be deleted.

### Consent and disclosure

11. WHEN the provider is saved, THE SYSTEM SHALL keep each AI feature off until the user turns that feature on in a dialog that lists what it sends.
12. WHEN the user runs an AI action, THE SYSTEM SHALL show the exact text to be sent and the host, and SHALL send it only after the user confirms. A switch "Do not ask again for this feature" MAY be offered, and it is off by default.
13. THE SYSTEM SHALL send only song titles and artists (and, where the feature needs it, album, play count and the user's own choice of song). It SHALL NOT send cookies, tokens, account ids, e-mail, file paths, links or device ids.
14. THE SYSTEM SHALL send at most 50 songs per request.
15. THE SYSTEM SHALL send data only in response to an action by the user, and SHALL NOT call the provider in the background.

### Song suggestions

16. WHEN the user picks "Suggest songs" from a song, album, playlist or the queue, THE SYSTEM SHALL send up to 50 of those songs' titles and artists and ask for song suggestions.
17. WHEN the answer arrives, THE SYSTEM SHALL show the suggestions as a list labelled "AI suggestions, may be wrong", and SHALL look up each one in the normal catalog search.
18. IF a suggested song is not found, THEN THE SYSTEM SHALL show it as not found and SHALL NOT invent a match.
19. THE SYSTEM SHALL NOT add suggestions to the queue or a playlist until the user chooses to.

### Taste reading

20. WHEN the user picks "Read my taste", THE SYSTEM SHALL offer the sources to send (top artists and play counts from the listening-stats history while it is on, liked songs, or a chosen playlist) and SHALL show the exact list in the payload preview before sending.
21. WHERE the listening-stats recording is off or the history is empty, THE SYSTEM SHALL offer only the other sources and SHALL NOT read the history.
22. WHEN the reading arrives, THE SYSTEM SHALL show it as text labelled with the model name and the host, and save it on this computer only until the user deletes it.

### Errors and offline

23. IF the endpoint is unreachable, the key is rejected, the model is missing or the answer is empty, THEN THE SYSTEM SHALL show one short message and SHALL keep playing.
24. WHILE the app is offline and the endpoint is not on this computer, THE SYSTEM SHALL disable AI actions and say why.
25. IF the answer is not in the expected form, THEN THE SYSTEM SHALL show nothing from it except a message that the answer could not be used.
26. THE SYSTEM SHALL NOT log prompts, answers or keys.
27. THE SYSTEM SHALL make every AI control reachable by keyboard with a visible focus. Any loading animation SHALL stop when reduced motion is on.
28. THE SYSTEM SHALL list `the endpoint you set` on the README egress list for this feature, and SHALL NOT add a fixed host.

## Out of Scope

- Hosting, reselling or bundling a model, a key or a proxy.
- Chat with an AI, voice control, and AI-made music or covers.
- Sending audio, covers, lyrics or the microphone recording to the provider in this spec. Lyrics translation is defined in the lyrics-plus spec and uses only the provider settings defined here.
- Training or fine-tuning on the user's history.
- Native APIs that are not OpenAI-compatible (for example a Claude or Gemini native API).

## Open Questions

Decided by the owner on 2026-11-14:
- The user may set any endpoint. There are no preset providers (criterion 9), and "the endpoint you set" is enough for the README egress list (criterion 28).
- HTTPS is required, except plain HTTP for loopback, and for private LAN addresses with a warning (criterion 5).
- The taste reading may use listening-stats data (top artists and counts) while stats are on, and shows it in the payload preview before sending (criteria 20 and 21).
- Lyrics and AI: `coc.md` (Owner Decisions, 2026-11-14) lets the lyrics of the song on screen go to the user's endpoint for translation, only after the user turns it on and reads a warning. No other feature sends lyrics. The lyrics-plus spec owns that flow and uses the provider settings here.

The design settles the API key lookup, model list discovery, request size and timeout limits, and how song titles in an answer are kept from acting as instructions (titles are data).
<!-- Sources:
- .kiro/steering/product.md (Approved additions: ai-features, lyrics-plus), coc.md (User Privacy; Owner Decisions 2026-11-14; lyrics rule)
- https://docs.ollama.com/api/openai-compatibility (OpenAI-compatible /v1/chat/completions on a local server, key required but ignored)
- crates/nemo-ytm/src/credentials.rs (credential store in use)
-->
