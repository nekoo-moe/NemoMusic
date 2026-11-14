## Goal

NemoMusic plays the music files on the user's computer, FLAC included, through the same native player and the same untouched output path as streamed songs. The user picks folders. NemoMusic scans them, reads tags and embedded covers, and keeps the library on disk so it is there after a restart. The user can build local playlists, import and export M3U files, play one-file albums from CUE sheets, edit tags, and open files from the file manager or by drag and drop. Lossless local files on a bit-perfect path get the true-lossless badge. Anything that changes the sound (ReplayGain) is off by default.

## User Stories

- As a listener with a FLAC collection, I want to point NemoMusic at my music folders, so that my files play like any other song.
- As a listener, I want my library to be there when I start the app, so that I do not wait for a full scan.
- As an audiophile, I want to see "Lossless" only when the file is lossless and the path is bit-perfect, so that I can trust the badge.
- As a listener, I want local playlists and M3U import and export, so that my lists move between apps.
- As a listener with ripped albums, I want one-file albums with a CUE sheet to show as separate tracks, so that I can skip and queue them.
- As a listener, I want to double-click a file or drop it on the window and hear it, so that I need no import step.
- As a listener, I want to fix wrong tags in the app, so that my library stays tidy.

## Acceptance Criteria

1. WHEN the user adds a folder in Settings, THE SYSTEM SHALL scan it in the background and show tracks as they are found, without blocking playback or the screen.
2. THE SYSTEM SHALL let the user add and remove watched folders, and SHALL pick the folder with a system folder dialog or a typed path.
3. WHILE a folder is watched, THE SYSTEM SHALL pick up added, changed and removed files on the next start and on a manual rescan. Live file watching is an Open Question.
4. WHEN the user removes a watched folder, THE SYSTEM SHALL remove its tracks from the library and SHALL NOT delete any file on disk.
5. THE SYSTEM SHALL store the library (tracks, tags, playlists, folder list) on this computer in a file that survives restarts, and SHALL show it within one second of start without a rescan.
6. IF the library file is damaged, THEN THE SYSTEM SHALL keep a copy of it, rebuild the library by rescanning, and tell the user.
7. WHEN a file is scanned, THE SYSTEM SHALL read title, artist, album, album artist, track and disc number, year, genre, duration and embedded cover, and SHALL fall back to the file name for a missing title.
8. IF a file has no readable tags or cannot be decoded, THEN THE SYSTEM SHALL still list it with the file name, mark it unplayable when it cannot be decoded, and say why on request.
9. THE SYSTEM SHALL list a file as playable only if the player can decode it. Formats the player cannot decode today SHALL NOT be offered as playable (see Open Questions 1 and 2).
10. WHEN a local track plays, THE SYSTEM SHALL use the same player, queue, gapless playback, seek, media keys and signal-path view as other tracks.
11. WHEN two local tracks of one album follow each other in the queue, THE SYSTEM SHALL play them without a gap, using the encoder delay and padding data where the format gives it.
12. WHEN a local file is lossless (FLAC, ALAC or WAV) and the output path is bit-perfect, THE SYSTEM SHALL show the true-lossless badge with the real sample rate and bit depth.
13. IF the path is not bit-perfect (shared mode, resampling or any effect on), THEN THE SYSTEM SHALL show the codec and format only, without the true-lossless badge, and SHALL say why in the signal-path view.
14. THE SYSTEM SHALL never show "lossless" for a lossy local file (MP3, AAC, Opus, Vorbis), and SHALL show "Unknown" for facts it could not read.
15. THE SYSTEM SHALL keep ReplayGain off by default. WHEN the user turns it on, THE SYSTEM SHALL read the ReplayGain or R128 tags, offer track or album mode and a pre-gain, and show a gain stage in the signal path.
16. IF a track has no ReplayGain tag, THEN THE SYSTEM SHALL play it at unity gain and SHALL NOT scan or guess a gain unless the user asks for a scan (Open Question 5).
17. WHILE ReplayGain is off, THE SYSTEM SHALL do no gain arithmetic, and the bit-exact output test SHALL still pass.
18. WHEN the user creates, renames, reorders or deletes a local playlist, THE SYSTEM SHALL save it at once and keep it after a restart.
19. WHEN the user imports an M3U or M3U8 file, THE SYSTEM SHALL read relative and absolute paths and UTF-8 for M3U8, skip lines it cannot resolve, and report how many were skipped.
20. WHEN the user exports a playlist, THE SYSTEM SHALL write an M3U8 file in UTF-8 to a place the user chooses.
21. IF a playlist entry points to a missing file, THEN THE SYSTEM SHALL keep the entry, show it as missing, and play the next entry.
22. WHEN a scan finds an audio file with a CUE sheet of the same name or an embedded CUE, THE SYSTEM SHALL show each CUE track as a track with its title, artist, start and end, and SHALL play it from the one audio file.
23. WHEN the user edits tags, THE SYSTEM SHALL show the old and new values, write to the file only after the user confirms, keep the file's other data, and update the library.
24. IF a tag write fails (read-only file, file in use), THEN THE SYSTEM SHALL leave the file unchanged and say so.
25. WHEN the user opens a supported file from the file manager or passes it on the command line, THE SYSTEM SHALL play it, in the running app when there is one, and SHALL NOT add it to the library unless it lies in a watched folder.
26. WHEN the user drops files or a folder on the window, THE SYSTEM SHALL offer to play them now, add them to the queue, or add them to a playlist.
27. THE SYSTEM SHALL reach every local-library action with the keyboard, and SHALL show a visible focus.
28. THE SYSTEM SHALL keep the library, covers and playlists on this computer only, SHALL contact no new network host, and SHALL NOT write file paths or tags to logs at info level or above.
29. WHEN the user signs out, THE SYSTEM SHALL keep the local library, because it holds no account data.
30. WHEN the scan list or covers animate, THE SYSTEM SHALL follow the reduced-motion setting.

