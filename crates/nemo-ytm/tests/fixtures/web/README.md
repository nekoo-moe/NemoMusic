# Web client fixtures

Answers of `POST https://www.youtube.com/youtubei/v1/{search,browse,next}` with the `WEB` client
(`clientVersion` 2.20261112.10.00, `hl=en`, `gl=US`, no cookies), captured by hand on 2026-11-14
for the youtube-extras spike (task 7; results in `.kiro/specs/youtube-extras/design.md`, Risks).

Stripped to the parts the parsers read: tracking and logging fields, menus, accessibility text,
emoji pickers and extra thumbnails are removed; lists are cut to one or two items. Continuation
tokens are replaced by `TOKEN-<n>` and signed `googlevideo.com` URLs by a placeholder. In the
comments page, comment ids, entity keys, author channels and names, and comment text are
invented (consistent across the threads and the mutations, so the key join still works). Video
titles, channel names and ids are public catalog data and kept as served.

| File | Request | Shows |
|---|---|---|
| `search.json` | `search`, query "lofi girl" | `officialCardViewModel` (the channel card; no `channelRenderer` was served), a live `videoRenderer` (`BADGE_STYLE_TYPE_LIVE_NOW`, "watching", no `lengthText`), a normal `videoRenderer`, a playlist `lockupViewModel`, the continuation section |
| `browse-channel.json` | `browse`, `UCSJ4gkVC6NrvII8umztf0Ow` | `pageHeaderViewModel` header, `channelMetadataRenderer`, every tab with its `params` |
| `browse-videos.json` | `browse` with the Videos tab `params` | `richGridRenderer` of `richItemRenderer` / `lockupViewModel` uploads, continuation |
| `next-comments-token.json` | `next`, `dQw4w9WgXcQ` | the `comment-item-section` with the comments continuation token |
| `next-comments-page.json` | `next` with that token | `commentThreadRenderer` holding a `commentViewModel` (keys only) and the `frameworkUpdates.entityBatchUpdate.mutations` with `commentEntityPayload` and toolbar entities |
| `next-comments-off.json` | `next`, a made-for-kids video | `comment-item-section` with a `messageRenderer` "Comments are turned off." |

The older `commentRenderer` layout was not served, also not with `clientVersion`
2.20230601.00.00, so there is no fixture for it.
