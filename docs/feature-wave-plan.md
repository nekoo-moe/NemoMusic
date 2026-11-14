# Feature wave plan: build order

Generated from the 17 `tasks.md` files of the 2026-11-14 wave: 417 tasks in 26 steps. Done: 47 of 417 (a task is done when its `tasks.md` line is ticked).
A step starts when every task of the step before it has merged. Tasks inside one step do not depend on each other and never edit the same `nemo-core` file, so they may run in parallel (at most a few at a time on the build machine). Steps of the same dependency depth are split only by those file clashes.
Each task is one PR (`workflow.md` section 3). Regenerate this file when a `tasks.md` changes.

## Step 1 (23 of 37 done)
- [x] `account-actions#1` (wave 0, nemo-core) Move the `SecretStore` trait into `nemo-core` - nemo-core: lib, secret_store
- [x] `account-actions#4` (wave 0, nemo-core) Add `SignOutWipe`, `WipeScope` and `WipeError` - nemo-core: account
- [x] `account-actions#7` (wave 0, nemo-core) Add the rating and playlist-edit types to the catalog - nemo-core: catalog
- [x] `account-actions#8` (wave 0, nemo-core) Add `StreamRequest`, `AudioTrackInfo` and `resolve_with` - nemo-core: stream
- [x] `ai-features#1` (wave 0, nemo-core) Add the `AiService` contract - nemo-core: ai
- [x] `cast#4` (wave 0, nemo-core) Add the parked player mode (Park and Unpark) - nemo-core: player
- [x] `integrations#1` (wave 0, nemo-core) Add `OptInInfo`, the shared opt-in data type - nemo-core: opt_in
- [x] `integrations#4` (wave 0, nemo-core) Add `PlayRule`, the shared play-count rule - nemo-core: hooks
- [x] `integrations#6` (wave 0, nemo-core) Add the `OsNotifier` contract - nemo-core: notify
- [x] `local-library#1` (wave 0, nemo-core) Add the local playback types to `nemo-core` - nemo-core: library
- [x] `local-library#3` (wave 0, nemo-audio) Make `FileSource` the shared file reader
- [x] `lyrics-plus#1` (wave 0, nemo-core) Add `LineMeta` and the defaulted `LyricsService::line_meta` - nemo-core: lyrics_plus
- [x] `offline-cache#1` (wave 0, nemo-core) Add the downloads types to `nemo-core` - nemo-core: downloads
- [x] `offline-cache#2` (wave 0, nemo-core) Add `LyricsService::keep_offline` with a default - nemo-core: services
- [x] `offline-cache#3` (wave 0, nemo-app) Add `AppPaths.offline`
- [x] `soundcloud#1` (wave 0, nemo-core) Add `Origin::SoundCloud` and `MediaUrl` to nemo-core - nemo-core: audio, track
- [x] `soundcloud#6` (wave 0, nemo-soundcloud) Create the `nemo-soundcloud` crate skeleton
- [ ] `video-playback#1` (wave 0, nemo-video) Spike: libmpv on Windows and Linux (own binding over `libloading`)
- [x] `youtube-extras#5` (wave 0, nemo-core) Add the defaulted `LyricsProvider::timeout` and honor it in the chain - nemo-core: lyrics
- [ ] `account-actions#12` (wave 1, nemo-ytm) Spike: several accounts, one session each
- [ ] `account-actions#13` (wave 1, nemo-ytm) Spike: the Premium session for `yt-dlp`
- [ ] `account-actions#14` (wave 1, nemo-ytm) Spike: check the `ytmapi-rs` edit queries
- [ ] `account-actions#15` (wave 1, nemo-ytm) Spike: the cheapest release check
- [ ] `cast#1` (wave 1, ) Spike: cast a googlevideo stream to a real Chromecast and a real DLNA speaker
- [ ] `integrations#11` (wave 1, ) Spike: Discord IPC crate
- [ ] `integrations#12` (wave 1, ) Spike: Windows toast from an unpackaged build
- [ ] `listen-together#1` (wave 1, ) Spike: talk to a self-hosted metroserver
- [x] `local-library#4` (wave 1, nemo-audio) Spike: choose the Opus decoder
- [x] `local-library#5` (wave 1, nemo-audio) Decode ALAC, Vorbis and AIFF
- [ ] `lyrics-plus#8` (wave 1, ) Spike: Spotify sign-in, token and lyrics with an owner test account
- [ ] `lyrics-plus#9` (wave 1, ) Spike: share-card text shaping on `tiny-skia` and `cosmic-text`
- [x] `now-playing-styles#2` (wave 1, nemo-ui) Store the style and the animated-cover switches
- [ ] `playlist-import#2` (wave 1, nemo-library) Spike: ISRC search and the real export formats
- [ ] `song-recognition#1` (wave 1, ) Spike: does the service answer without a location or time zone
- [ ] `soundcloud#7` (wave 1, nemo-ytm) Spike: what `yt-dlp` returns for public SoundCloud tracks, and whether `symphonia` reads AAC HLS
- [x] `youtube-extras#6` (wave 1, nemo-ytm) Spike: YouTube captions through `yt-dlp`
- [x] `youtube-extras#7` (wave 1, nemo-ytm) Spike: live streams and the innertube calls

## Step 2 (4 of 4 done)
- [x] `account-actions#11` (wave 0, nemo-core) Add `ReleaseFeed` and `ReleaseNotice` - nemo-core: lib, releases
- [x] `soundcloud#3` (wave 0, nemo-core) Add `ByteSource::is_seekable` and forward it through `MediaAdapter` - nemo-core: services
- [x] `youtube-extras#1` (wave 0, nemo-core) Add search scope, video and channel types and the defaulted `Catalog` methods - nemo-core: catalog
- [x] `youtube-extras#3` (wave 0, nemo-core) Add `PlayerSnapshot.live` - nemo-core: player

