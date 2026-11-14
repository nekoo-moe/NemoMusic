# ADR 0004: Crates and shared rules for the 2026-11-14 feature wave

Status: proposed - owner approves it together with the designs

## Owner decisions recorded
- All 17 features in `product.md` (Scope, "Approved additions") are approved (owner, 2026-11-14).
- The owner accepted the recommended answers to the requirements' open questions (2026-11-14). Each spec's `requirements.md` records the answers that touch it.
- Song recognition stays in, and the owner accepts its patent and Terms of Service risk.

## Context
`structure.md` allows `nemo-app` to depend on every crate, and every other crate to depend on `nemo-core` only. Library, ytm and lyrics crates must not depend on each other. The wave adds services (SoundCloud, Spotify, Last.fm, AI, Cast, rooms, recognition), local data (library, stats, offline cache) and audio stages (effects, crossfade, exclusive output). Seventeen designs written in parallel need one map, or they will put the same thing in different places.

## Decision 1: where the code lives
Existing crates take what is clearly theirs:

| Crate | Takes |
|---|---|
| `nemo-core` | New shared types and traits only (see Decision 2). No I/O. |
| `nemo-audio` | Effects chain (EQ, delay, reverb, ReplayGain gain), crossfade mixer, device switching, WASAPI exclusive sink, a file `ByteSource` for local and cached files, more decoders (ALAC and Vorbis through `symphonia` features, Opus through a decoder crate). |
| `nemo-library` | The SQLite library, folder watching, local playlists and M3U, CUE sheets, tag editing, stored ReplayGain values. |
| `nemo-ytm` | Offline cache (downloads and index), dislike and playlist edits, several accounts, Premium session for `yt-dlp`, audio-language choice, YouTube search, live streams, comments, Return YouTube Dislike, YouTube captions as a `LyricsProvider`, and a generic `yt-dlp` URL resolver behind a `nemo-core` trait. |
| `nemo-lyrics` | Own lyrics, romanization, the translation hook (calls a `nemo-core` trait, never an AI crate). |
| `nemo-ui` | Every screen. Talks to services only through `nemo-core` traits and messages. |
| `nemo-app` | Wiring: builds each service and injects it into the others through `nemo-core` traits. Still no business logic. |

New crates, each depending on `nemo-core` and its own third-party crates only:

| New crate | Holds |
|---|---|
| `nemo-secrets` | The OS credential store. The `SecretStore` trait moves from `nemo-ytm/src/credentials.rs` to `nemo-core`; this crate holds the Windows and Linux implementations. Every crate that needs a secret gets one through the trait. |
| `nemo-soundcloud` | SoundCloud catalog and pages. Stream lookup uses the `nemo-core` URL-resolver trait that `nemo-ytm` implements with `yt-dlp`. |
| `nemo-spotify` | The user's Spotify session, Spotify lyrics (a `LyricsProvider`) and Canvas (an animated-art trait). |
| `nemo-stats` | The local play log (SQLite) and every aggregate: charts, clock, receipt, year in music, recaps. |
| `nemo-integrations` | OS media controls and media keys, Last.fm, Discord Rich Presence. |
| `nemo-ai` | The client for the endpoint the user sets, behind a `nemo-core` trait used by lyrics translation and suggestions. |
| `nemo-recognize` | Microphone and loopback capture, the fingerprint, the recognition request. |
| `nemo-cast` | Cast and DLNA discovery and control, and the LAN-only stream server. |
| `nemo-together` | The Listen Together room client. |
| `nemo-video` | Video decoding. `nemo-ui` shows frames through a `nemo-core` trait, never by depending on this crate. |

A design may merge two of these or add a module elsewhere if it says why. A design that needs a crate-to-crate edge other than "depends on `nemo-core`" must stop and raise it.

## Decision 2: shared traits in `nemo-core`
`nemo-core` files are shared. Each design names the traits and types it adds, signatures only. At implementation time only one task at a time edits a given `nemo-core` file, and the task that adds a trait lands before the tasks that use it. Expected additions: `SecretStore`, a URL resolver for `yt-dlp`, a new `Origin` value for SoundCloud, a translator, an animated-art source, a play-log and stats service, a downloads service, a remote-playback service (Cast and rooms), a recognizer, and video frames.

