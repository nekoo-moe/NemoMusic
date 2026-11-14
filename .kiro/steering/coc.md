---
inclusion: always
---

# Conduct and Ethics Guardrails

Applies to Kiro and to every contributor. Community behavior rules: [CODE_OF_CONDUCT.md](../../CODE_OF_CONDUCT.md). Other steering: product.md, tech.md, structure.md, code.md, design.md, workflow.md.

## Honest Context

- The catalog comes from YouTube Music and SoundCloud, which serve only lossy audio.
- Access to both is unofficial and may breach their Terms of Service. Never claim otherwise in code, docs, UI, or commits.

## Artists and Rights Holders

- Respect artists and rights holders. NemoMusic plays music for listening, not for taking.
- Show artist, album, and credits as the source provides them. Never strip attribution.

## Forbidden Features

Do not add, and do not help add:

- DRM bypass or circumvention of any access control.
- Bulk export or redistribution of streamed content (YouTube, SoundCloud), or any download that leaves the app's private cache in a playable form.
- Scraping at abuse scale: no rate-limit evasion, account farming, or proxy rotation.
- Sharing or sale of extracted streams, keys, or cookies.

Keep request volume low and human-paced. Honor backoff and error signals.

## Owner Decisions (2026-11-10)

- The owner accepted the YouTube Terms of Service risk. This covers unofficial access, Google sign-in, ad blocking and SponsorBlock.
- Ad blocking means the app does not request or play ads. It must never unlock Premium audio quality or bypass DRM. Say plainly in the README and UI that it can restrict the account and reduces income for artists.
- Local only: no project server, no telemetry endpoint. Contact third-party hosts only if they are on the README egress list.
- Lyrics are copyrighted text. Show them, cache them locally, never export or redistribute them. Always show the provider name. (One exception, 2026-11-14 below.)
- SponsorBlock data has its own license. Follow its attribution terms (see `docs/adr/0003-adblock-sponsorblock.md`).

## Owner Decisions (2026-11-14)

- The owner approved the additions listed in product.md (Scope, "Approved additions") and accepted their Terms of Service risk: SoundCloud, the offline cache, Premium quality through the signed-in session, Spotify lyrics and animated covers through the user's own Spotify sign-in, song recognition through a Shazam-compatible service, and Listen Together rooms compatible with Metrolist.
- Offline cache: songs the user saves, and a capped set of the most-played songs (smart downloads), stay in the app's private cache. They play only inside NemoMusic, are never exported, and are deleted on sign-out. The badge shows the cached file's real codec and bitrate. Downloads stay human-paced (see Forbidden Features).
- Premium quality is used only for accounts that have Premium. It never unlocks Premium for other accounts.
- Lyrics excerpt: the user may share up to 4 lines of one song as an image, with title, artist and provider credit. Full lyrics are never exported. Lyrics the user writes or times stay on this computer.
- Every new host still goes on the README egress list in the change that adds it, and every new service names its risk in the README and in its settings.

## Honest Audio Quality Labeling

- Label what is actually played: codec, bitrate, sample rate when known.
- Never label lossy audio as lossless, "HD", "Hi-Fi", or similar. No upsampling presented as quality.
- Lossless labels apply only to local files whose format is lossless.
- If quality is unknown, show "unknown", not a guess.

## User Privacy

- No telemetry, analytics, or crash reporting without explicit opt-in, off by default.
- No credentials, cookies, tokens, or account identifiers in logs, error messages, test fixtures, or git.
- Store tokens and cookies in the OS keychain only. Never in plain files or config.
- Collect the minimum data. Keep listening history local unless the user chooses otherwise. Listening stats, the year in music and recaps are computed on this computer.
- The microphone records only while the user runs song recognition, and only an audio fingerprint leaves the machine, never the recording.
- AI features, Last.fm scrobbling, Discord presence, Listen Together, Cast and notifications from followed artists are opt-in and off by default. Each says what it sends, and to whom, before it is turned on.
- Check `.gitignore` and staged files for secrets before any commit.

## No Dark Patterns

- No nagging, fake urgency, hidden opt-outs, or pre-checked consent.
- Settings that affect privacy or network use must be clear and reversible.
- Disclose the unofficial YouTube and SoundCloud access and its risk plainly in the UI and README.

## Reporting a Concern

- Conduct issues: contact `<CONTACT-EMAIL>` (placeholder, owner to fill in; never invent an address).
- Legal or rights-holder concerns: open an issue or contact the owner. Act on takedown requests promptly.
- Security issues: report privately, not in a public issue.

## Kiro Must Stop and Ask the Owner

Pause and ask before any change that touches:

- YouTube Terms of Service, access methods, or endpoints used.
- Credentials, cookies, tokens, login flows, or keychain handling.
- Legal risk: licensing, copyright, takedowns, third-party code with unclear terms.
- User data: storage, logging, network transmission, telemetry, or deletion.
- Anything that looks like a forbidden feature above.

Do not work around a refusal. Explain the concern and wait.

## Open Decisions for the Owner

- None open from this file. Offline caching was decided on 2026-11-14 (see Owner Decisions).