## Step 3 (1 of 1 done)
- [x] `audio-effects#1` (wave 0, nemo-core) Add the effects settings types and `GaplessPolicy` to `nemo-core` - nemo-core: effects, lib

## Step 4 (1 of 1 done)
- [x] `cast#2` (wave 0, nemo-core) Add the shared remote-playback types to nemo-core - nemo-core: lib, remote

## Step 5 (1 of 1 done)
- [x] `playlist-import#1` (wave 0, nemo-core) Add `import.rs` with the import types and the `PlaylistImport` trait - nemo-core: import, lib

## Step 6 (0 of 1 done)
- [ ] `youtube-extras#4` (wave 0, nemo-core) Add `captions.rs` with `SubtitleSource`, `SubtitleCue` and `CaptionError` - nemo-core: captions, lib

## Step 7 (17 of 35 done)
- [x] `account-actions#2` (wave 0, nemo-secrets) Add the `nemo-secrets` crate with the platform stores
- [x] `account-actions#5` (wave 0, nemo-ytm) Run the registered wipe steps on sign-out
- [x] `account-actions#6` (wave 0, nemo-core) Add the account list types and `AccountSwitcher` - nemo-core: account
- [x] `account-actions#9` (wave 0, nemo-audio) Resolve through a `StreamRequest` in `nemo-audio`
- [x] `audio-effects#2` (wave 0, nemo-core) Add the new `ProcessingStage` values - nemo-core: audio
- [ ] `audio-effects#3` (wave 0, nemo-core) Add the effects and transition commands, events and snapshot fields - nemo-core: player
- [ ] `cast#3` (wave 0, nemo-core) Add the Cast control trait and its types to nemo-core - nemo-core: cast, lib
- [x] `integrations#2` (wave 0, nemo-ui) Add the shared `OptInRow` settings component
- [x] `integrations#3` (wave 0, nemo-app) Add the opt-in gate and the egress-list check in `nemo-app`
- [x] `integrations#5` (wave 0, nemo-core) Add `PlayTracker`, `PlayHookEvent`, `FinishedPlay` and `PlayObserver` - nemo-core: hooks
- [x] `integrations#7` (wave 0, nemo-core) Add `IntegrationsService` and the `Backend` fields - nemo-core: integrations, services
- [x] `local-library#2` (wave 0, nemo-core) Add the library service types to `nemo-core` - nemo-core: library
- [x] `lyrics-plus#2` (wave 0, nemo-core) Add the own-lyrics contract - nemo-core: lyrics_plus
- [x] `lyrics-plus#5` (wave 0, nemo-core) Add the Spotify account contract - nemo-core: spotify
- [ ] `soundcloud#2` (wave 0, nemo-core) Add `Transport`, the four new `ResolvedStream` fields and `Track.source_url` in one change - nemo-core: stream, track
- [ ] `audio-effects#5` (wave 1, nemo-audio) Parse and write AutoEq profile files
- [x] `local-library#6` (wave 1, nemo-audio) Add the Opus decoder through one codec registry
- [ ] `now-playing-styles#1` (wave 1, ) Spike: Spotify Canvas lookup with an owner test account
- [x] `offline-cache#4` (wave 1, nemo-ytm) Create the offline index and file layout
- [x] `offline-cache#8` (wave 1, nemo-lyrics) Pin saved lyrics in the lyrics cache
- [ ] `playlist-import#3` (wave 1, nemo-library) Read Exportify and Soundiiz CSV files in nemo-library
- [ ] `playlist-import#4` (wave 1, nemo-library) Normalize text and score candidates
- [ ] `youtube-extras#8` (wave 1, nemo-ytm) Add the pure caption functions: `parse_json3`, `cues_to_doc`, `pick_track`
- [ ] `account-actions#18` (wave 2, nemo-ytm) Like and dislike through `set_rating`
- [ ] `account-actions#19` (wave 2, nemo-ytm) Edit playlist details on YouTube Music
- [ ] `account-actions#27` (wave 2, nemo-ytm) Choose the audio track by preferred language
- [ ] `audio-effects#7` (wave 2, nemo-audio) Add the biquad filters and the automatic preamp
- [ ] `audio-effects#8` (wave 2, nemo-audio) Add the delay stage
- [ ] `audio-effects#9` (wave 2, nemo-audio) Add the reverb stage
- [ ] `audio-effects#14` (wave 2, nemo-audio) Build the `TransitionMixer`
- [ ] `integrations#26` (wave 2, nemo-integrations) Compose release notifications as plain text
- [x] `local-library#19` (wave 2, nemo-audio) Play CUE spans in the player
- [ ] `lyrics-plus#31` (wave 2, nemo-ui) Render the share card
- [x] `now-playing-styles#4` (wave 2, nemo-ui) Dispatch the Now Playing views on the style
- [ ] `youtube-extras#12` (wave 2, nemo-ytm) Search all of YouTube

## Step 8 (0 of 2 done)
- [ ] `listen-together#2` (wave 0, nemo-core) Add the room control trait and its types to nemo-core - nemo-core: lib, together
- [ ] `soundcloud#5` (wave 0, nemo-core) Add `Backend.soundcloud` - nemo-core: services

## Step 9 (0 of 1 done)
- [ ] `output-devices#1` (wave 0, nemo-core) Add the output device types to `nemo-core` - nemo-core: lib, output

## Step 10 (0 of 1 done)
- [ ] `song-recognition#2` (wave 0, nemo-core) Add the Recognizer trait and its types to nemo-core - nemo-core: lib, recognize

## Step 11 (0 of 1 done)
- [ ] `video-playback#2` (wave 0, nemo-core) Add the video types and traits - nemo-core: lib, video

