# YouTube Music client fixtures

Answers of `POST https://music.youtube.com/youtubei/v1/{browse,search}` with the `WEB_REMIX`
client, no cookies and no account, captured by hand on 2026-11-15. There is no personal data:
titles, artist names, ids and thumbnail URLs are public catalog data and kept as served.

## Pages (real-explore spec)

`browse` answers with `clientVersion` 1.20251110.01.00, `hl=en`, `gl=US` (three requests, at
least 20 s apart). The answers were localised by the server to Singapore despite `gl=US`; that
does not change the layout.

Stripped to the parts the readers in `src/feeds/` look at: `responseContext` (with the visitor
data), tracking and logging fields, menus, overlays, badges, accessibility text and continuation
data are removed; query strings of `i.ytimg.com` thumbnail URLs are cut; shelves are cut to their
first three items (six for the mood tiles), and the mood page to its first two shelves of two.

| File | Request | Shows |
|---|---|---|
| `browse-explore.json` | `browseId` `FEmusic_explore` | the header-less `gridRenderer` of the New releases / Charts / Moods & genres buttons; carousels "New albums & singles" (`musicTwoRowItemRenderer` albums), "Moods & genres" (`musicNavigationButtonRenderer` with `FEmusic_moods_and_genres_category` and `params`), "Trending" (`musicResponsiveListItemRenderer` rows) and "New music videos" (cards with `MUSIC_VIDEO_TYPE_OMV` watch endpoints) |
| `browse-charts.json` | `browseId` `FEmusic_charts` | an empty `musicShelfRenderer` (the country picker), carousels "Video charts" (chart playlists) and "Top artists" (artist rows) |
| `browse-mood.json` | `browseId` `FEmusic_moods_and_genres_category`, the `params` of the "Chill" tile | `musicHeaderRenderer` with the mood title, carousels of playlist cards |

## Search (search-like-ytm spec)

`search` answers with `clientVersion` 1.20261111.01.00, `hl=en`, `gl=VN`, no `params` (one
request). Stripped to the parts the parser in `src/search/` reads: tracking and logging fields,
menus, accessibility text, badges, colours and button styles are removed, and the result list is
cut to 12 of its 25 entries (one or more of each kind).

| File | Request | Shows |
|---|---|---|
| `search-hvl.json` | `search`, query "HVL" | the `chipCloudRenderer` header (two chips), the top-result `musicCardShelfRenderer` (album, Play and Shuffle buttons), then one `itemSectionRenderer` per result in YouTube's order: songs, an artist, music videos (`OMV`, `OFFICIAL_SOURCE_MUSIC`), a second artist, a playlist, a single, a profile (`MUSIC_PAGE_TYPE_USER_CHANNEL`), a podcast episode and a podcast |
| `search-hvl-songs.json` | `search`, query "HVL", `params` of the Songs chip | the selected chip, one `musicShelfRenderer` "Songs" (4 of 20 rows: artist and album linked, duration, no type label) |
| `search-hvl-albums.json` | `search`, query "HVL", `params` of the Albums chip | one `musicShelfRenderer` "Albums" (4 of 20 rows with the "Album" / "Single" label, linked artist, year) |
| `search-hvl-playlists.json` | `search`, query "HVL", `params` of the Community playlists chip | one `musicShelfRenderer` "Community playlists" (4 of 20 rows: linked author, views) |

The filtered answers came from three more requests on 2026-11-15, 25 s apart, same client and
no cookies. Their continuations were removed. The chip `params` of that day, all from the
unfiltered answer: Albums `EgWKAQIYAWoQEAkQAxAFEAQQChAVEBAQEQ%3D%3D`, Songs
`EgWKAQIIAWoQEAkQAxAFEAQQChAVEBAQEQ%3D%3D`, Artists `EgWKAQIgAWoQEAkQAxAFEAQQChAVEBAQEQ%3D%3D`,
Videos `EgWKAQIQAWoQEAkQAxAFEAQQChAVEBAQEQ%3D%3D`, Community playlists
`EgeKAQQoAEABahAQCRADEAUQBBAKEBUQEBAR`, Profiles `EgWKAQJYAWoQEAkQAxAFEAQQChAVEBAQEQ%3D%3D`,
Episodes `EgWKAQJIAWoQEAkQAxAFEAQQChAVEBAQEQ%3D%3D`, Podcasts
`EgWKAQJQAWoQEAkQAxAFEAQQChAVEBAQEQ%3D%3D`.

## Album and playlist headers (collection-sort-meta spec)

`browse` answers with `clientVersion` 1.20261111.01.00, `hl=en`, `gl=VN`, no cookies (two
requests on 2026-11-15, 25 s apart). Stripped like the pages above: `responseContext`, tracking,
menus, buttons, overlays, badges, accessibility text, the playlist `description` and the sort
menu of the playlist shelf are removed, and the track lists are cut to their first three rows.
The playlist owner is a person's channel, so its name (`Playlist Owner`), channel id
(`UCsyntheticowner000000000`) and picture URL were replaced by synthetic values; the album
artist is public catalog data and kept.

| File | Request | Shows |
|---|---|---|
| `browse-playlist.json` | `browseId` `VLPLRzHtJL72A7MlKdmSGwaL9VlvVSqUM7MT` | `musicResponsiveHeaderRenderer` with `subtitle` "Playlist • 2026", `secondSubtitle` "437K views • 30 tracks • 36 minutes" and the owner in `facepile.avatarStackViewModel`; a `musicPlaylistShelfRenderer` |
| `browse-album.json` | `browseId` `MPREb_7uoXaDktF5l` | `musicResponsiveHeaderRenderer` with `subtitle` "Album • 2026", `secondSubtitle` "8 songs • 28 minutes", the artist in `straplineTextOne` and their picture in `straplineThumbnail`; a `musicShelfRenderer` of rows |
