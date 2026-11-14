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

1. WHEN the app is first installed, THE SYSTEM SHALL record history by default and SHALL say in Settings that it is stored only on this computer. (Owner to confirm the default, see Open Questions.)
2. WHEN the user turns history recording off, THE SYSTEM SHALL record no new plays and SHALL keep the existing history until the user clears it.
3. WHEN a track has been heard for 30 seconds of play time, or for half its length if the track is shorter than 60 seconds, THE SYSTEM SHALL record one play with the track id, title, artist, album, source (YouTube Music, SoundCloud or local file), start time and heard seconds.
4. THE SYSTEM SHALL count play time only while audio plays. Time paused and time skipped by seeking SHALL NOT count.
5. WHEN the same track plays again, including repeat one, THE SYSTEM SHALL record a new play.
6. WHEN a track is skipped before the rule in criterion 3 is met, THE SYSTEM SHALL NOT record a play.
7. THE SYSTEM SHALL store the history in a local database file under the app data folder, and SHALL NOT send it to any host.
8. WHEN the user opens the Stats screen, THE SYSTEM SHALL show top songs, top artists and top albums for this week, this month, this year and all time, ranked by play count.
9. WHEN the user picks a past month or year, THE SYSTEM SHALL show the charts for that period.
10. WHEN the user opens the listening clock, THE SYSTEM SHALL show plays per hour of the local day for the chosen period.
11. WHEN a period has no plays, THE SYSTEM SHALL show an empty state and SHALL NOT show invented numbers.
12. WHEN the user chooses "Make receipt", THE SYSTEM SHALL create an image of the chosen period's top songs with play counts, the period and the NemoMusic name.
13. THE SYSTEM SHALL NOT put lyrics, account name, email or account id into the receipt image.
14. WHEN a receipt or recap image is made, THE SYSTEM SHALL save it to a file the user picks or copy it to the clipboard, and SHALL NOT upload it.
15. WHEN a calendar year has plays, THE SYSTEM SHALL offer a year in music summary: total plays, total listening time, top songs, artists and albums, the busiest month and the busiest hour.
16. WHEN a month ends and the user has turned monthly recaps on, THE SYSTEM SHALL create a recap playlist of that month's top songs. Monthly recaps are off by default.
17. WHEN the user picks "Save recap as local playlist", THE SYSTEM SHALL create a local playlist. Songs from a local source SHALL play from the local file.
18. WHERE the signed-in account allows playlist edits through the account-actions spec, THE SYSTEM SHALL offer "Save recap to YouTube Music" and SHALL ask before it writes to the account. Only songs with a YouTube Music id are added, and the SYSTEM SHALL say how many were left out.
19. WHEN the user chooses "Export my stats", THE SYSTEM SHALL write a CSV or JSON file with the user's own plays to a path the user picks. The file SHALL contain no cookies, tokens, account ids or lyrics.
20. WHEN the user chooses "Clear history" and confirms, THE SYSTEM SHALL delete all recorded plays, and every chart, receipt cache and recap derived from them.
21. WHEN the user signs out, THE SYSTEM SHALL keep local-file plays and SHALL delete plays and account history that came from the signed-in account, as required by the sign-out wipe rule. (Owner to confirm, see Open Questions.)
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

1. Default: record history from first run (criterion 1), or ask on first run? `coc.md` says keep history local and avoid pre-checked consent, but nothing leaves the machine. The owner decides.
2. Play rule: 30 seconds or half of a short track. Match Last.fm's rule (30 seconds minimum length, half or 4 minutes) instead, so both counts agree?
3. Export (criterion 19): approved as a proposal. Confirm that the user may export their own stats.
4. Sign-out (criterion 21): should plays of signed-in songs be deleted, or only account history pulled from YouTube Music? Plays are the user's own data, but product.md says sign-out wipes cached account data.
5. Should the receipt image show cover art? Covers are the artists' work, but the lyrics-excerpt rule allows title and artist only. Text only is the safe default.
6. The design must settle: table layout (schema, index, retention), whether to import YouTube Music account history once (`HistorySection` in `crates/nemo-core/src/catalog.rs`, `history()` in `crates/nemo-app/src/merged.rs`), and how a play maps across sources for the same song.
7. Exists today: no local play log. `rusqlite` is already used by `nemo-lyrics` (`crates/nemo-lyrics/src/cache.rs`) and is pinned in `tech.md`, so a candidate is reuse in a new store. Chart drawing is a candidate for iced canvas.

<!-- Sources:
- .kiro/steering/product.md (Local only; Approved additions: listening-stats)
- .kiro/steering/coc.md (User Privacy: listening history local)
- .kiro/steering/tech.md (rusqlite row)
- https://www.last.fm/api/scrobbling (scrobble rule used as a comparison in Open Question 2)
- crates/nemo-core/src/catalog.rs, crates/nemo-app/src/merged.rs, crates/nemo-lyrics/src/cache.rs (existing code checked)
-->