## Step 12 (0 of 44 done)
- [ ] `account-actions#3` (wave 0, nemo-secrets) Add `ScopedStore` to `nemo-secrets`
- [ ] `account-actions#10` (wave 0, nemo-core) Add `PlayerCommand::SwitchAudioTrack` - nemo-core: player
- [ ] `cast#5` (wave 0, nemo-cast) Create the nemo-cast crate skeleton
- [ ] `integrations#8` (wave 0, nemo-integrations) Create the `nemo-integrations` crate skeleton
- [ ] `integrations#9` (wave 0, nemo-app) Add the play hub thread in `nemo-app`
- [ ] `listening-stats#1` (wave 0, nemo-core) Add the `StatsService` contract and `Backend.stats` - nemo-core: services, stats
- [ ] `listen-together#3` (wave 0, nemo-together) Create the nemo-together crate and validate server addresses
- [ ] `lyrics-plus#3` (wave 0, nemo-core) Add the romanizer contract - nemo-core: lyrics_plus
- [ ] `lyrics-plus#6` (wave 0, nemo-core) Add the animated-art source contract - nemo-core: animated_art
- [ ] `lyrics-plus#7` (wave 0, nemo-spotify) Create the `nemo-spotify` crate skeleton
- [ ] `output-devices#2` (wave 0, nemo-audio) Extend the sink traits for health, reporting and device lists
- [ ] `song-recognition#3` (wave 0, nemo-recognize) Create the nemo-recognize crate skeleton
- [ ] `soundcloud#4` (wave 0, nemo-core) Add `url_resolver.rs` with the `UrlResolver` trait and `rank_formats` - nemo-core: lib, url_resolver
- [ ] `video-playback#3` (wave 0, nemo-core) Add `PlaybackClock` and implement it in `nemo-audio` - nemo-core: video
- [ ] `account-actions#17` (wave 1, nemo-ytm) Add the release notice store
- [ ] `audio-effects#4` (wave 1, nemo-app) Save the effects and transition settings in `settings.json`
- [ ] `audio-effects#6` (wave 1, nemo-audio) Add the built-in presets and the preset store
- [ ] `local-library#7` (wave 1, nemo-audio) Let streams play as Opus
- [ ] `local-library#8` (wave 1, nemo-library) Create the `library.db` store with migrations
- [ ] `lyrics-plus#10` (wave 1, nemo-lyrics) Store own lyrics in the lyrics database
- [ ] `offline-cache#5` (wave 1, nemo-ytm) Clean up the cache at start
- [ ] `offline-cache#6` (wave 1, nemo-ytm) Remove songs and wipe the cache
- [ ] `offline-cache#7` (wave 1, nemo-ytm) Download one song with ranged reads
- [ ] `output-devices#5` (wave 1, nemo-app) Save the chosen device by ID and name
- [ ] `playlist-import#5` (wave 1, nemo-library) Implement `ImportEngine` and the match session
- [ ] `soundcloud#10` (wave 1, nemo-ytm) Add `HlsSource` for on-demand HLS in nemo-ytm
- [ ] `video-playback#6` (wave 1, nemo-video) Add the `nemo-video` crate with the libmpv binding
- [ ] `video-playback#8` (wave 1, nemo-ytm) Resolve the video stream and the caption list
- [ ] `youtube-extras#9` (wave 1, nemo-ytm) Implement `YtmCaptions` over the `yt-dlp` runner
- [ ] `account-actions#20` (wave 2, nemo-ytm) Create playlists and add, remove and move songs
- [ ] `audio-effects#10` (wave 2, nemo-audio) Build the `EffectsChain` with ramps and the clip counter
- [ ] `integrations#27` (wave 2, nemo-integrations) Show notifications on Windows
- [ ] `integrations#28` (wave 2, nemo-integrations) Show notifications on Linux
- [ ] `local-library#21` (wave 2, nemo-audio) Apply ReplayGain as an off-by-default stage
- [ ] `local-library#22` (wave 2, nemo-audio) Measure loudness with `ebur128`
- [ ] `local-library#25` (wave 2, nemo-core) Keep the lossless badge honest for local formats - nemo-core: audio
- [ ] `lyrics-plus#28` (wave 2, nemo-ui) Build the lyrics editor model
- [ ] `lyrics-plus#32` (wave 2, nemo-ui) Save the card as PNG or copy it to the clipboard
- [ ] `now-playing-styles#5` (wave 2, nemo-ui) Add the Classic style
- [ ] `now-playing-styles#6` (wave 2, nemo-ui) Add the Apple Music style layout and its `design.md` exceptions
- [ ] `offline-cache#13` (wave 2, nemo-audio) Play saved songs from the cache
- [ ] `offline-cache#15` (wave 2, nemo-ytm) Add offline mode with the offline catalog and request gates
- [ ] `youtube-extras#13` (wave 2, nemo-ytm) Open channel pages
- [ ] `video-playback#17` (wave 3, nemo-ui) Draw frames with `VideoSurface`

## Step 13 (0 of 1 done)
- [ ] `video-playback#5` (wave 0, nemo-core) Add the video settings - nemo-core: services

## Step 14 (0 of 1 done)
- [ ] `youtube-extras#2` (wave 0, nemo-core) Add comment and dislike types, `CommentSource`, `VideoRatings` and the `Backend` fields - nemo-core: catalog, services