## Decision 3: rules every design follows
- Storage: one SQLite file per owning crate (as `nemo-lyrics` does now), under the app data folder, with migrations. No shared database.
- Secrets: only through `SecretStore`, wrapped in `Secret<T>` (`nemo-core/src/secret.rs`) so they never print.
- Opt-in: each opt-in feature has one settings entry with the switch, what is sent, to which hosts, and its risk text, shown before the first request. One shared UI pattern for all of them.
- Network: every new host goes on the README egress list in the task that first contacts it.
- Audio: every stage that alters samples registers in the signal path, is off by default and does no arithmetic when off. The real-time rules of `code.md` hold. The bit-exact test passes with every stage off.
- Sign-out: every crate that keeps account data registers a wipe step that sign-out runs.
- Steering: `tech.md` (crates) and `structure.md` (new crates and edges) change in the implementing task, as their change rules say. Designs list the edits.

## Decision 4: shared items, owners and users
Checked against the 17 designs on 2026-11-14. One spec owns each item and defines it in full; the other specs use its name. When two designs differ, the owner's name wins. Items marked "new" had no owner before this review and were assigned here. Names below are the final ones; the designs were edited to match.

| # | Item (`nemo-core` unless noted) | Owner | Used by |
|---|---|---|---|
| 1 | `PlayerCommand::{SetEffects, SetTransition}`, `PlayerEvent::{EffectsChanged, TransitionStarted, TransitionSkipped}`, `PlayerSnapshot.{effects, transition}` | audio-effects | listen-together (sends `SetTransition` with mode Off while a room is live), nemo-ui |
| 2 | `effects.rs`: `EffectsSettings`, `EqSettings`, `EqFilter`, `FilterKind`, `DelaySettings`, `ReverbSettings`, `PreampMode`, `EffectsStatus`, `TransitionMode`, `SkipFade`, `TransitionSettings`, `TransitionKind`, `TransitionSkipReason` | audio-effects | nemo-app, nemo-ui |
| 3 | `ProcessingStage::{Equalizer { .. }, Delay, Reverb, Crossfade, Clipped, ReplayGain}` (`ReplayGain` is new) | audio-effects | local-library (ReplayGain), output-devices (no stage in exclusive mode), nemo-ui signal path |
| 4 | `GaplessPolicy` (trait) | audio-effects | local-library implements it; nemo-audio calls it |
| 5 | `output.rs`: `DeviceId`, `OutputDevice`, `SavedDevice`, `OutputOptions`, `OutputStatus`, `FallbackReason`; `PlayerCommand::{ListOutputDevices, SetOutputDevice, SetOutputOptions, RetryOutput, ResumeOnReturnedDevice}`; `PlayerEvent::{OutputDevicesChanged, OutputChanged, OutputOpenFailed, DeviceLost, SavedDeviceReturned, OutputTaken, ExclusiveFellBack}`; `PlayerSnapshot.output` | output-devices | audio-effects (`TransitionSkipped`), nemo-app. cast uses `CastDeviceId` (row 41), not `DeviceId` |
| 6 | `PlayerCommand::SwitchAudioTrack` | account-actions | video-playback |
| 7 | `PlayerCommand::{Park, Unpark { at }}`, `PlayerEvent::ParkedChanged` | cast | nemo-audio, nemo-app, integrations hub (reads the remote position while parked) |
| 8 | `PlayerSnapshot.live` | youtube-extras | nemo-ui |
| 9 | `Origin::SoundCloud`, `Track.source_url`, `MediaUrl` | soundcloud | listening-stats (`origin`), listen-together and cast (refuse or route), offline-cache (YouTube only), nemo-audio `prepare` |
| 10 | `Transport`, `ResolvedStream.{transport, preview}`, `ByteSource::is_seekable` | soundcloud | youtube-extras (live HLS), nemo-audio |
| 11 | `url_resolver.rs`: `UrlResolver`, `MediaQuery`, `MediaEntry`, `EntryKind`, `Site`, `FormatPolicy`, `FormatCandidate`, `rank_formats`, `ResolverState` | soundcloud | youtube-extras (live), nemo-ytm implements |
| 12 | `HlsSource` (`nemo-ytm/hls`, not `nemo-core`) | soundcloud | youtube-extras |
| 13 | `StreamRequest`, `AudioTrackInfo`, `StreamResolver::resolve_with`, `ResolvedStream.{audio_language, audio_tracks}` | account-actions | cast (codec preference), offline-cache (Premium), video-playback (language) |
| 14 | Decoders ALAC, Vorbis, AIFF and Opus (`nemo-audio`); `Codec::Opus` already exists in `audio.rs`; `supported_stream_codecs()` | local-library | soundcloud and account-actions (rankers wait for it), youtube-extras |
| 15 | File `ByteSource` (`FileSource` in `nemo-audio`) | local-library | offline-cache |
| 16 | `library.rs`: `LocalTrackLookup`, `LocalPlayback`, `TrackSpan`, `ReplayGainMode`, `ReplayGainValues`, `LocalTrackDetails`, `TrackStatus`, `LoudnessMeter`, `LocalLibraryService`, `LocalPlaylist`, `PlaylistEntry`, `TagEdit` | local-library | nemo-audio `prepare`, playlist-import and listening-stats (save to a local playlist) |
| 17 | `downloads.rs`: `DownloadService`, `OfflineStore`, `NetworkMode`, `CachedAudio`, `SaveRequest`, `SaveState`, `StorageReport`, `DownloadEvent` | offline-cache | cast (refuses saved songs), account-actions (actions offline), nemo-app |
| 18 | `LyricsService::keep_offline` | offline-cache | nemo-lyrics implements; lyrics-plus |
| 19 | `import.rs`: `PlaylistImport`, `ImportSession`, `ImportRow`, `MatchCandidate`, `Confidence` | playlist-import | nemo-ui, nemo-app |
| 20 | `catalog.rs`: `SearchScope`, `VideoSummary`, `ChannelSummary`, `ChannelDetail`, `ChannelRef`, `SearchResults.{videos, channels}`, `Catalog::{search_scoped, channel, live_streams}` | youtube-extras | soundcloud (maps results), nemo-ui |
| 21 | `CommentSource`, `Comment`, `CommentPage`, `CommentError`, `VideoRatings`, `Dislikes`, `Backend.{comments, ratings}` | youtube-extras | nemo-ui |
| 22 | `captions.rs`: `SubtitleSource`, `SubtitleCue` (new) | youtube-extras | video-playback (subtitles), lyrics-plus (captions provider in the chain) |
| 23 | `Backend.soundcloud` | soundcloud | nemo-app |
| 24 | `SecretStore`, `StoreError`, `MAX_SECRET_BYTES`, `is_valid_key`, `MemoryStore`; crate `nemo-secrets` | account-actions | integrations, ai-features, nemo-spotify (lyrics-plus), nemo-ytm |
| 25 | `AccountId`, `AccountList`, `AccountEntry`, `PremiumStatus`, `AccountSwitcher`, `MAX_ACCOUNTS` | account-actions | nemo-ui, youtube-extras (account-specific data) |
| 26 | `SignOutWipe`, `WipeScope`, `WipeError` | account-actions | offline-cache, youtube-extras, ai-features, lyrics-plus (Spotify), listen-together, cast. Not registered by listening-stats (Decision 3) |
| 27 | `LikeStatus`, `Catalog::set_rating`, `CatalogError::{Rejected, RateLimited}`, `PlaylistEditor` (with `create`, new), `PlaylistEditInfo`, `Privacy`, `SetVideoId` | account-actions | playlist-import, nemo-ui |
| 28 | `releases.rs`: `ReleaseFeed`, `ReleaseNotice` | account-actions | integrations (OS notifications) |
| 29 | `notify.rs`: `OsNotifier`, `OsNotification`, `NotifierState`, `NotificationEvent` (new) | integrations | account-actions (release notices) |
| 30 | `hooks.rs`: `PlayRule`, `PlayTracker`, `PlayHookEvent`, `FinishedPlay`, `PlayObserver` | integrations | listening-stats, offline-cache (`record_play`) |
| 31 | `integrations.rs`: `IntegrationsService`, `LastFmStatus`, `NativeWindow`, `NoIntegrations`, `Backend.{integrations, notifier}` | integrations | nemo-ui, nemo-app |
| 32 | `stats.rs`: `StatsService`, `StatsPeriod`, `RankedSong`, `RankedName`, `HourClock`, `YearSummary`, `Recap`, `Receipt`, `NoStats`, `Backend.stats` | listening-stats | ai-features (taste input), nemo-ui |
| 33 | `ai.rs`: `AiService`, `Translator`, `AiRequest`, `PayloadPreview`, `AiAnswer`, `AiError`, `NoAi`, `NoTranslator`, `Backend.{ai, translator}` | ai-features | lyrics-plus (translation), nemo-ui |
| 34 | `lyrics_plus.rs`: `OwnLyrics`, `OwnLyricsDraft`, `Romanizer`, `RomanLang`, `Romanization`, `LyricsTranslation`, `LangTag`, `Translation`, `LineMeta`, `LyricsService::line_meta` | lyrics-plus | now-playing-styles (`LineMeta`), nemo-ui |
| 35 | `spotify.rs`: `SpotifyAccount`, `SpotifyState` | lyrics-plus | now-playing-styles (Canvas), nemo-app |
| 36 | `animated_art.rs`: `AnimatedArtSource`, `ArtQuery`, `AnimatedArt`, `ArtLookup`, `ArtSourceInfo` | lyrics-plus | now-playing-styles |
| 37 | `video.rs`: `VideoFrame`, `VideoFrameSource`, `VideoController`, `VideoState`, `VideoInfo`, `VideoQuality`, `ResolvedVideo`, `SubtitleTrack`, `VideoError`, `VideoStreamResolver`, `ScreenWake` | video-playback | nemo-ui, youtube-extras (picture of a video result) |
| 38 | `PlaybackClock` | video-playback | nemo-audio implements it; nemo-video reads it |
| 39 | `ClipPlayer` (silent looping clip over the same engine; new) | video-playback | now-playing-styles |
| 40 | `recognize.rs`: `Recognizer`, `InputSource`, `RecognizedSong`, `RecognitionState`, `CancelFlag`, `HistoryEntry`, `RecognizeError` | song-recognition | nemo-ui, nemo-app |
| 41 | `remote.rs`: `RemotePlayback`, `RemoteSlot`, `RemoteLease`, `RemoteKind`, `RoomRole`, `RemoteMode`, `RemoteStatus`, `CommandGate`, `RemoteBusy`; `cast.rs`: `CastControl`, `CastDevice`, `CastDeviceId` (renamed from `DeviceId`), `DeviceKind`, `CastEvent`, `Discovery` | cast | listen-together (`RemotePlayback`, `RoomRole`), nemo-app (gate), integrations hub |
| 42 | `together.rs`: `RoomControl`, `RoomStatus`, `Member`, `MemberId`, `RoomEvent`, `LinkState`, `RoomError` | listen-together | nemo-ui, nemo-app |
| 43 | `opt_in.rs`: `OptInInfo` (renamed from `IntegrationInfo`; replaces `AiProviderInfo`, `ArtSourceInfo` and the risk note of `ProviderInfo::opt_in` in the settings screens; see "Opt-in settings pattern") | integrations | every design in the pattern table, nemo-ui `OptInRow` |