## Out of Scope

- Ripping CDs, downloading, or fetching tags and covers from online services.
- Moving, renaming or deleting the user's files.
- Local video files.
- Network shares and cloud drives beyond what the operating system presents as a folder.
- Lossless claims for any non-local source.
- Smart playlists and rules.

## Open Questions

These need the owner (workflow.md section 8, coc.md):

1. Formats. Today `symphonia` is built with `flac`, `wav`, `pcm`, `isomp4`, `aac`, `mkv`, `mp3`. ALAC and Vorbis are not enabled, and symphonia 0.6.1 has no Opus decoder. The scan list (`AUDIO_EXTENSIONS`) already names `ogg` and `opus`, which cannot play. Enable ALAC and Vorbis (feature names unverified), and add a separate Opus decoder for local `.opus`, or leave them out? ALAC matters because product.md lists it as lossless.
2. AIFF and other lossless formats (WavPack, APE): in or out of the first release?
3. Storage: `tech.md` lists `rusqlite` as "proposed". Confirm it for the library file, or choose another store.
4. Folder dialog: no picker crate is in use. Which one is approved (candidates: `rfd`; licence and maintenance unverified)?
5. ReplayGain for files with no tags: allow an opt-in background scan that writes only to the library, never to the file?
6. Watching: scan on start and on demand only, or a live file watcher (candidate `notify`, unverified)?
7. The true-lossless badge needs a bit-perfect path. On Linux the path is shared mode until direct ALSA exists, so the badge never shows there. Accept that?
8. Tag editing writes to the user's files. Accept it as in scope, with the confirm step in criterion 23?
9. Technical: where the CUE sheet and its audio file live relative to each other, and how gapless works across CUE tracks of one file. The design settles this.

<!-- Sources:
.kiro/steering/product.md (Scope, Quality badge rule), coc.md (User Privacy), code.md (Bit-perfect), tech.md (Library DB, Tags)
.kiro/specs/app-mvp/tasks.md item 2.3; crates/nemo-library/src/scan.rs, tags.rs; crates/nemo-audio/Cargo.toml; crates/nemo-core/src/audio.rs
https://lib.rs/crates/symphonia (formats supported by the project: AAC, ALAC, FLAC, MKV, MP3, MP4, OGG, Vorbis, WAV)
https://lib.rs/crates/symphonia-adapter-libopus (symphonia has no native Opus support)
-->
