## Goal

NemoMusic keeps a private listening history on this computer and turns it into charts, a listening clock, a shareable listening receipt image, a year in music and monthly recap playlists. Nothing leaves the machine unless the user shares an image or exports a file. The history can be turned off, cleared and exported by the user at any time.

## User Stories

- As a listener, I want every real play recorded on my computer, so that my charts match what I heard.
- As a listener, I want top songs, artists and albums for a week, month, year or all time, so that I can see what I played most.
- As a listener, I want a listening clock by hour, so that I can see when I listen.
- As a listener, I want to make a receipt image of my own stats, so that I can share it if I choose.
- As a listener, I want a year in music and a monthly recap playlist, so that I can look back.
- As a privacy-minded listener, I want to pause, export and clear my history, so that I stay in control.

## Acceptance Criteria

1. WHEN the app is first installed, THE SYSTEM SHALL record no plays. The Stats page SHALL have one switch, "Record my listening on this computer", that is off and not pre-checked, and a text that says the history is stored only on this computer and never leaves it. Plays heard while the switch is off are not recorded and cannot be added later.
2. WHEN the user turns the recording switch off, THE SYSTEM SHALL record no new plays and SHALL keep the existing history until the user chooses "Clear stats".
3. WHEN a track is longer than 30 seconds and has been heard for half its length or for 4 minutes, whichever comes first, THE SYSTEM SHALL record one play with the track id, title, artist, album, source (YouTube Music, SoundCloud or local file), start time and heard seconds. This is Last.fm's rule, so stats and scrobbles agree. WHERE the track length is unknown, THE SYSTEM SHALL count the play at 4 minutes heard.
4. THE SYSTEM SHALL count play time only while audio plays. Time paused and time skipped by seeking SHALL NOT count.
5. WHEN the same track plays again, including repeat one, THE SYSTEM SHALL record a new play.
6. WHEN a track is skipped before the rule in criterion 3 is met, or is 30 seconds long or shorter, THE SYSTEM SHALL NOT record a play.
7. THE SYSTEM SHALL store the history in a local database file under the app data folder, and SHALL NOT send it to any host.
8. WHEN the user opens the Stats screen, THE SYSTEM SHALL show top songs, top artists and top albums for this week, this month, this year and all time, ranked by play count.
9. WHEN the user picks a past month or year, THE SYSTEM SHALL show the charts for that period.
10. WHEN the user opens the listening clock, THE SYSTEM SHALL show plays per hour of the local day for the chosen period.
11. WHEN a period has no plays, THE SYSTEM SHALL show an empty state and SHALL NOT show invented numbers.
12. WHEN the user chooses "Make receipt", THE SYSTEM SHALL create an image of the chosen period's top songs with play counts, the period and the NemoMusic name. The image SHALL be text only and SHALL NOT show cover art.
13. THE SYSTEM SHALL NOT put lyrics, account name, email or account id into the receipt image.
14. WHEN a receipt or recap image is made, THE SYSTEM SHALL save it to a file the user picks or copy it to the clipboard, and SHALL NOT upload it.
15. WHEN a calendar year has plays, THE SYSTEM SHALL offer a year in music summary: total plays, total listening time, top songs, artists and albums, the busiest month and the busiest hour.
16. WHEN a month ends and the user has turned monthly recaps on, THE SYSTEM SHALL create a recap playlist of that month's top songs. Monthly recaps are off by default.
17. WHEN the user picks "Save recap as local playlist", THE SYSTEM SHALL create a local playlist. Songs from a local source SHALL play from the local file.
18. WHERE the signed-in account allows playlist edits through the account-actions spec, THE SYSTEM SHALL offer "Save recap to YouTube Music" and SHALL ask before it writes to the account. Only songs with a YouTube Music id are added, and the SYSTEM SHALL say how many were left out.
19. WHEN the user chooses "Export my stats" (allowed: it is the user's own data), THE SYSTEM SHALL write a CSV or JSON file with the user's own plays to a path the user picks. The file SHALL contain no cookies, tokens, account ids or lyrics.
20. WHEN the user chooses "Clear stats" and confirms, THE SYSTEM SHALL delete all recorded plays, and every chart, receipt and recap derived from them.
21. WHEN the user signs out, THE SYSTEM SHALL keep all recorded plays, because they are this computer's history and not data of the account. The sign-out text SHALL say so and SHALL point to "Clear stats".
22. WHEN a song's metadata is missing, THE SYSTEM SHALL show "Unknown artist" or "Unknown album" and SHALL NOT guess.
23. WHEN the history database cannot be opened or written, THE SYSTEM SHALL show a short message, keep playing, and SHALL NOT crash.
24. WHILE the app is offline, THE SYSTEM SHALL record and show stats as normal.
25. THE SYSTEM SHALL let the user reach every Stats control, period picker and the receipt command with the keyboard, with a visible focus.
26. WHEN reduced motion is on, THE SYSTEM SHALL draw chart and clock changes without animation.
27. THE SYSTEM SHALL NOT label any stats feature "Wrapped" or use another app's name, and SHALL add no new network host.

## Out of Scope

- Sharing stats with other people, friends or a leaderboard.
- A server, an account or a sync of stats between computers.
- Last.fm history import or export (see the integrations spec for scrobbling).
- Genre or mood analysis, and AI commentary (see the ai-features spec).
- Backfilling the local history from the YouTube Music account history.

## Open Questions

Decided by the owner on 2026-11-14:
- Recording is off until the user turns it on (criterion 1). No pre-checked consent.
- A play counts by Last.fm's rule (criterion 3), so stats and scrobbles agree.
- Exporting the user's own stats is allowed (criterion 19).
- Sign-out keeps local stats (criterion 21).
- The receipt image is text only (criterion 12).

None is left open for the owner. The design settles the table layout, retention, the mapping of one song across sources and the chart drawing. The YouTube Music account history is not imported (see Out of Scope).
<!-- Sources:
- .kiro/steering/product.md (Local only; Approved additions: listening-stats)
- .kiro/steering/coc.md (User Privacy: listening history local)
- .kiro/steering/tech.md (rusqlite row)
- https://www.last.fm/api/scrobbling (scrobble rule used as a comparison in Open Question 2)
- crates/nemo-core/src/catalog.rs, crates/nemo-app/src/merged.rs, crates/nemo-lyrics/src/cache.rs (existing code checked)
-->