Result: 43 rows. Every item has one owner. Rows 3 (`ReplayGain`), 22, 27 (`create`), 29 and 39 were gaps that two designs relied on and nobody defined. Two items were defined under two names and were merged: `DeviceId` (output-devices wins, cast uses `CastDeviceId`) and the translator contract (ai-features `Translator` wins, lyrics-plus calls it).

Four rules from the map:
- `ResolvedStream` gets four new fields from two designs (`transport`, `preview` from soundcloud; `audio_language`, `audio_tracks` from account-actions), and `Track` gets `source_url`. They land in one task that adds all of them with `Default`, so no later task edits every struct literal again.
- Two rankers exist on purpose: `rank_audio` (account-actions) picks among YouTube Music formats; `rank_formats` (soundcloud) picks among `yt-dlp` formats of other sites and live streams. They do not merge. The owner's listening test sets the soundcloud factors.
- Every `PlayerCommand` addition is handled in `nemo-audio/src/control_cmd.rs`. Commands from rows 1, 5, 6 and 7 are independent, so their tasks may land in any order, but only one task at a time edits `player.rs`.
- Position and play counting while parked or casting: `PlayTracker` reads `RemotePlayback::remote_status().position` instead of `PlayerControl::position()` when the mode is not `Local`.

## Opt-in settings pattern
One pattern for every feature that sends data off this computer, reaches a new host, or records what the user plays. It replaces the separate texts and structs the designs invented (`IntegrationInfo`, `AiProviderInfo`, `ArtSourceInfo`, `ProviderInfo::opt_in`).

