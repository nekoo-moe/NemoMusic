# Developer tools: requirements

## Goal

A hidden developer panel inside the app shows what the providers (YouTube Music, SoundCloud, lyrics, SponsorBlock, `yt-dlp`, the stream resolver) did and why a call failed: a trace of provider requests, the redacted log, one card per provider, sign-in diagnostics and system facts, with one button that copies a redacted text report. The panel is invisible until the Settings > About version text is clicked 10 times. Everything it shows stays in memory on this computer, is redacted when it is captured, is never written to disk and is never sent anywhere. Copying the report is the user's own action.

Owner request (2026-11-19, Vietnamese, translated): "Add a dev window, hidden behind the text 'NemoMusic - Version 0.3.0': when the user clicks it 10 times, the dev window opens. Put the features a developer needs, like bug trace and so on, right into the app but hidden as I said, so it is easier for you to investigate provider errors."

## User Stories

- As the owner, I want to open a hidden panel in the running app, so that I can see why a provider call failed without a debugger.
- As the owner, I want a trace of provider requests (provider, operation, host, status, time, resolver tier), so that I can tell a network, rate-limit, sign-in or extractor failure apart.
- As the owner, I want errors that happened before I opened the panel to be there too, so that I do not have to reproduce them.
- As the owner, I want one button that copies a redacted report, so that I can paste it into a chat without leaking a token, cookie, URL or account name.
- As a listener, I want the panel to stay invisible and cost nothing noticeable, so that the app looks and behaves as before.

## Acceptance Criteria

1. WHEN the version text in Settings > About is clicked 10 times, each click less than 3 seconds after the previous one, THE SYSTEM SHALL open the developer panel and show the toast `Developer tools`; THE SYSTEM SHALL open it once per run of 10 clicks.
2. WHEN 3 seconds pass without a click, THE SYSTEM SHALL reset the click count to 0; a click anywhere else SHALL NOT reset it.
3. THE SYSTEM SHALL draw the About text, with its hover, cursor and tooltip, exactly as without this feature, and SHALL show no counter or hint.
4. WHILE the panel is unlocked in this run, THE SYSTEM SHALL toggle it with `Ctrl+Shift+D`; THE SYSTEM SHALL have no other entry point (no Settings row, no menu item) and SHALL NOT keep the unlocked state after the app closes.
5. THE SYSTEM SHALL keep a bounded in-memory ring of about 2000 events (oldest dropped first), filled from the first moment of the run, whether or not the panel was opened.
6. WHEN an event is captured, THE SYSTEM SHALL keep only its time, level, target, message and an allow-list of structured fields, and SHALL redact them before they enter the ring: URLs reduced to the host, query strings, bearer and cookie-like text, e-mail addresses, long id-like tokens and paths under the home folder (shown as `~`).
7. THE SYSTEM SHALL never write the ring or the report to a file, a log or the network; THE SYSTEM SHALL write only to the clipboard, and only when the user presses `Copy report`.
8. WHEN `NEMO_LOG` is `info` or unset, THE SYSTEM SHALL still put provider trace events (target `nemo_trace`) into the ring and SHALL keep the log file content unchanged.
9. THE SYSTEM SHALL emit a provider trace event at the HTTP layers of `nemo-ytm`, `nemo-soundcloud`, `nemo-lyrics` and `nemo-spotify` (host level only), at the stream resolver tiers and cache lookups, at `yt-dlp` runs, and at the sign-in stages; an event holds `provider`, `op`, `host`, `status`, `ms` and, where they apply, `bytes`, `tier`, `attempt` and `rest`, and never a URL, query, id, title, token, cookie or user name.
10. THE SYSTEM SHALL emit no trace event from the audio thread and SHALL NOT change what any provider call does.
11. WHEN the panel is open, THE SYSTEM SHALL offer the tabs `Trace`, `Log`, `Providers`, `Account` and `System`, as sliding chips.
12. THE `Trace` tab SHALL list provider requests newest first (time, provider, op, host, status, ms, tier), filter by provider and by `Errors only`, and show the fields of a selected row.
13. THE `Log` tab SHALL list redacted log lines, filter by level (error to trace) and by a text filter, and have a pause switch that freezes the list.
14. THE `Providers` tab SHALL show one card per provider with the last success time, the last error kind, the error count, p50 and p95 ms of the last 50 calls and the rest or backoff state, plus the resolver tier counts, the in-process solver state with its auto-off marker, the stream cache size and hit rate, and the `yt-dlp` version and path kind (managed, user, PATH) without the full path.
15. THE `Account` tab SHALL show, for YouTube Music and SoundCloud, whether the account is signed in, whether a token is present (yes or no), the token length class, the cookie names present, and the last sign-in stage and outcome, and SHALL show no token, cookie or account value.
16. THE `System` tab SHALL show the version, OS, build profile, the config keys that are set (values only for non-secret keys), the egress hosts contacted this session with counts, the uptime, the audio device and format, and frame time statistics.
17. WHEN a fact has no status call to read it from, THE SYSTEM SHALL show `Unknown`.
18. WHEN `Copy report` is pressed, THE SYSTEM SHALL put one plain-text report of all tabs, redacted again, on the clipboard and show `Copied` once; WHEN `Clear` is pressed, THE SYSTEM SHALL empty the ring.
19. THE SYSTEM SHALL keep the panel read-only: it has no action that changes app state except Clear, Copy and the toggles inside the panel.
20. THE SYSTEM SHALL draw the panel as an in-app overlay with a scrim, opaque `surface-container-high`, `shape.xl`, tabular numerals through `font.mono-num`, rows 32 to 40 px, scrolling inside, usable at 960 x 600 and 200% scale, closed by `Esc`, reachable by keyboard with a visible focus ring, and honouring Reduce motion.
21. WHILE the panel is closed, THE SYSTEM SHALL request no frame and run no timer for it; WHILE it is open and focused, THE SYSTEM SHALL redraw only when its data changes or on a 1 s tick.
22. THE SYSTEM SHALL carry data from other crates to the panel through the read-only `nemo-core` `DevProbe` contract whose types hold plain strings, numbers and enums only.

## Out of Scope

- A second operating-system window (the app uses one `iced::application` window; `iced::daemon` is not adopted).
- Writing traces to disk, uploading them, or any remote or automatic sharing.
- Changing provider behaviour, retry rules, limits or what the app fetches.
- Capturing payloads, bodies, headers, titles, ids or user names.
- Actions that change state (re-run a call, reset a limiter, sign out).
- Capture or panels for audio-thread internals.

## Open Questions

- Paths under the home folder keep their tail after `~` (for example `~/Music/...`). A library file name may then show in a copied report. Assumed acceptable because the owner copies the report on purpose; the report text says nothing else about the library.
- Trace events carry no song or playlist id at all, so a failing call cannot be tied to one song in the panel. Assumed enough for provider errors; a per-call correlation number could be added later.
