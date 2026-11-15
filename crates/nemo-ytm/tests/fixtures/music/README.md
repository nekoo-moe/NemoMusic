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