1. **Off by default.** The switch is off on a new install. Nothing is requested, built or started while it is off. The only exception is OS media controls, which send nothing off the computer (integrations, criterion 9).
2. **One data type, `OptInInfo`, in `nemo-core/src/opt_in.rs`** (owner: integrations, renamed from `IntegrationInfo`): `id`, `title`, `sends` (plain text: what leaves the computer), `hosts` (exact host names, or "the endpoint you set" and "the server you type" when the user supplies it), `risk` (plain text), `version` (a number), `default_on` (false except media controls). Services return it; `nemo-ui` never knows a service by name.
3. **One component, `OptInRow`, in `nemo-ui/src/gui/views/opt_in.rs`.** It shows the title, the switch, then "What is sent", "To" and "Risk" in plain text. Turning the switch on does not turn it on yet: a panel with the same three texts opens, with "Turn on" and "Cancel", neither pre-selected, reachable by keyboard.
4. **Shown before the first request.** The confirmation is stored as `confirmed_version`. `nemo-app` builds the service only when the switch is on and `confirmed_version == version`. A new host or new data raises `version`, so the panel shows again before the next request. A test with a counting fake host proves zero requests while the switch is off or unconfirmed.
5. **Risk text names the real risk.** Unofficial access: "may breach the service's terms and the account can be restricted". Public data: where it shows (Last.fm profile, Discord profile). Text sent to a third party: whose terms apply. Local screen text: who can read it (notifications). Never "safe", never a promise about the other side.
6. **Turning off is one click, no panel.** Requests stop at once. The row says what stays on this computer and offers "Delete now" where data is kept.
7. **After turn-on** the row shows the hosts in use and one status line ("Connected", "yt-dlp missing"). Every `hosts` entry also appears in the README egress list with "only when <feature> is on". A test in `nemo-app` compares the two lists.
8. **User-supplied hosts** (AI endpoint, Listen Together server) show the typed host in the panel, and a different host asks again.
9. **Accessibility.** Keyboard order, visible focus, no animation under reduced motion, readable at 200% text scale (`design.md` checklist).