## Step 15 (0 of 48 done)
- [ ] `ai-features#2` (wave 0, nemo-core) Add the `Translator` contract and the `Backend` fields - nemo-core: ai, services
- [ ] `listening-stats#2` (wave 0, nemo-stats) Create the `nemo-stats` crate skeleton
- [ ] `lyrics-plus#4` (wave 0, nemo-core) Add the lyrics-translation contract - nemo-core: lyrics_plus
- [ ] `output-devices#3` (wave 0, nemo-core) Add the output commands, events and snapshot field - nemo-core: player
- [ ] `video-playback#4` (wave 0, nemo-core) Add `ClipPlayer` and `ClipInfo` - nemo-core: video
- [ ] `account-actions#16` (wave 1, nemo-ytm) Store one session per account and migrate the old keys
- [ ] `cast#6` (wave 1, nemo-cast) Serve the cast file from a LAN-only HTTP server
- [ ] `cast#8` (wave 1, nemo-cast) Vendor cast_channel.proto and add the Cast V2 frame codec
- [ ] `cast#9` (wave 1, nemo-cast) Store pinned device certificates
- [ ] `integrations#10` (wave 1, ) Spike: OS media controls with `souvlaki`
- [ ] `integrations#13` (wave 1, nemo-integrations) Add the integrations database
- [ ] `listen-together#5` (wave 1, nemo-together) Store the display-name blocklist
- [ ] `local-library#9` (wave 1, nemo-library) Rebuild a damaged library file
- [ ] `local-library#10` (wave 1, nemo-library) Load the library from the store at start
- [ ] `lyrics-plus#11` (wave 1, nemo-lyrics) Keep line metadata and provider romanization through the cache
- [ ] `lyrics-plus#14` (wave 1, nemo-spotify) Open `spotify.sqlite` and keep `sp_dc` in the credential store
- [ ] `output-devices#4` (wave 1, nemo-audio) Spike: match `cpal` device IDs to WASAPI endpoint IDs
- [ ] `output-devices#6` (wave 1, nemo-audio) List devices with IDs and report health in the `cpal` sink
- [ ] `song-recognition#4` (wave 1, nemo-recognize) Port the signature format and the Hanning window from SongRec
- [ ] `song-recognition#6` (wave 1, nemo-recognize) Store the recognition history
- [ ] `song-recognition#7` (wave 1, nemo-recognize) Capture a microphone into a ring buffer
- [ ] `soundcloud#8` (wave 1, nemo-ytm) Implement `YtDlpUrlResolver::list` and `state`
- [ ] `soundcloud#11` (wave 1, nemo-ytm) Add seek by segment to `HlsSource`, and the fMP4 join if the spike needs it
- [ ] `soundcloud#12` (wave 1, nemo-ytm) Add the live window to `HlsSource`
- [ ] `video-playback#7` (wave 1, nemo-video) Render frames into a pool of three
- [ ] `video-playback#9` (wave 1, nemo-ytm) Find the music video of a song
- [ ] `youtube-extras#10` (wave 1, nemo-ytm) Add `CaptionsLyricsProvider` and register it last in the lyrics chain
- [ ] `audio-effects#11` (wave 2, nemo-audio) Run the chain in the audio callback and apply `SetEffects`
- [ ] `cast#10` (wave 2, nemo-cast) Discover Cast devices over mDNS while the picker is open
- [ ] `integrations#17` (wave 2, nemo-app) Report the remote position while parked or casting
- [ ] `integrations#19` (wave 2, nemo-integrations) Add the signed Last.fm API client
- [ ] `integrations#29` (wave 2, nemo-app) Notify new releases from the release check
- [ ] `local-library#15` (wave 2, nemo-library) Store and edit local playlists
- [ ] `lyrics-plus#29` (wave 2, nemo-ui) Preview the draft with the lyrics-view line finder
- [ ] `now-playing-styles#7` (wave 2, nemo-ui) Compute the word fill fraction
- [ ] `now-playing-styles#10` (wave 2, nemo-ui) Draw the Apple backdrop
- [ ] `now-playing-styles#11` (wave 2, nemo-ui) Add the cover slot with still covers
- [ ] `offline-cache#9` (wave 2, nemo-ytm) Queue and pace downloads on one worker
- [ ] `offline-cache#16` (wave 2, nemo-app) Gate the other services that call YouTube hosts
- [ ] `offline-cache#19` (wave 2, nemo-ytm) Wipe the cache on sign-out
- [ ] `output-devices#8` (wave 2, nemo-audio) Carry the decoders out of a stopped pipeline
- [ ] `playlist-import#6` (wave 2, nemo-app) Wire the import engine and saving to a local playlist in nemo-app
- [ ] `video-playback#10` (wave 2, nemo-video) Keep the picture on the audio clock
- [ ] `youtube-extras#15` (wave 2, nemo-ytm) Read comments (posting comes later)
- [ ] `youtube-extras#16` (wave 2, nemo-ytm) Add the Return YouTube Dislike client
- [ ] `integrations#31` (wave 3, ) Package the notification entry points
- [ ] `lyrics-plus#37` (wave 3, nemo-ui) Add line selection and the share preview
- [ ] `video-playback#24` (wave 3, ) Ship libmpv with the app

