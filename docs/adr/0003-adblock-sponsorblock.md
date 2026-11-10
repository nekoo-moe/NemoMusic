# ADR 0003: Ad blocking and SponsorBlock

Status: proposed - owner to confirm

## Owner decisions recorded
- Ad blocking and SponsorBlock are wanted (owner, 2026-11-10).
- The owner accepts the YouTube Terms of Service risk for both. This is a decision, not a claim that the risk is gone.
- Unchanged: no DRM or Premium-quality bypass, no project server, no telemetry.
## Part A: Ad blocking

### What it means here
NemoMusic does not embed YouTube's web player. `yt-dlp` resolves the stream URL of the track itself, and the app plays that URL. Ads are separate player-inserted items; the client never asks for them, so it never plays them. Nothing is "removed" from audio. No filter lists, no DOM or network-request filtering, no browser engine.

### Are filter lists needed?
No. Filter lists (uBlock-style) exist to hide page elements and block ad requests inside a browser. NemoMusic has no page. Adding one would add a dependency and a parser for nothing.

Unverified: YouTube has tested server-side ad insertion (ads stitched into the stream). If the resolved stream ever contains stitched ads, the client cannot cut them without a segment source. Treat as a known gap; do not claim "no ads ever" in the UI. Detection and handling is out of scope for this ADR.

### Premium vs free
- Premium account: YTM already serves no ads. The switch has no effect and the UI says so.
- Free account: ads are not requested while the switch is on. Audio quality stays at whatever the free tier serves. A track that needs Premium stays unavailable and shows "Not available on this account".

### The feature must NOT
- Unlock a quality tier, bitrate or codec the signed-in account is not served.
- Touch DRM, signature or token checks beyond what `yt-dlp` does for normal resolution; no forged Premium status, headers or cookies.

### Consequences (stated plainly)
- It may breach YouTube's Terms of Service.
- The account may be limited or suspended. Google can do this without notice.
- Artists and rights holders earn less from these listens. Premium or buying music supports them.
- It can stop working at any time if YouTube changes how ads are delivered.

### Switch and wording
- Setting: Settings > Playback > "Don't play ads". Applies only to YouTube Music sources.
- Default: off, with no pre-checked consent (`coc.md`, No Dark Patterns). The first-run dialog explains the risk and offers the switch, unchecked.
- Helper text under the switch: "Ads are never requested. This may break YouTube's Terms of Service, can get your account restricted, and means artists earn less from your listens. Premium accounts already have no ads."
- The README repeats the same text. UI uses Material + Liquid Glass switch styling per `design.md`.
## Part B: SponsorBlock

### Verified API (checked 2026-11-10, wiki.sponsor.ajay.app)
- Base: `https://sponsor.ajay.app`.
- Privacy lookup: `GET /api/skipSegments/:sha256HashPrefix`. Prefix is the first 4 to 32 hex chars of the SHA-256 of the video id; 4 is recommended. The server returns every video that shares the prefix; the client filters by exact `videoID`.
- Query: `categories` (repeatable `category`), `actionType` (repeatable, default `skip`), `service` (default `YouTube`), optional `trimUUIDs=true`.
- Response: array of `{ videoID, segments: [{ segment: [start, end] seconds, UUID, category, actionType, locked, votes, videoDuration, description }] }`. 404 means no data for the prefix; 400 means bad input.
- Categories on the Types page: `sponsor`, `selfpromo`, `interaction`, `intro`, `outro`, `preview`, `hook`, `filler`; also `poi_highlight` (point, action `poi`), `exclusive_access` (action `full`), `chapter`. `music_offtopic` ("Music: Non-Music Section") is listed in the userStats response and has its own wiki page, but the Types page text I fetched did not list it: treat its exact id as verified from userStats only.
- Action types: `skip`, `mute`, `full` (whole video is labelled; start and end both 0), `poi`, `chapter`.
- The wiki warns that `filler` is very aggressive and should not be on by default.
- `videoDuration` is the duration when submitted (0 if unknown, about +-1 s). Compare it with the real duration and ignore segments when they differ by more than 2 s (outdated).

### Database license
- The API and database follow CC BY-NC-SA 4.0 unless the author gives explicit permission (sponsor.ajay.app/database). The wiki says the API follows the same license.
- NonCommercial: NemoMusic must stay non-commercial while it uses SponsorBlock data. Any sale, paid tier, ads or sponsorship of NemoMusic needs written permission from the SponsorBlock author first. Owner: confirm this limit.
- ShareAlike: applies to adapted data. Do not store, republish, bundle or fixture-commit segment data. Cache only locally, per user, for playback. Never ship segments in the repo or installer; tests use synthetic segments.
- Code license: the repo is GPL-3.0. Calling a public API at runtime does not make the code a derivative of the data, but NC-licensed data is not GPL-compatible, so the data must never be part of the distributed work. Legal reading is unverified; ask a lawyer before any commercial step.
- Attribution: name SponsorBlock and its author (Ajay Ramachandran), link `https://sponsor.ajay.app`, name the license with a link, and say the data was fetched live. Show it in Settings > About > Credits and in the README. Exact wording follows the "Attribution Template" on the SponsorBlock wiki (template text unverified; copy it when implementing).

