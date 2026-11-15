Spec pre-approved by the owner on 2026-11-15 (owner report of the same day, real Linux run, signed in).

## Goal

The Explore screen shows what music.youtube.com Explore shows: new albums and singles, the charts, the moods and genres (each one opens its own page of playlists from YouTube Music), and new music videos. The calm seed shelves ("Lo-fi to settle in", "Piano", "Acoustic mornings", "Jazz for the evening") appear only when Explore never loaded, under one short line that says so.

## User Stories

- As a listener, I want Explore to show this week's new albums and singles, so that I find new music the way I do on music.youtube.com.
- As a listener, I want the moods and genres YouTube Music offers, so that "Chill" or "Focus" opens YouTube Music's own playlists, not a search.
- As a listener, I want the charts and new music videos, so that I can see what is popular now.
- As a signed-in listener, I want Explore to use my session, so that it matches what YouTube Music shows me.
- As a listener on a bad connection, I want a failed reload to keep what I had, so that the screen never goes blank.

## Acceptance Criteria

1. WHEN the first-run notice is accepted, THE SYSTEM SHALL ask the catalog for its Explore page (`Catalog::explore`) once per account owner (signed out, or one signed-in account), and not before the notice is accepted.
2. WHEN Explore answers with content, THE SYSTEM SHALL show, in order: the new albums and singles, the moods and genres as tiles, the chart shelves, then every other shelf of the page (for example "Trending" and "New music videos").
3. WHEN the user presses a mood or genre tile from the catalog, THE SYSTEM SHALL open a mood page that lists the shelves of `Catalog::mood_playlists` for that tile, and SHALL ask for each page at most once while it is kept.
4. WHEN the user presses a song or music video card on Explore or a mood page, THE SYSTEM SHALL play the songs and videos of that shelf from that card on.
5. WHEN the account owner changes (sign-in, switch, sign-out) or the user presses Refresh, THE SYSTEM SHALL ask for Explore again, with the session the other signed-in calls use.
6. IF Explore fails or answers nothing AND no answer arrived before, THEN THE SYSTEM SHALL show one short line with Retry, the built-in mood tiles (which search) and the seed shelves of Home.
7. IF a later Explore request fails, THEN THE SYSTEM SHALL keep the last answer on screen and show one short line with Retry; it SHALL NOT retry by itself.
8. IF a mood page fails or is empty, THEN THE SYSTEM SHALL say so on the page with a Retry button.
9. WHEN Explore answers, THE SYSTEM SHALL use its new releases and moods for the daily sections of Home, as before.
10. THE SYSTEM SHALL read the real Explore, charts and mood page layouts, proven by tests on recorded anonymous answers with no cookies and no personal data.

## Out of Scope

- Sharp cover art (the sharp-covers spec).
- The full "New releases" and "Moods & genres" sub-pages (`FEmusic_new_releases`, `FEmusic_moods_and_genres`); the Explore page gives the first shelf of each.
- Country picker of the charts page.
- Podcast shows and episodes on Explore shelves (no podcast page yet).

## Open Questions

- None blocking. The charts page answered with the server's guessed country (Singapore from the capture host) despite `gl=US`; users get their own region, which matches the website.