## Step 16 (0 of 42 done)
- [ ] `ai-features#3` (wave 0, nemo-ai) Create the `nemo-ai` crate skeleton
- [ ] `cast#7` (wave 1, nemo-cast) Relay a playing stream through the LAN server
- [ ] `listening-stats#3` (wave 1, nemo-stats) Add the play log database
- [ ] `listening-stats#4` (wave 1, nemo-stats) Add the cross-source song key
- [ ] `listening-stats#5` (wave 1, nemo-stats) Add calendar period bounds
- [ ] `listen-together#4` (wave 1, nemo-together) Vendor listentogether.proto and add the envelope codec
- [ ] `lyrics-plus#12` (wave 1, nemo-lyrics) Implement `LyricsService::keep_offline` with a pinned flag
- [ ] `now-playing-styles#3` (wave 1, nemo-spotify) Add the Canvas cache and its miss table
- [ ] `song-recognition#5` (wave 1, nemo-recognize) Port the fingerprint algorithm from SongRec
- [ ] `song-recognition#8` (wave 1, nemo-recognize) Capture this computer's sound (loopback)
- [ ] `soundcloud#9` (wave 1, nemo-ytm) Implement `YtDlpUrlResolver::resolve` with format ranking
- [ ] `soundcloud#13` (wave 1, nemo-ytm) Add `YtmStreamOpener` and use it in place of `HttpStreamOpener`
- [ ] `account-actions#21` (wave 2, nemo-ytm) Switch between signed-in accounts
- [ ] `audio-effects#12` (wave 2, nemo-audio) Let delay and reverb tails ring out
- [ ] `cast#11` (wave 2, nemo-cast) Discover DLNA renderers over SSDP while the picker is open
- [ ] `cast#12` (wave 2, nemo-cast) Run a Cast V2 session with the Default Media Receiver
- [ ] `integrations#14` (wave 2, nemo-integrations) Publish playback to the OS media session and handle media keys
- [ ] `integrations#20` (wave 2, nemo-integrations) Connect Last.fm through the browser
- [ ] `integrations#24` (wave 2, nemo-integrations) Show the playing song as Discord presence
- [ ] `local-library#11` (wave 2, nemo-library) Scan folders incrementally with one writer
- [ ] `local-library#16` (wave 2, nemo-library) Import and export M3U and M3U8
- [ ] `lyrics-plus#15` (wave 2, nemo-lyrics) Add the romanization frame and script detection
- [ ] `lyrics-plus#20` (wave 2, nemo-lyrics) Use own lyrics before the provider chain
- [ ] `lyrics-plus#22` (wave 2, nemo-signin) Sign in to Spotify through `nemo-signin --site spotify` or cookie paste
- [ ] `lyrics-plus#30` (wave 2, nemo-ui) Close the timing-offset gaps
- [ ] `now-playing-styles#8` (wave 2, nemo-ui) Draw the word-by-word fill and line roles
- [ ] `now-playing-styles#12` (wave 2, nemo-ui) Add the animated-cover lookup controller
- [ ] `offline-cache#10` (wave 2, nemo-ytm) Enforce the size limit and free space
- [ ] `offline-cache#11` (wave 2, nemo-ytm) Save cover, metadata, lyrics and segments with each song
- [ ] `output-devices#7` (wave 2, nemo-audio) Open the chosen device through a new `DeviceSinkFactory` (shared mode)
- [ ] `output-devices#12` (wave 2, nemo-audio) Pack `f32` samples into device bytes
- [ ] `output-devices#13` (wave 2, nemo-audio) Negotiate the exclusive format
- [ ] `playlist-import#7` (wave 2, nemo-app) Save an import as a YouTube Music playlist
- [ ] `song-recognition#10` (wave 2, nemo-recognize) Send the recognition request
- [ ] `soundcloud#15` (wave 2, nemo-soundcloud) Implement `SoundcloudCatalog`
- [ ] `video-playback#11` (wave 2, nemo-video) Start and stop video at the audio position
- [ ] `video-playback#13` (wave 2, nemo-video) Play silent looping clips
- [ ] `youtube-extras#17` (wave 2, nemo-ytm) Register the "ytm-extras" sign-out step
- [ ] `audio-effects#19` (wave 3, nemo-ui) Build the equalizer screen
- [ ] `integrations#30` (wave 3, nemo-integrations) Add the notification opt-in text and status line
- [ ] `playlist-import#8` (wave 3, nemo-ui) Add the file pick and matching progress screens
- [ ] `youtube-extras#24` (wave 3, nemo-ytm) Post, reply and like comments (later task, owner decision)

## Step 17 (0 of 38 done)
- [ ] `ai-features#4` (wave 1, nemo-ai) Validate the endpoint URL
- [ ] `ai-features#5` (wave 1, nemo-ai) Add the AI settings and readings database
- [ ] `listen-together#6` (wave 1, nemo-together) Open the room socket
- [ ] `lyrics-plus#13` (wave 1, nemo-lyrics) Add the `lyrics_translation` table
- [ ] `youtube-extras#11` (wave 1, nemo-ytm) Play live streams through the shared HLS source
- [ ] `account-actions#22` (wave 2, nemo-ytm) Add a second account
- [ ] `account-actions#24` (wave 2, nemo-ytm) Hand the session to `yt-dlp` without exposing it
- [ ] `account-actions#28` (wave 2, nemo-ytm) Check for new releases at a calm pace
- [ ] `ai-features#8` (wave 2, nemo-ai) Parse answers strictly
- [ ] `audio-effects#13` (wave 2, nemo-audio) Fade on a manual skip
- [ ] `cast#13` (wave 2, nemo-cast) Run a UPnP AVTransport session
- [ ] `integrations#15` (wave 2, nemo-integrations) Give the OS media session the cover art
- [ ] `integrations#16` (wave 2, nemo-ui) Hand the native window to the media controls
- [ ] `integrations#21` (wave 2, nemo-integrations) Scrobble finished plays through the queue
- [ ] `listening-stats#6` (wave 2, nemo-stats) Record counted plays from the play hub
- [ ] `local-library#12` (wave 2, nemo-library) Read all tag fields and mark unplayable files
- [ ] `lyrics-plus#16` (wave 2, nemo-lyrics) Romanize Japanese (Hepburn)
- [ ] `lyrics-plus#17` (wave 2, nemo-lyrics) Romanize Korean, Russian, Ukrainian and Greek
- [ ] `lyrics-plus#18` (wave 2, nemo-lyrics) Romanize Mandarin (Pinyin) and Cantonese (Jyutping)
- [ ] `lyrics-plus#21` (wave 2, nemo-lyrics) Put YouTube captions last in the chain
- [ ] `lyrics-plus#23` (wave 2, nemo-spotify) Get the access token and the Spotify track id
- [ ] `now-playing-styles#9` (wave 2, nemo-ui) Show duets and background vocals
- [ ] `now-playing-styles#14` (wave 2, nemo-ui) Play the loop in the cover slot
- [ ] `offline-cache#12` (wave 2, nemo-ytm) Implement `DownloadService`
- [ ] `offline-cache#14` (wave 2, nemo-ytm) Answer segments and covers from the cache
- [ ] `output-devices#9` (wave 2, nemo-audio) Switch device during playback
- [ ] `output-devices#14` (wave 2, nemo-audio) Run the exclusive render loop against a backend trait
- [ ] `song-recognition#9` (wave 2, nemo-recognize) Turn the captured audio into signatures
- [ ] `soundcloud#14` (wave 2, nemo-audio) Resolve SoundCloud tracks in `prepare`
- [ ] `video-playback#12` (wave 2, nemo-video) Drop the video before the audio under load
- [ ] `video-playback#14` (wave 2, nemo-app) Wire video into `nemo-app`
- [ ] `audio-effects#20` (wave 3, nemo-ui) Import, export and manage presets and AutoEq profiles in the screen
- [ ] `integrations#18` (wave 3, nemo-ui) Add the OS media controls row in Settings
- [ ] `integrations#25` (wave 3, nemo-ui) Add the Discord presence row in Settings
- [ ] `lyrics-plus#38` (wave 3, nemo-ui) Add the landscape full-screen lyrics layout
- [ ] `lyrics-plus#39` (wave 3, nemo-lyrics) Romanize Thai (Later)
- [ ] `now-playing-styles#16` (wave 3, nemo-ui) Add the style cards and animated-cover rows in Settings > Now Playing
- [ ] `playlist-import#9` (wave 3, nemo-ui) Add the review screen

