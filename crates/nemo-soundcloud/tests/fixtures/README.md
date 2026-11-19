# SoundCloud fixtures

Recorded from the web API v2 on 2026-11-19 (anonymous, a few requests), then rewritten: every id, title,
name, address and signature is invented. The field names and shapes are the real ones. They hold no
`client_id`, no token, no cookie and no real account or track. Do not add real values; a test in
`src/fixtures.rs` fails on a `client_id`, `oauth`, a signature or a host that is not in `HOSTS`.

## Account lists (soundcloud-account, PR 3)

`likes.json`, `likes_page2.json`, `library_all.json`, `followings.json`, `history.json` and `feed.json`
were NOT recorded: no signed-in session was available. They are written by hand from the web app's route
table (`users/:id/track_likes`, `me/library/all`, `users/:id/followings`, `me/play-history/tracks`,
`stream`) and the shapes of the recorded public lists (`collection` of items with a `track`, `playlist` or
user object, and `next_href`). The item `type` words (`playlist`, `playlist-like`, `track-repost`) and the
`played_at` field are the part not checked against a real answer. The 13-digit `played_at` values start
with 7000 so the fixture lint accepts them as invented.
