---
inclusion: always
---

# Conduct and Ethics Guardrails

Applies to Kiro and to every contributor. Community behavior rules: [CODE_OF_CONDUCT.md](../../CODE_OF_CONDUCT.md). Other steering: product.md, tech.md, structure.md, code.md, design.md, workflow.md.

## Honest Context

- The catalog comes from YouTube Music, which serves only lossy audio.
- Access is unofficial and may breach YouTube's Terms of Service. Never claim otherwise in code, docs, UI, or commits.

## Artists and Rights Holders

- Respect artists and rights holders. NemoMusic plays music for listening, not for taking.
- Show artist, album, and credits as the source provides them. Never strip attribution.

## Forbidden Features

Do not add, and do not help add:

- DRM bypass or circumvention of any access control.
- Mass-download, bulk export, or redistribution of YouTube content.
- Scraping at abuse scale: no rate-limit evasion, account farming, or proxy rotation.
- Sharing or sale of extracted streams, keys, or cookies.

Keep request volume low and human-paced. Honor backoff and error signals.

## Owner Decisions (2026-11-10)

- The owner accepted the YouTube Terms of Service risk. This covers unofficial access, Google sign-in, ad blocking and SponsorBlock.
- Ad blocking means the app does not request or play ads. It must never unlock Premium audio quality or bypass DRM. Say plainly in the README and UI that it can restrict the account and reduces income for artists.
- Local only: no project server, no telemetry endpoint. Contact third-party hosts only if they are on the README egress list.
- Lyrics are copyrighted text. Show them, cache them locally, never export or redistribute them. Always show the provider name.
- SponsorBlock data has its own license. Follow its attribution terms (see `docs/adr/0003-adblock-sponsorblock.md`).

## Honest Audio Quality Labeling

- Label what is actually played: codec, bitrate, sample rate when known.
- Never label lossy audio as lossless, "HD", "Hi-Fi", or similar. No upsampling presented as quality.
- Lossless labels apply only to local files whose format is lossless.
- If quality is unknown, show "unknown", not a guess.

## User Privacy

- No telemetry, analytics, or crash reporting without explicit opt-in, off by default.
- No credentials, cookies, tokens, or account identifiers in logs, error messages, test fixtures, or git.
- Store tokens and cookies in the OS keychain only. Never in plain files or config.
- Collect the minimum data. Keep listening history local unless the user chooses otherwise.
- Check `.gitignore` and staged files for secrets before any commit.

## No Dark Patterns

- No nagging, fake urgency, hidden opt-outs, or pre-checked consent.
- Settings that affect privacy or network use must be clear and reversible.
- Disclose the unofficial YouTube access and its risk plainly in the UI and README.

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

- Offline caching of streamed audio: allowed or not, and if allowed, limits and where. Undecided. Do not implement or rule it out.