## Step 18 (0 of 32 done)
- [ ] `account-actions#23` (wave 2, nemo-ytm) Sign out of one account or all
- [ ] `account-actions#25` (wave 2, nemo-ytm) Resolve Premium audio with the active session
- [ ] `ai-features#6` (wave 2, nemo-ai) Build the exact payload in `prepare`
- [ ] `ai-features#7` (wave 2, nemo-ai) Add the HTTP client for the chat endpoint
- [ ] `audio-effects#15` (wave 2, nemo-audio) Mix transitions in the feeder
- [ ] `cast#14` (wave 2, nemo-cast) Start and stop a cast session in CastService
- [ ] `integrations#22` (wave 2, nemo-integrations) Handle Last.fm rejections and disconnect
- [ ] `listening-stats#7` (wave 2, nemo-stats) Record SoundCloud plays with their source
- [ ] `listening-stats#8` (wave 2, nemo-stats) Answer the chart queries
- [ ] `listening-stats#10` (wave 2, nemo-stats) Export the user's plays to a file
- [ ] `listen-together#7` (wave 2, nemo-together) Map server time to local time
- [ ] `listen-together#8` (wave 2, nemo-together) Build a fake metroserver for tests
- [ ] `local-library#13` (wave 2, nemo-library) Read ReplayGain, R128 and gapless tags
- [ ] `local-library#14` (wave 2, nemo-library) Parse CUE sheets into tracks
- [ ] `local-library#17` (wave 2, nemo-library) Edit tags without risking the file
- [ ] `lyrics-plus#19` (wave 2, nemo-lyrics) Romanize Arabic, Persian, Hebrew and Hindi as Partial
- [ ] `lyrics-plus#24` (wave 2, nemo-spotify) Add the Spotify lyrics provider
- [ ] `lyrics-plus#26` (wave 2, nemo-lyrics) Implement the lyrics-translation hook
- [ ] `now-playing-styles#13` (wave 2, nemo-spotify) Add the Spotify Canvas source
- [ ] `offline-cache#17` (wave 2, nemo-ytm) Count plays and keep the most-played songs
- [ ] `offline-cache#18` (wave 2, nemo-ytm) Check saved songs for takedowns
- [ ] `output-devices#10` (wave 2, nemo-audio) Watch for device changes
- [ ] `output-devices#15` (wave 2, nemo-audio) Open WASAPI exclusive mode on Windows
- [ ] `song-recognition#11` (wave 2, nemo-recognize) Implement ShazamRecognizer
- [ ] `soundcloud#16` (wave 2, nemo-app) Wire SoundCloud in nemo-app behind the opt-in switch
- [ ] `video-playback#15` (wave 2, nemo-app) Keep the screen awake while video plays
- [ ] `video-playback#16` (wave 2, nemo-ui) Pick subtitle cues by the audio clock
- [ ] `youtube-extras#14` (wave 2, nemo-ytm) List live stations and streams
- [ ] `audio-effects#23` (wave 3, nemo-ui) Credit the AutoEq project
- [ ] `now-playing-styles#17` (wave 3, ) Take the style snapshots and update the UI docs
- [ ] `playlist-import#10` (wave 3, nemo-ui) Add the save step and drop the session afterwards
- [ ] `video-playback#18` (wave 3, nemo-ui) Add the Video switch to Now Playing

