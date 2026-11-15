Spec approved by the owner on 2026-11-15 (design delegated; decisions below).

Owner decisions (2026-11-15):
- Reload in the UI layer, through the existing `nemo-core` catalog and account state. No `nemo-core` public API change.
- Requests stay human-paced: one reload at a time, a second press while one runs does nothing, steps run one after the other.
- The Refresh control is an icon button named `Refresh`, with a tooltip, `F5` and `Ctrl+R`, a visible focus ring, a target of at least 32 px, and a calm busy state that honours reduced motion.
- A failed reload keeps the old data on screen and shows one short message.
- Sign-out already wipes data. That stays as it is.

## Goal

After the user signs in to YouTube Music, or switches to another account, Home, the library playlists, liked songs, albums, artists and history show that account's data without a restart. A Refresh button in the top bar (also `F5` and `Ctrl+R`) reloads the same data on demand. A failed reload never empties the screen.

## User Stories

- As a listener who just signed in, I want my playlists and liked songs to appear at once, so that I do not have to restart the app.
- As a listener with two accounts, I want the views to follow the account I switched to, so that I never see the other account's music.
- As a listener, I want a Refresh button, so that I can pull in a playlist I just made on my phone.
- As a listener on a bad connection, I want a failed refresh to keep what I had, so that the screen never goes blank.
- As a signed-in listener, I want the sidebar library to show my own playlists (with Liked music), albums and artists, so that it matches music.youtube.com and not the suggestions on Home.

## Acceptance Criteria

1. WHEN a sign-in succeeds, THE SYSTEM SHALL reload the Home shelves and then the account data (library playlists, liked songs, albums, artists, history, explore data), once.
2. WHEN the signed-in account changes to another account, THE SYSTEM SHALL reload the same data once and forget the likes changed in this session.
3. WHEN the account state changes without a UI message (a saved session checked at start), THE SYSTEM SHALL notice it on the next update or frame and reload.
4. WHEN the user presses Refresh, `F5` or `Ctrl+R` after the first-run notice was accepted, THE SYSTEM SHALL reload the same data.
5. WHILE a reload runs, THE SYSTEM SHALL ignore further Refresh presses and SHALL run the reload steps one after the other.
6. WHEN the account changes while a reload runs, THE SYSTEM SHALL drop the answer of the step in flight and then start one reload for the new account.
7. IF a reload step fails, THEN THE SYSTEM SHALL keep the data that was on screen for that step and, when the reload ends, show one short notice.
8. WHEN an answer of the start-up loads arrives after a reload replaced that data, THE SYSTEM SHALL drop it.
9. WHILE a reload runs, THE SYSTEM SHALL show the Refresh control dimmed with the tooltip `Refreshing your music…`, with no motion.
10. THE SYSTEM SHALL draw the Refresh control as a 40 px icon button in the top bar, with the tooltip `Refresh (F5)` and the shared focus ring.
11. WHEN the user signs out, THE SYSTEM SHALL keep today's behaviour: the sign-out wipe is unchanged, and only the account data is loaded again (as before this change).
12. WHILE the first-run notice is not accepted, THE SYSTEM SHALL make no reload request.
13. WHILE signed in, THE SYSTEM SHALL list in the left sidebar's "Your library" the account's own lists only: under Playlists `library_playlists()` (Liked music first, as YouTube Music returns it), under Artists `library_artists()`, under Albums `library_albums()` followed by the albums of local files. The same holds after a sign-in, an account switch and a Refresh.
14. WHILE signed out, THE SYSTEM SHALL list only local albums in the sidebar, or the hint `Sign in to YouTube Music to see your playlists, albums and artists here.`; it SHALL NOT list items of Home, Explore or search shelves.
15. WHEN the account changes, THE SYSTEM SHALL drop the previous account's library at once; IF the new account's library then fails to load, THEN THE SYSTEM SHALL show `Could not load your library. Press Refresh (F5) to try again.` instead of another account's lists. While it loads the sidebar says `Loading your library…`; an account with nothing saved shows `Nothing saved here yet.`

## Out of Scope

- Detail pages (album, artist, playlist) already opened: they stay cached; `Retry` still reloads a failed one.
- The local music folders (rescanned by `nemo-app`).
- Automatic periodic refresh.
- The account switcher UI (`AccountSwitcher` is not wired into `Backend` yet); a switch is seen through `AccountService::state`.

## Open Questions

- None. Root cause and design are in `design.md`.
