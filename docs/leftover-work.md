# Leftover work (as of 2026-11-15)

The feature wave stands at 83 of 418 tasks done. PRs #3 to #109 are merged. `docs/feature-wave-plan.md` has the build order; regenerate it with the plan script after each wave.

## Open now
- [PR #110](https://github.com/nekoo-moe/NemoMusic/pull/110): regenerated plan, new task account-actions#39, `structure.md` line for `nemo-audio`. Merge it after CI.

## Next wave: ready tasks (no owner input needed)
| Area | Tasks |
|---|---|
| nemo-core contracts | ai-features#2, lyrics-plus#4, soundcloud#4, youtube-extras#2, youtube-extras#4 |
| New crates | cast#5 (nemo-cast), listening-stats#2 (nemo-stats), listen-together#3 (nemo-together) |
| Audio | output-devices#2, audio-effects#6, audio-effects#10 |
| App | audio-effects#4, output-devices#5, integrations#17, offline-cache#16 |
| Library / lyrics | local-library#9, #10, #15, lyrics-plus#11 |
| YouTube | account-actions#39, offline-cache#9, #19, youtube-extras#13 |
| Integrations | integrations#13, #19, #28 |
| UI | lyrics-plus#29 |
| Spikes, no hardware | integrations#10, integrations#12 |

## Blocked on the owner
| Needs | Tasks |
|---|---|
| Google throwaway and Premium accounts | account-actions#12, #13, #14, #15 |
| Spotify test account | lyrics-plus#8 |
| Chromecast and DLNA speaker | cast#1 |
| Discord app ID | integrations#11 |
| Docker for metroserver | listen-together#1 |
| Exported playlists | playlist-import#2 |
| A song clip | song-recognition#1 |
| SoundCloud risk text and host capture | soundcloud#7 |
| Linux Mint runs | lyrics-plus#9, video-playback#1 (Linux half) |
| Desktop session | video-playback#1: iced window test, 10-minute audio run |
| `design.md` exceptions approved | now-playing-styles#6 (Apple Music style) |
| Last.fm API account; ask the Metrolist server operator | integrations, listen-together |

## Owner questions
1. Fix the short-song silence bug? The feeder pauses to refill when the next song has under 200 ms buffered. That is an audio-path change, so it needs a new task.
2. Should "Clear lyrics cache" keep the lyrics of songs saved offline?
3. Should AIFF get the Lossless badge? It shows it now, like WAV.
4. Approve the four `design.md` exceptions for the Apple Music style.

## Known gaps (no task yet)
- `library.db` drops `Track.source_url`, so a SoundCloud song in a local playlist loses its page URL. A fix needs the SoundCloud host list in `nemo-library`, which has no edge to `nemo-soundcloud`.
- WebM Opus keeps the last packet's padding, because symphonia ignores `DiscardPadding`. Gapless joins of two WebM Opus streams get up to 20 ms of near-silence.
- `nemo-core/src/audio.rs` is at 499 lines. Move its tests out before the next edit.

## How to resume
- Scratch: `C:\Users\Alyosha\.kiro\crew\scratch\runtime-47065560`. It holds the lane worktrees (`wt-lane-a`, `wt-lane-b`, `wt-lane-c`, `wt-merge`), the target dirs and the logs.
- Briefs: `impl-brief.md` and `wave3-brief.md`. Copy `lane-*-wave3.md` to write the wave 4 lane briefs.
- Scripts: `cargo-run.ps1` runs one build at a time; also `pr-status.ps1`, `plan-tasks.ps1` and `resolve-block.ps1`.
- Rules: one PR per task. Merge only after CI passes and a review. Merge stacked PRs bottom-up. After merging PRs from more than one lane, run workspace clippy on main before the next batch.