Where each design points here (one line in its "Steering and README changes"):

| Feature | Sent | Hosts | Design |
|---|---|---|---|
| SoundCloud | search text and page URLs, through `yt-dlp` | `soundcloud.com`, `api-v2.soundcloud.com`, `*.sndcdn.com`, `playback.media-streaming.soundcloud.cloud` | soundcloud |
| Smart downloads | background fetches of songs played often, and takedown checks | `www.youtube.com`, `*.googlevideo.com` | offline-cache |
| Premium audio, release notices, OS release notifications | the session to `yt-dlp`; the followed-artist check; (notifications: nothing leaves) | `www.youtube.com`, `music.youtube.com`, `*.googlevideo.com` | account-actions, integrations |
| All of YouTube search, comments, dislike counts, YouTube captions | queries, comment text, video id | `www.youtube.com`, `returnyoutubedislikeapi.com` | youtube-extras |
| Spotify lyrics, animated covers | the user's `sp_dc` session, track title and artist | `accounts.spotify.com`, `open.spotify.com`, `spclient.wg.spotify.com`, `api-partner.spotify.com`, the Canvas host (unverified) | lyrics-plus, now-playing-styles |
| AI provider, AI translation | seed songs, taste data, lyric lines | the endpoint you set | ai-features, lyrics-plus |
| Last.fm, Discord presence | plays and titles; the title and artist to the local Discord app | `ws.audioscrobbler.com` (and `www.last.fm` in the browser); none for Discord | integrations |
| Song recognition | an audio fingerprint; then the matched title and artist as a YouTube Music search | `amp.shazam.com`, `music.youtube.com` | song-recognition |
| Listen Together | display name, room code, track ids, titles | the server you type | listen-together |
| Cast | the playing stream, to a device on the local network | none on the internet | cast |
| Listening stats | nothing (recording is local) | none | listening-stats |

## Consequences
- `nemo-ytm` loses its credential module to `nemo-secrets` and `nemo-core`; its sign-in flow is otherwise unchanged.
- The crate count grows from 8 to 18. Each new crate is small and testable alone.
- `nemo-app` gets more wiring. If it passes 500 lines per file, it splits as `code.md` asks.
