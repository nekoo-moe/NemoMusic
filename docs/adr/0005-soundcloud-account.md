# ADR 0005: SoundCloud as a full second source: native client, account login, client id

Status: approved by the owner on 2026-11-19 (request below); details to confirm are in the spec's Open Questions
Date: 2026-11-19
Related: `.kiro/specs/soundcloud-full/`, `.kiro/specs/soundcloud/` (partly replaced), ADR 0001 (Google sign-in, same pattern), ADR 0004 (crates and shared rules), `coc.md`, `product.md`.

## Owner request (2026-11-19)
"Add the whole SoundCloud music flow: login, playing music with a low load delay, and features like playlists and so on working fully, like YTM." The owner accepted the unofficial-access risk for SoundCloud on 2026-11-14 and now approves login, a native client in `nemo-soundcloud` (with its own HTTP client), `yt-dlp` as a fallback only, and the new hosts and public API additions the spec lists.

## Context
The first SoundCloud design (2026-11-14) read public tracks through `yt-dlp` and gave `nemo-soundcloud` no sockets. That cannot give a low start delay (a process start and a full page extraction for every track) and has no account. SoundCloud has no open official API for this (registration needed, terms for a desktop player unverified), so every option is unofficial.

## Decision 1: how the app talks to SoundCloud
A native client in `nemo-soundcloud` for the web API v2 at `api-v2.soundcloud.com`, the same calls soundcloud.com makes. Why: one HTTPS call per step instead of a process, typed answers, paging, and the account endpoints exist only there. `yt-dlp` stays for a later fallback. The crate still depends on `nemo-core` only; it carries its own small copy of the HTTP rules of `nemo-ytm/src/http.rs` because crates must not depend on each other. No new crate: `reqwest` =0.13.5 and `tokio` =1.54.1 are already pinned for `nemo-ytm`.

## Decision 2: the client id
Every API call needs a `client_id` query value. The web app ships one in a script on `a-v2.sndcdn.com`. The app reads `https://soundcloud.com/`, fetches the listed scripts from last to first and takes the first match of `client_id:"<32 letters or digits>"` (yt-dlp's `_update_client_id` does the same; verified on 2026-11-19: the id sat in the last of nine scripts). It lives in memory only, is never logged, is replaced once on a 401 or 403, and is not a project secret or a project-owned identity: the project registers nothing and ships no id. Risk: SoundCloud can change where the id sits; the fallback and the refresh limit the damage. Cost: one 3 MB script per launch, so the app discovers the id in the background when SoundCloud is on.

## Decision 3: login
Same pattern as ADR 0001 and the existing sign-in window: `nemo-signin` opens `https://soundcloud.com/signin` in a webview in its own process, waits until a page of `soundcloud.com` carries the `oauth_token` cookie (the web app's `OAUTH_TOKEN_COOKIE_NAME`, read from its script) and prints it once on stdout. `nemo-app` stores it only through `SecretStore` (`nemo-secrets`, key `soundcloud/oauth-token`), checks it with `GET /me` and sends `Authorization: OAuth <token>` to `api-v2.soundcloud.com` only. yt-dlp's `--username oauth --password <token>` and `Authorization: OAuth` header confirm this is how a web session authenticates. Rejected: asking for the password (the app must never see it); the password endpoint `api-auth.soundcloud.com/web-auth/sign-in/password` (needs bot checks the app cannot pass honestly); the official OAuth flow (needs an app registration the project cannot ship); a cookie paste (open question 1; not in the first version). A 401 means the session ended. Sign-out deletes the key, the window's profile folder and every cache of account data.

## Decision 4: playback
Tracks list `media.transcodings[]`. The app ranks the playable ones with `rank_formats`, resolves the winner with one call (`GET <transcoding url>?client_id=…`, plus `track_authorization` when the track has one and the OAuth header when signed in) and reads the signed address it returns. Facts of 2026-11-19 (81 tracks from five searches): 76 offer only HLS as fragmented MP4 AAC at 160 and 96 kbps (plus the adaptive `abr_sq` list, skipped); 4 offer MP3 progressive and HLS (three old full uploads, one 30 s preview); 1 is encrypted only (`cbc-encrypted-hls`, `ctr-encrypted-hls`). The app plays AAC HLS as the normal case, reads each segment into memory only, never writes audio to disk, never reads an encrypted stream and never tries to (`coc.md`: no DRM circumvention). A preview stays a preview.

## Decision 5: what the app sends
Searches, page and list reads, the stream lookups of tracks the user plays (and the next one in the queue), and the account's own reads and writes when the user acts. No analytics or play-count events of the web player, no telemetry. Request rate is human-paced: one API call at a time, 250 ms apart, 429 and `Retry-After` obeyed (60 s default). Because the app sends no play events, SoundCloud probably does not count these plays for artists; the README says so. The history write (`POST /me/play-history`) is a separate switch, off by default.

## Hosts (README egress rows, each "only when SoundCloud is on")
| Host | Why |
|---|---|
| `soundcloud.com` | The home page, to find the client id; the sign-in page while signing in |
| `api-v2.soundcloud.com` | Search, pages, stream lookup, the account |
| `a-v2.sndcdn.com` | The web app's script that holds the client id |
| `i1.sndcdn.com` | Covers and avatars |
| `playback.media-streaming.soundcloud.cloud` | The HLS audio (playlist, init and segments) |
| `*.sndcdn.com` | Preview and legacy MP3 files (`cf-preview-media.sndcdn.com`, seen), allowed by the `MediaUrl` list for https only, no user, no port |

Hosts that appear in answers but are not contacted (`api.soundcloud.com`, `wave.sndcdn.com`, artist links) stay off the list. A host seen only in the future needs the owner's approval first.

## Honest text (draft, owner to approve)
README and settings, in this order: "SoundCloud is not an official integration. NemoMusic uses the same web calls as soundcloud.com, without an official API key. SoundCloud may block this, may restrict your account if you sign in, and its terms may not allow it. When you sign in, a token for your SoundCloud session is kept in your computer's credential store and is sent only to SoundCloud. Plays here may not count in artists' SoundCloud statistics. SoundCloud audio is lossy (AAC or MP3); NemoMusic never calls it lossless. Off by default." The opt-in `version` becomes 2, so everyone who confirmed the first text confirms this one.

## Consequences
- `structure.md` and `tech.md` change in PR 1 (labelled), the README rows land in PR 1 so the owner sees the whole set before any host is contacted (the first request comes in PR 2 for the API, script and image hosts, PR 3 for the media hosts).
- The old `soundcloud` spec's criteria 21 (no sign-in) and 24 (only through `yt-dlp`) and its "no sockets" design are replaced; its core types are reused.
- If SoundCloud changes the client id location or the transcoding shapes, parsing fails in one place and the fallback or a patch fixes it; there is no server of ours to update.

## Sources
- yt-dlp `yt_dlp/extractor/soundcloud.py` (Unlicense): `_update_client_id`, `_call_api` refresh on 401 or 403, `_extract_info_dict` (transcodings, `ctr-` and `cbc-` encrypted protocols treated as DRM, `abr` skipped), `Authorization: OAuth` header.
- soundcloud.com web app scripts on `a-v2.sndcdn.com` (route table: `me`, `users/:id/track_likes`, `me/library/all`, `me/play-history/tracks`, `mixed-selections`, `system-playlists/:urn`, `playlists`, `me/followings/:id`), read 2026-11-19.
- Anonymous probes on 2026-11-19 of the calls named in `.kiro/specs/soundcloud-full/requirements.md` (Sources).