## Step 19 (0 of 27 done)
- [ ] `account-actions#26` (wave 2, nemo-ytm) Rank YouTube audio formats by what can play
- [ ] `account-actions#29` (wave 2, nemo-core) Wire the account settings and services in `nemo-app` - nemo-core: services
- [ ] `ai-features#9` (wave 2, nemo-ai) Save, test and remove the provider
- [ ] `audio-effects#16` (wave 2, nemo-audio) Plan transitions in control and apply `SetTransition`
- [ ] `cast#15` (wave 2, nemo-cast) Route player commands and the queue to the device
- [ ] `cast#16` (wave 2, nemo-cast) Pick the stream route per track
- [ ] `listening-stats#9` (wave 2, nemo-stats) Add the year in music and monthly recaps
- [ ] `listening-stats#12` (wave 2, nemo-stats) Draw the listening receipt image
- [ ] `listen-together#9` (wave 2, nemo-together) Create, join, approve and leave a room
- [ ] `local-library#18` (wave 2, nemo-library) Implement `LocalLibraryService` and `LocalTrackLookup`
- [ ] `lyrics-plus#25` (wave 2, nemo-app) Wire Spotify into the app and the sign-out wipe
- [ ] `lyrics-plus#27` (wave 2, nemo-app) Wire own lyrics, the romanizer and translation into the UI backend
- [ ] `offline-cache#20` (wave 2, nemo-app) Wire the offline cache into the app
- [ ] `output-devices#11` (wave 2, nemo-audio) Pause on device loss and offer to resume on return
- [ ] `output-devices#16` (wave 2, nemo-audio) Choose exclusive or shared in the factory and fall back
- [ ] `song-recognition#12` (wave 2, nemo-app) Wire recognition into nemo-app
- [ ] `youtube-extras#18` (wave 2, nemo-app) Wire the new services in nemo-app behind their switches
- [ ] `audio-effects#24` (wave 3, nemo-audio) Measure the callback cost on the reference machine
- [ ] `integrations#23` (wave 3, nemo-ui) Add the Last.fm row in Settings
- [ ] `listening-stats#13` (wave 3, nemo-ui) Add the Stats page with the recording switch and charts
- [ ] `playlist-import#11` (wave 3, ) Document the import and run the manual checks
- [ ] `soundcloud#17` (wave 3, nemo-ui) Show the SoundCloud result group, the "Preview" tag and the badge in the UI
- [ ] `soundcloud#19` (wave 3, nemo-ui) Add the SoundCloud settings entry
- [ ] `video-playback#19` (wave 3, nemo-ui) Add the quality menu and the video line
- [ ] `video-playback#20` (wave 3, nemo-ui) Add the subtitle menu and overlay
- [ ] `video-playback#21` (wave 3, nemo-ui) Add full screen
- [ ] `video-playback#23` (wave 3, nemo-ui) Add the "Keep screen awake" setting

## Step 20 (0 of 40 done)
- [ ] `account-actions#30` (wave 2, nemo-app) Send one grouped OS notification per release check
- [ ] `ai-features#10` (wave 2, nemo-ai) Send confirmed requests and implement `Translator`
- [ ] `audio-effects#17` (wave 2, nemo-audio) Choose the smart transition per song pair and honour gapless marks
- [ ] `cast#17` (wave 2, nemo-app) Wire Cast into nemo-app
- [ ] `listening-stats#11` (wave 2, nemo-stats) Clear all stats
- [ ] `listen-together#10` (wave 2, nemo-together) Handle host handover, kicks and blocks
- [ ] `listen-together#11` (wave 2, nemo-together) Follow the room as a guest
- [ ] `listen-together#13` (wave 2, nemo-together) Send the host's playback to the room
- [ ] `listen-together#14` (wave 2, nemo-together) Reconnect after a dropped connection
- [ ] `local-library#20` (wave 2, nemo-library) Join CUE tracks and gapless albums without a fade
- [ ] `local-library#23` (wave 2, nemo-library) Run the opt-in loudness scan in the background
- [ ] `now-playing-styles#15` (wave 2, nemo-app) Wire the animated-art sources into the app
- [ ] `output-devices#17` (wave 2, nemo-audio) Keep the exclusive path bit-perfect and apply the output options
- [ ] `output-devices#18` (wave 2, nemo-audio) Reopen the exclusive device when the song format changes
- [ ] `account-actions#31` (wave 3, nemo-ui) Add the dislike button
- [ ] `account-actions#32` (wave 3, nemo-ui) Add the playlist edit menu
- [ ] `account-actions#34` (wave 3, nemo-ui) Add the account switcher
- [ ] `account-actions#35` (wave 3, nemo-ui) Add the Premium audio setting
- [ ] `account-actions#36` (wave 3, nemo-ui) Add the audio language setting and the track switch
- [ ] `audio-effects#22` (wave 3, nemo-ui) Show effects and transitions in the signal path
- [ ] `integrations#32` (wave 3, ) Document integrations in the README
- [ ] `integrations#33` (wave 3, ) Run the manual checks
- [ ] `listening-stats#14` (wave 3, nemo-ui) Add the listening clock chart
- [ ] `listening-stats#15` (wave 3, nemo-ui) Add receipt and export commands
- [ ] `lyrics-plus#33` (wave 3, nemo-ui) Add the lyrics editor screen
- [ ] `lyrics-plus#34` (wave 3, nemo-ui) Show romanized lines, the toggle and the settings
- [ ] `lyrics-plus#35` (wave 3, nemo-ui) Add translation to the lyrics view and Settings > Lyrics
- [ ] `lyrics-plus#36` (wave 3, nemo-ui) Add the Spotify and YouTube captions rows in Settings > Lyrics
- [ ] `offline-cache#21` (wave 3, nemo-ui) Add "Save offline" and the saved mark
- [ ] `offline-cache#22` (wave 3, nemo-ui) Add the storage view with the limit and removal
- [ ] `offline-cache#24` (wave 3, nemo-ui) Add the offline switch and shortcut
- [ ] `offline-cache#25` (wave 3, nemo-ytm) Add the smart-downloads opt-in row
- [ ] `song-recognition#13` (wave 3, nemo-recognize) Add the recognition entry to settings
- [ ] `song-recognition#14` (wave 3, nemo-ui) Add the Recognize button and the Listening state
- [ ] `soundcloud#18` (wave 3, nemo-ui) Add SoundCloud track, playlist and user pages
- [ ] `youtube-extras#19` (wave 3, nemo-ui) Add the scope switch, video and channel results and the channel page to the UI
- [ ] `youtube-extras#20` (wave 3, nemo-ui) Add the Live area and the live player state
- [ ] `youtube-extras#21` (wave 3, nemo-ui) Add the read-only comments panel
- [ ] `youtube-extras#22` (wave 3, nemo-ui) Show the dislike label
- [ ] `youtube-extras#23` (wave 3, nemo-ui) Add the four settings entries

