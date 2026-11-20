## Goal

YouTube Music and SoundCloud are two separate listening areas, not one mixed list. One press on a `Listening area` control moves the whole listening side of the window (sidebar list, Library, Search, Home, Explore and the pages they open) to the other service. The control lives only in the `Your accounts` sheet and in Settings, never on Home or beside it. The SoundCloud account (name, avatar, sign-in, sign-out) sits in the same sheet as the YouTube Music account.

Owner request (2026-11-19, Vietnamese, translated): "SoundCloud is good now, but I want the YTM tab and the SCL tab split, not mixed together. It must affect the Playlist part of the Sidebar and the other features too. One button press and the whole listening area moves to the other place. Put it in Your Account and at least one other place, but do NOT put the button on the homepage or near it. Also add the SCL login data there (to Your Account)."

## User Stories

- As a listener who uses both services, I want each one in its own area, so that my YouTube Music playlists and my SoundCloud likes never sit in one list.
- As a listener, I want one press to move everything I browse to the other service, so that I do not look for a switch on every screen.
- As a listener, I want the song that plays to keep playing when I switch, so that browsing the other service never interrupts the music.
- As a SoundCloud user, I want my SoundCloud account details and sign-in next to my YouTube Music account, so that I manage both in one place.
- As a keyboard user, I want to move the choice with Left and Right.

## Acceptance Criteria

Areas and screens (PR 1)
1. THE SYSTEM SHALL have two listening areas, YouTube Music and SoundCloud, and no area that mixes them. `Source::All`, the source chips of the sidebar, Library and Search, the SoundCloud group under YouTube Music's results and every other branch for `All` SHALL be removed.
2. WHILE YouTube Music is the area, THE SYSTEM SHALL offer Home, Explore, Search and Library, and show the YouTube Music account lists (playlists, albums, artists) and the local-file albums in the sidebar list. Local files stay in the YouTube Music area.
3. WHILE SoundCloud is the area, THE SYSTEM SHALL offer Library and Search only, show only the SoundCloud playlists, albums and artists in the sidebar list, search only SoundCloud, and send a request for Home or Explore to the Library.
4. THE SYSTEM SHALL offer the SoundCloud area only while SoundCloud is on, running and signed in. WHEN it is not offered, THE SYSTEM SHALL show one plain sentence in place of the control instead of a dead choice.
5. WHEN the person searches in one area, THE SYSTEM SHALL NOT run that search in the other, and SHALL show no result of the other service. An answer that arrives after a switch SHALL be kept for the area that asked and not shown in the other.
6. WHEN the person presses the active area, or an area that is not offered, THE SYSTEM SHALL change nothing.

The switch (PR 1)
7. THE SYSTEM SHALL draw the `Listening area` control (two choices, `YouTube Music` and `SoundCloud`) at the top of the `Your accounts` sheet and in a short card at the top of the SoundCloud section of Settings, and nowhere else. It SHALL NOT be on Home, the top bar, the sidebar or the player bar.
8. THE SYSTEM SHALL keep the button that opens the account sheet in the top bar on every screen, and the sheet SHALL open whether or not YouTube Music is signed in. In the SoundCloud area the button SHALL show the SoundCloud account.
9. WHEN the area is switched, THE SYSTEM SHALL open the landing page of the new area (YouTube Music: Home; SoundCloud: Library, its first entry), start its history there so Back never reaches a page of the other area, swap the sidebar list, and remember per area the search box with its answers and chip, and the Library sort order.
10. WHEN the area is switched, THE SYSTEM SHALL NOT touch playback: the queue, the player bar, Now Playing and the lyrics go on. A song of the other service keeps playing, and its album, artist or playlist page opens normally from Now Playing; the page opens and the area stays.
11. WHILE a page of the other service is open (from Now Playing), THE SYSTEM SHALL light no sidebar entry.
12. WHILE SoundCloud is the area, THE SYSTEM SHALL refresh and read only SoundCloud, and WHILE YouTube Music is the area, only YouTube Music; the account data, Home feed and Explore of an inactive area SHALL NOT load in the background until it becomes the area. The reload after a sign-in is the one read the other area gets, on the update that follows it.

Saved choice (PR 1)
13. THE SYSTEM SHALL store the area in the settings file (key `listening_area`, the label of the choice) and read it at start. IF the value is missing, unknown, or names SoundCloud while SoundCloud cannot run, THEN THE SYSTEM SHALL start in YouTube Music and store that.
14. IF SoundCloud is turned off, signed out or its session ends while it is the area, THEN THE SYSTEM SHALL go back to YouTube Music at once and store it, with no popup.

Your accounts (PR 2)
15. THE SYSTEM SHALL title the sheet `Your accounts` and show, after the `Listening area` control, a `YouTube Music` card (what the sheet showed before: name, avatar, handle, sign-out with confirm, or the sign-in parts) and a `SoundCloud` card.
16. The `SoundCloud` card SHALL show: SoundCloud off: one sentence and a `Turn on SoundCloud in Settings` text button; on and signed out: `Sign in with SoundCloud` and `Paste a token instead` (the flow of the Settings card); signed in: round avatar, name, and `Sign out` with the same confirm. It SHALL show a handle and counts only if the account info already holds them, and SHALL never show a token, a cookie or an id.
17. THE SYSTEM SHALL draw the SoundCloud account pieces from the same components and use the same messages as the Settings SoundCloud card, so both places show one state. The sheet SHALL fit 960 x 600 and 200% scale and scroll inside the card when it is taller.

Polish (PR 3)
18. WHILE SoundCloud is the area, THE SYSTEM SHALL show the line `SoundCloud` under the brand in `label-sm` and `on-surface-variant`; WHILE YouTube Music is the area, nothing.
19. WHEN the area is switched, THE SYSTEM SHALL cross-fade the sidebar list with the motion module, and show it at once under Reduce motion. It SHALL draw no frame while nothing moves.
20. WHEN the `Listening area` control has focus, THE SYSTEM SHALL show the focus ring, reach it in drawing order, and move the choice with Left and Right.
21. THE SYSTEM SHALL add no network call and no host for any of this, and take every colour, size and duration from the theme and motion modules.

## Out of Scope

- A SoundCloud Home or Explore screen. `nemo-soundcloud` can read `/mixed-selections` and the trending chart (`home.rs`), but no screen of the app shows them; the Home view is built for YouTube Music's feed, daily mixes and local albums. A later spec can wire it.
- SoundCloud handle and follower, like and track counts. `SourceAccountInfo` holds a name and an avatar only; `/me` is read for those two fields and the rest is dropped. Showing more needs a field in `nemo-core` and in the cached header, which this spec does not add.
- A third place for the switch, a keyboard shortcut for it, and a "last used" memory beyond the settings key.
- Playback, the queue and Now Playing (unchanged).

## Open Questions

- "SCL login data" is read as the SoundCloud account details and sign-in controls inside `Your accounts` (the owner may correct this).
- The SoundCloud area lands on Library (the first entry) because it holds the likes and playlists a listener switches for, and Home does not exist there. Search first would land on an empty box.
- The sidebar lights nothing for a page of the other service, instead of Library, because that page is in neither area's Library. The owner may prefer Library lit.
