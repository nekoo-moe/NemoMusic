## Goal

The user brings a playlist from Spotify or another app into NemoMusic. They choose an export file, the app matches each song to YouTube Music, shows a review screen with the confidence of each match and lets the user fix them, and then saves the result as a YouTube Music playlist in the account or as a local playlist. Nothing is saved until the user confirms. The first version reads files. Reading a public link is a decision for the owner.

## User Stories

- As someone leaving Spotify, I want to import a playlist file, so that my playlists come with me.
- As a careful user, I want to see how sure each match is, so that I fix the wrong ones before saving.
- As a signed-in user, I want the result as a YouTube Music playlist, so that it shows on my other devices.
- As a signed-out user, I want a local playlist, so that I can import without an account.
- As a privacy-minded user, I want the import to stay on this computer and talk only to YouTube Music, so that my files go nowhere else.

## Acceptance Criteria

1. WHEN the user chooses "Import playlist" and picks a file, THE SYSTEM SHALL read it on this computer and send the file to no host.
2. THE SYSTEM SHALL read CSV files with a header row and detect the columns from the header names, not from their position.
3. THE SYSTEM SHALL read the Exportify columns "Track Name", "Artist Name(s)", "Track Duration (ms)", "ISRC" and "Album Name", and the Soundiiz playlist columns `title`, `artist`, `album`, `isrc`, `duration`.
4. IF a column is missing, THEN THE SYSTEM SHALL still import with the columns it has, and SHALL need at least a title.
5. IF a file is not valid UTF-8 text, is empty, is over a size limit or has no title column, THEN THE SYSTEM SHALL say so in plain words and import nothing.
6. WHEN the file is read, THE SYSTEM SHALL show the playlist name (from the file name), the song count and any rows it skipped, with the reason.
7. WHEN matching starts, THE SYSTEM SHALL search YouTube Music for each song with its title and artist, one request at a time, human-paced, honoring backoff and 429 answers.
8. WHEN it scores candidates, THE SYSTEM SHALL use title, artist, duration and, when the data holds one, the ISRC, and SHALL give each song a confidence of high, medium, low or none.
9. WHEN matching runs, THE SYSTEM SHALL show progress, let the user cancel, and keep the results found so far.
10. WHEN matching ends, THE SYSTEM SHALL show a review screen with each source song beside its match: title, artist, duration, confidence and a play preview.
11. THE SYSTEM SHALL sort low and none matches first and SHALL NOT pre-select a match with confidence none.
12. WHEN the user fixes a match, THE SYSTEM SHALL let them search again, pick another candidate, or skip the song.
13. WHEN the user plays a preview on the review screen, THE SYSTEM SHALL play the candidate through the normal player.
14. WHEN the user confirms, THE SYSTEM SHALL save only songs that are not skipped, in the file's order, and show how many were saved and how many were not.
15. WHEN the user chooses a YouTube Music playlist and is signed in, THE SYSTEM SHALL create a playlist in the account with the name the user confirms, using the playlist editing of the account-actions spec.
16. IF the user is not signed in, THEN THE SYSTEM SHALL offer only the local playlist and say why.
17. WHEN the user chooses a local playlist, THE SYSTEM SHALL save it on this computer and send nothing to YouTube.
18. IF saving to the account fails part way, THEN THE SYSTEM SHALL say which songs were saved, and offer a local copy and a retry by the user.
19. THE SYSTEM SHALL show the list of songs that found no match and offer to save that list as a text file the user names.
20. WHEN the import finishes, THE SYSTEM SHALL keep no copy of the imported file and no match history after the review screen closes, other than the saved playlist.
21. WHEN the user signs out, THE SYSTEM SHALL delete any import still in progress.
22. WHILE the import screen is open, THE SYSTEM SHALL reach every action by keyboard with a visible focus, and SHALL honor the reduced-motion setting for progress.
23. WHEN the README is read, THE SYSTEM SHALL state that file import contacts only hosts already listed (`music.youtube.com`), and list any host added for link import.

## Out of Scope

- Exporting a NemoMusic playlist to another app.
- Importing liked songs, albums, artists or podcasts (playlists only).
- Importing from a service's private API, or logging in to another service for the user.
- Matching with a paid or hosted matching service, and any server run by the project.
- Syncing playlists after the import.
- Playlists of local files (the `local-library` spec).

## Open Questions

1. Public link. Spotify's Web API today: a Development Mode app needs the app owner to have Premium, allows 5 allowlisted users, and from March 2026 returns playlist items only for playlists the user owns or collaborates on (Spotify's February 2026 migration guide). A project with no server has no shared client id, and each user would have to register an app. So a link to someone else's public playlist cannot be read through the official API. Reading the public web page or embed instead is unverified and may break Spotify's terms. Does the owner want link import at all? Proposal: no, files only in version one.
2. If Spotify sign-in is wanted (the lyrics-plus spec already plans the user's own Spotify sign-in), should it also read the user's own playlists here? It would need a client id the user supplies, plus the hosts `accounts.spotify.com` and `api.spotify.com` (unverified), opt-in and off by default.
3. Other file formats. The Soundiiz export page lists CSV, XML, JSON, TXT, URL and XSPF. TuneMyMusic's file export format is unverified. Which to support first? Proposal: CSV, then XSPF and plain "title - artist" text.
4. ISRC. Exportify and Soundiiz CSV include it. Unverified: whether YouTube Music search can return or filter by ISRC. If not, ISRC only helps when a matched candidate carries one. The design settles how matching uses it.
5. Matching quality. Proposed thresholds for high, medium and low need real playlists to tune. Which sample set does the owner supply?
6. Where matching lives. `structure.md` has no crate for it. `nemo-ytm` owns catalog calls but must not depend on other crates; a parser could sit in `nemo-core` or a new crate. Design decides.
7. Dependency: playlist creation in the account needs the playlist editing of the `account-actions` spec. Should this spec wait for it, or ship local-only first?
8. Volume: a 1,000-song playlist at one search at a time takes a long while. Is a hard limit per import wanted, so the pace stays polite?

<!-- Sources:
.kiro/steering/product.md (Scope; playlist-import), .kiro/steering/coc.md (Forbidden Features; User Privacy)
crates/nemo-core/src/catalog.rs (Catalog trait, search, playlists)
https://github.com/watsonbox/exportify (CSV field list, includes ISRC)
https://support.soundiiz.com/hc/en-us/articles/38063068900754-How-to-Export-Your-Playlists-and-Favorites-to-a-File (formats and playlist CSV columns)
https://developer.spotify.com/documentation/web-api/tutorials/february-2026-migration-guide (playlist items only for owned or collaborative playlists; Premium owner; 5 users)
https://developer.spotify.com/documentation/web-api/concepts/quota-modes (development vs extended quota; organisations only, 250k MAU)
-->