## Step 21 (0 of 23 done)
- [ ] `ai-features#11` (wave 2, nemo-ai) Wire `nemo-ai` and its sign-out step
- [ ] `audio-effects#18` (wave 2, nemo-audio) Skip the crossfade on a format change in exclusive mode
- [ ] `listen-together#12` (wave 2, nemo-together) Correct guest drift by seeking
- [ ] `local-library#24` (wave 2, nemo-app) Wire the library into the app
- [ ] `output-devices#19` (wave 2, nemo-audio) Release an idle exclusive device and recover when another app takes it
- [ ] `account-actions#33` (wave 3, nemo-ui) Add, remove and move songs from the playlist view
- [ ] `account-actions#37` (wave 3, nemo-ui) Add the release notice settings and list
- [ ] `audio-effects#21` (wave 3, nemo-ui) Add the delay, reverb and crossfade controls
- [ ] `cast#18` (wave 3, nemo-cast) Add the Cast entry to settings
- [ ] `cast#19` (wave 3, nemo-ui) Build the device picker
- [ ] `cast#20` (wave 3, nemo-ui) Show casting on the player bar and in the signal path
- [ ] `cast#22` (wave 3, ) Document Cast and the local network in the README
- [ ] `listening-stats#16` (wave 3, nemo-ui) Add the year in music and recap views
- [ ] `listening-stats#18` (wave 3, nemo-ui) Add "Clear stats" and the sign-out text
- [ ] `lyrics-plus#40` (wave 3, ) Check egress, docs and the live Spotify path
- [ ] `now-playing-styles#18` (wave 3, nemo-app) Turn on animated covers once `nemo-video` ships
- [ ] `offline-cache#23` (wave 3, nemo-ui) Add the download queue controls
- [ ] `offline-cache#26` (wave 3, ) Write the offline-cache risk text in the README
- [ ] `output-devices#20` (wave 3, nemo-ui) Build the output device screen
- [ ] `song-recognition#15` (wave 3, nemo-ui) Show the result card and Find in NemoMusic
- [ ] `soundcloud#20` (wave 3, nemo-core) Run the listening test and the manual checks - nemo-core: url_resolver
- [ ] `video-playback#22` (wave 3, nemo-ui) Change the audio language from the video view
- [ ] `youtube-extras#25` (wave 3, nemo-ui) Add the post, reply and like actions to the comments panel

## Step 22 (0 of 17 done)
- [ ] `listen-together#15` (wave 2, nemo-together) Implement TogetherClient with the command gate
- [ ] `local-library#26` (wave 2, nemo-app) Hand opened files to the running app
- [ ] `account-actions#38` (wave 3, ) Run the account manual checks
- [ ] `ai-features#12` (wave 3, nemo-ui) Add the "AI provider" settings entry
- [ ] `audio-effects#25` (wave 3, ) Listen for clicks and check transitions by ear
- [ ] `cast#21` (wave 3, nemo-ui) Show cast failures, hints and the hand-back
- [ ] `listening-stats#17` (wave 3, nemo-ui) Save a recap as a playlist
- [ ] `local-library#27` (wave 3, nemo-ui) Add the folder list with the system dialog
- [ ] `local-library#28` (wave 3, nemo-ui) Show the library with scan progress and track status
- [ ] `local-library#29` (wave 3, nemo-ui) Add local playlist screens
- [ ] `local-library#31` (wave 3, nemo-ui) Add the tag editor with a confirm step
- [ ] `local-library#32` (wave 3, nemo-ui) Add ReplayGain settings and show the gain stage
- [ ] `offline-cache#27` (wave 3, ) Run the manual checks
- [ ] `output-devices#21` (wave 3, nemo-ui) Show output messages and the output line of the signal path
- [ ] `song-recognition#16` (wave 3, nemo-ui) Add the recognition history view
- [ ] `video-playback#25` (wave 3, ) Run the video manual checks
- [ ] `youtube-extras#26` (wave 3, ) Run the manual checks

## Step 23 (0 of 9 done)
- [ ] `listen-together#16` (wave 2, nemo-app) Wire Listen Together into nemo-app
- [ ] `ai-features#13` (wave 3, nemo-ui) Add the feature switches and the payload preview dialog
- [ ] `cast#23` (wave 3, ) Manual checks on real devices
- [ ] `listening-stats#19` (wave 3, ) Document stats and run the manual checks
- [ ] `local-library#30` (wave 3, nemo-ui) Add M3U import and export to the playlist screens
- [ ] `local-library#33` (wave 3, nemo-ui) Offer play, queue or playlist for dropped files
- [ ] `local-library#34` (wave 3, ) Update the README for the local library
- [ ] `output-devices#22` (wave 3, ) Test the output path on the owner's reference hardware
- [ ] `song-recognition#17` (wave 3, ) Manual checks

## Step 24 (0 of 6 done)
- [ ] `ai-features#14` (wave 3, nemo-ui) Add "Suggest songs"
- [ ] `ai-features#15` (wave 3, nemo-ui) Add "Read my taste"
- [ ] `listen-together#17` (wave 3, nemo-together) Add the Listen Together entry to settings
- [ ] `listen-together#18` (wave 3, nemo-ui) Add the create and join dialogs
- [ ] `listen-together#20` (wave 3, nemo-ui) Show the live room on the player bar
- [ ] `local-library#35` (wave 3, ) Run the manual checks on Windows and Linux Mint

## Step 25 (0 of 3 done)
- [ ] `ai-features#16` (wave 3, nemo-ui) Handle offline and errors in the UI, document, and run manual checks
- [ ] `listen-together#19` (wave 3, nemo-ui) Add the room panel
- [ ] `listen-together#21` (wave 3, ) Describe the room server in the README

## Step 26 (0 of 1 done)
- [ ] `listen-together#22` (wave 3, nemo-together) Interop and listening checks with metroserver