### Limits and uptime
- Rate limits: per-IP limits exist (429 documented for submissions); read-endpoint numbers are unverified and may change.
- Uptime: a single volunteer-run service, no SLA (unverified history). Mirrors listed on the wiki are database dumps only and do not serve the extension API. Self-hosting is rejected (breaks "no project server").
- Client rules: one lookup per track, 3 s timeout, cache per video in memory and in the local DB for 24 h (including "no segments"), exponential backoff on 429 or 5xx, honor any `Retry-After`. No prefetching the whole queue; look up the next track only when it is being pre-buffered.

### Scope: music videos only
Segments exist only for YouTube videos. Audio-only "songs" in YTM (`ATV` type) have none. Only tracks resolved as a music video (`OMV` or user-generated video) are looked up. Local files and audio-only tracks never trigger a request.

### Design
- Trait in `nemo-core`, implementation in `nemo-ytm`:

```rust
pub trait SegmentSource: Send + Sync {
    /// Segments for one track, empty if none or offline. Never errors the player.
    async fn segments(&self, video: &VideoId, duration: Duration)
        -> Result<Vec<Segment>, SegmentError>;
}
pub struct Segment { pub start: Duration, pub end: Duration, pub category: SegmentCategory, pub id: String }
```

- Request: compute SHA-256 of the video id, send only the first 4 hex chars, filter the answer locally. The full video id is never sent. Only `actionType=skip` is requested.
- `mute` segments are not used: muting is a sample change and breaks the bit-perfect rule. `full` is shown as a label only ("whole video marked X"), no action.
- Skip behavior: the decode stage seeks past the segment while the position is inside it. Seek only: no crossfade, no gain change, no DSP. Seeking happens at the decoder position ahead of the ring buffer, so the buffer never plays a gap. A splice click is possible; its audibility is unverified, measure in the first prototype.
- Gapless: a segment that reaches the track end (`end >= duration - 1 s`) is treated as "go to next track" and uses the normal gapless handoff. A skip never trims encoder delay or padding differently and never touches the next track's pre-buffer.
- Per-category settings (Settings > Playback > SponsorBlock), each: Off / Skip / Ask. Master switch off by default; the first-run dialog offers it with the privacy note below.

| Category | Default when master is on |
|---|---|
| music_offtopic | Skip |
| sponsor | Skip |
| selfpromo | Skip |
| interaction | Skip |
| intro, outro, preview, hook | Off |
| filler | Off (wiki says aggressive) |

- Indicator and undo: when a skip happens, a glass toast shows "Skipped: Non-music section (0:42)" for 5 s with an "Undo" button that seeks back to the segment start and disables that segment for this play. Keyboard: `Backspace` undoes the last skip. Also a marker on the seek bar for known segments.
- Offline or failure: no segments, no toast, no error dialog. Playback is unaffected. Settings shows "SponsorBlock unavailable" only when the user opens it.
- Privacy: the hash prefix reveals little, but the host still sees the user's IP and a 4-char prefix. No user id, no submissions, no voting, no view-count calls (`/api/viewedVideoSponsorTime` is not used). No automated submissions ever.

### Third-party hosts due to this feature
- `sponsor.ajay.app` (HTTPS, GET `/api/skipSegments/:prefix`). Add to the README network egress list; needs owner approval (`product.md`).
- Ad blocking adds no host.
## Open points for the owner
Confirm: (1) the NonCommercial limit and no-redistribution rule, (2) both switches default off with a first-run dialog, (3) the default categories.

<!-- Sources (fetched 2026-11-10):
https://wiki.sponsor.ajay.app/w/API_Docs
https://wiki.sponsor.ajay.app/w/Types
https://wiki.sponsor.ajay.app/w/Non-music
https://sponsor.ajay.app/database
https://github.com/ajayyy/SponsorBlock
https://raw.githubusercontent.com/ajayyy/SponsorBlockServer/master/src/middleware/requestRateLimit.ts
Unverified: read-endpoint rate limit numbers, server uptime history, attribution template text, music_offtopic on the Types page, server-side ad insertion in YTM, ATV/OMV video type split.
-->
