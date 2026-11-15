# YouTube Music client fixtures

Answers of `POST https://music.youtube.com/youtubei/v1/search` with the `WEB_REMIX` client
(`clientVersion` 1.20261111.01.00, `hl=en`, `gl=VN`, no cookies, no `params`), captured by hand
on 2026-11-15 for the search-like-ytm spec (one request).

Stripped to the parts the parser reads: tracking and logging fields, menus, accessibility text,
badges, colours and button styles are removed, and the result list is cut to 12 of its 25
entries (one or more of each kind). No account was signed in, so the answer holds no personal
data. Titles, artist names and ids are public catalog data and kept as served.

| File | Request | Shows |
|---|---|---|
| `search-hvl.json` | `search`, query "HVL" | the `chipCloudRenderer` header (two chips), the top-result `musicCardShelfRenderer` (album, Play and Shuffle buttons), then one `itemSectionRenderer` per result in YouTube's order: songs, an artist, music videos (`OMV`, `OFFICIAL_SOURCE_MUSIC`), a second artist, a playlist, a single, a profile (`MUSIC_PAGE_TYPE_USER_CHANNEL`), a podcast episode and a podcast |
