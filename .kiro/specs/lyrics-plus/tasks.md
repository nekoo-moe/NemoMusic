## Tasks

Cross-spec ids written `<spec>#?` point at the task of that spec that adds the named item (ADR 0004 Decision 4 row in brackets). Replace `?` with its number once that spec's `tasks.md` is merged.

- [ ] 1. Add `LineMeta` and the defaulted `LyricsService::line_meta`
  - Crate and files: `nemo-core`: new `src/lyrics_plus.rs`, `src/services.rs`, `src/lib.rs`. Touches nemo-core/src/lyrics_plus.rs, services.rs and lib.rs.
  - Does: Adds `LineMeta { singer, background, roman }` and `line_meta` with an empty default, so no implementor changes. Signatures as in the design.
  - _Requirements: none (groundwork)_
  - Depends on: none
  - Wave: 0
  - Checks: rustdoc on every item; `cargo test -p nemo-core`.
  - Steering: `structure.md` lists `nemo-core/src/lyrics_plus.rs` (one edit, covers tasks 2 to 4 too).

- [ ] 2. Add the own-lyrics contract
  - Crate and files: `nemo-core`: `src/lyrics_plus.rs`. Touches nemo-core/src/lyrics_plus.rs.
  - Does: Adds `OWN_LYRICS`, `DraftLine`, `OwnLyricsDraft`, `OwnLyricsError` (`thiserror`) and the `OwnLyrics` trait.
  - _Requirements: none (groundwork)_
  - Depends on: lyrics-plus#1
  - Wave: 0
  - Checks: `cargo test -p nemo-core`.

- [ ] 3. Add the romanizer contract
  - Crate and files: `nemo-core`: `src/lyrics_plus.rs`. Touches nemo-core/src/lyrics_plus.rs.
  - Does: Adds `RomanLang` (`#[non_exhaustive]`, all 12 languages), `RomanQuality`, `RomanScheme`, `RomanOptions`, `Romanization` and the `Romanizer` trait.
  - _Requirements: none (groundwork)_
  - Depends on: lyrics-plus#2
  - Wave: 0
  - Checks: `cargo test -p nemo-core`.

- [ ] 4. Add the lyrics-translation contract
  - Crate and files: `nemo-core`: `src/lyrics_plus.rs`. Touches nemo-core/src/lyrics_plus.rs.
  - Does: Adds `LangTag`, `TranslationAvailability`, `Translation`, `TranslationError` (wraps `AiError`) and the `LyricsTranslation` trait, using `PayloadPreview` and `AiError` of ai-features.
  - _Requirements: none (groundwork)_
  - Depends on: lyrics-plus#3, ai-features#? (`ai.rs`, row 33)
  - Wave: 0
  - Checks: `cargo test -p nemo-core`.

- [ ] 5. Add the Spotify account contract
  - Crate and files: `nemo-core`: new `src/spotify.rs`, `src/lib.rs`. Touches nemo-core/src/spotify.rs and lib.rs.
  - Does: Adds `SpotifyState`, `SpotifyError` (`thiserror`: `SignedOut`, `TokenRefused`, `Http(status)`, `Parse`, `Store`) and the `SpotifyAccount` trait. `sign_in_cookie` takes `Secret<String>`.
  - _Requirements: none (groundwork)_
  - Depends on: lyrics-plus#1 (lib.rs order)
  - Wave: 0
  - Checks: a test that `Debug` of every new type prints no secret.
  - Steering: `structure.md` lists `nemo-core/src/spotify.rs`.

- [ ] 6. Add the animated-art source contract
  - Crate and files: `nemo-core`: new `src/animated_art.rs`, `src/lib.rs`. Touches nemo-core/src/animated_art.rs and lib.rs.
  - Does: Adds `ArtSourceId`, `ArtQuery`, `AnimatedArt`, `ArtLookup`, `ArtSourceInfo`, `AnimatedArtError` (`SignedOut`, `Offline`, `Refused(status)`, `Unsupported`, `TooLarge`, `Store`) and `AnimatedArtSource`, as now-playing-styles needs them. No Apple Music variant.
  - _Requirements: none (groundwork)_
  - Depends on: lyrics-plus#5 (lib.rs order)
  - Wave: 0
  - Checks: `cargo test -p nemo-core`.
  - Steering: `structure.md` lists `nemo-core/src/animated_art.rs`.

- [ ] 7. Create the `nemo-spotify` crate skeleton
  - Crate and files: new `crates/nemo-spotify/` (`Cargo.toml`, `src/lib.rs`), workspace `Cargo.toml` members.
  - Does: Empty crate depending on `nemo-core` only, with `#![forbid(unsafe_code)]`, `#![warn(missing_docs)]` and `[lints] workspace = true`. No network code yet.
  - _Requirements: none (groundwork)_
  - Depends on: lyrics-plus#5
  - Wave: 0
  - Checks: `cargo tree -p nemo-spotify --edges normal` shows only `nemo-core` among `nemo-*` crates.
  - Steering: `structure.md`: `nemo-spotify` crate entry and the edge `nemo-app -> nemo-spotify`.

- [ ] 8. Spike: Spotify sign-in, token and lyrics with an owner test account
  - Crate and files: probe in `$KIROCREW_SCRATCH` (not merged); result in `.kiro/specs/lyrics-plus/design.md` Risks.
  - Does: Proves, with the owner's throwaway Spotify account: (a) `nemo-signin` on Spotify's page yields `sp_dc`; (b) `sp_dc` gives a web-player access token (note whether a computed code is now needed); (c) a title and artist search gives a track id; (d) `spclient.wg.spotify.com/color-lyrics/v2/track/{id}` returns `LINE_SYNCED` lines. Records the exact hosts and response shapes and writes hand-made fixtures with no real token or lyric text.
  - _Requirements: none (groundwork)_
  - Depends on: none
  - Wave: 1
  - Checks: needs the owner (throwaway Spotify account). Pass: a to d all work. Fail on (b) or (d): the provider ships disabled; tasks 23, 24 and 36 keep the code behind an off switch and the settings row says "Unavailable"; task 22 still ships sign-in only if the owner wants it for Canvas. Fail on (a): cookie paste only; task 22 drops `--site spotify`. Unconfirmed hosts are removed from tasks 22 to 24.

- [ ] 9. Spike: share-card text shaping on `tiny-skia` and `cosmic-text`
  - Crate and files: probe in `$KIROCREW_SCRATCH`; result in `.kiro/specs/lyrics-plus/design.md` Risks.
  - Does: Proves that `cosmic-text` 0.15.0 with system fallback fonts shapes Japanese, Korean, Chinese, Arabic, Hebrew and Hindi onto a `tiny-skia` 0.11.4 pixmap on Windows and Linux, with no `unsafe` and no environment variable.
  - _Requirements: none (groundwork)_
  - Depends on: none
  - Wave: 1
  - Checks: Pass: no tofu boxes on a clean Windows 11 and a default Linux desktop. Fail: task 31 bundles a licensed fallback font in `assets/fonts/` (with `LICENSES.md`) or shows a short notice for unsupported scripts.

- [ ] 10. Store own lyrics in the lyrics database
  - Crate and files: `nemo-lyrics`: new `src/own.rs`, `src/cache.rs` (`LyricsCache::init` creates `own_lyrics`).
  - Does: Implements `OwnLyrics` (`get`, `save`, `save_copy`, `delete`). `save` refuses a missing start with `MissingStart` and never spreads lines; it returns `Line`, or `Word` when every line has words. `clear()` keeps `own_lyrics`, as it keeps `lyrics_offset`.
  - _Requirements: 6, 7, 9, 10_
  - Depends on: lyrics-plus#2
  - Wave: 1
  - Checks: migration test on an existing database; `clear()` keeps own lyrics; `save_copy` leaves the provider row unchanged; save refuses a line with no start.

- [ ] 11. Keep line metadata and provider romanization through the cache
  - Crate and files: `nemo-lyrics`: `src/ttml.rs` (stop skipping `x-roman`), `src/netease.rs` (`romalrc`), `src/cache.rs` (store `LineMeta`), `src/service.rs` (`line_meta`).
  - Does: Singer, background-vocal and provider romanization facts survive the cache and are returned by `LyricsService::line_meta`, indexed like `LyricsDoc.lines`.
  - _Requirements: 14_
  - Depends on: lyrics-plus#1, lyrics-plus#10 (cache.rs order)
  - Wave: 1
  - Checks: TTML fixture with `x-roman`, a duet and a background line round-trips through the cache; old rows without meta return an empty list.

- [ ] 12. Implement `LyricsService::keep_offline` with a pinned flag
  - Crate and files: `nemo-lyrics`: `src/cache.rs`, `src/service.rs`.
  - Does: Pinned rows skip expiry. Spotify rows are still deleted on Spotify or NemoMusic sign-out; own lyrics are never cleared.
  - _Requirements: none (groundwork; offline-cache uses it)_
  - Depends on: lyrics-plus#11 (cache.rs order), offline-cache#? (`LyricsService::keep_offline`, row 18)
  - Wave: 1
  - Checks: pinned row survives expiry; a Spotify-row delete removes pinned Spotify rows. If offline-cache's tasks also list this implementation, keep only one.

- [ ] 13. Add the `lyrics_translation` table
  - Crate and files: `nemo-lyrics`: new `src/translate.rs` (storage part), `src/cache.rs` (`init` creates the table).
  - Does: Stores and reads one translation per track key and language, and forgets them. Local only; never exported.
  - _Requirements: 39_
  - Depends on: lyrics-plus#4, lyrics-plus#12 (cache.rs order)
  - Wave: 1
  - Checks: migration test; round-trip; `forget` removes all languages of a track.

- [ ] 14. Open `spotify.sqlite` and keep `sp_dc` in the credential store
  - Crate and files: `nemo-spotify`: `src/lib.rs` (`SpotifySession::open`, `wipe`), new `src/store.rs` (migrations, table `track_map`).
  - Does: `open` takes the app data folder, `Arc<dyn SecretStore>` and the helper path. Only `sp_dc` is kept, in `SecretStore`, wrapped in `Secret<String>`. `wipe` deletes the secret and every `spotify.sqlite` row.
  - _Requirements: 33_
  - Depends on: lyrics-plus#7, lyrics-plus#8, account-actions#? (`SecretStore`, row 24)
  - Wave: 1
  - Checks: migration test; wipe test finds no `sp_dc` and no `track_map` row; `Debug` of `SpotifySession` prints no secret.
  - Steering: `tech.md` row only if the crate adds `rusqlite` or `reqwest` features not yet listed for it.

- [ ] 15. Add the romanization frame and script detection
  - Crate and files: `nemo-lyrics`: new `src/romanize.rs`, `src/romanize/script.rs`.
  - Does: Implements `Romanizer` with no language yet: finds the script of each run with `unicode-script`, picks the language (kana means Japanese, і ї є ґ mean Ukrainian, پ چ ژ گ mean Persian, Han-only follows `RomanOptions.han`), romanizes only runs of enabled languages, and prefers the provider's romanization from `LineMeta.roman` with `from_provider = true`. No network dependency.
  - _Requirements: 12, 14, 16_
  - Depends on: lyrics-plus#3, lyrics-plus#11
  - Wave: 2
  - Checks: proptest: never panics, keeps `lines.len()`, leaves Latin text as written; `cargo tree -p nemo-lyrics` adds no HTTP crate for this module.
  - Steering: `tech.md` row `unicode-script` =0.5.9 (MIT OR Apache-2.0).

- [ ] 16. Romanize Japanese (Hepburn)
  - Crate and files: `nemo-lyrics`: new `src/romanize/japanese.rs`; app licence file gets the IPADIC notice.
  - Does: Uses `sabiyomi` =0.1.0, particles as pronounced. Footer label "Romanized: Hepburn".
  - _Requirements: 11, 17_
  - Depends on: lyrics-plus#15
  - Wave: 2
  - Checks: golden tests on made-up phrases (no real lyrics); `cargo deny check`; record the release size growth (about 49 MB expected). Needs the owner to check the IPADIC licence text (not on the `code.md` allowlist; `deny.toml` exception with reason).
  - Steering: `tech.md` row `sabiyomi` =0.1.0 (MIT); `deny.toml` exception for IPADIC.

- [ ] 17. Romanize Korean, Russian, Ukrainian and Greek
  - Crate and files: `nemo-lyrics`: new `src/romanize/korean.rs`, `src/romanize/cyrillic.rs`, `src/romanize/greek.rs`.
  - Does: Uses `transliteratio` =0.1.0 tables `rr`, `bgn` (ru), `uk` and `elot`. Korean is letter-wise with jamo position rules and carries no "Partial" label, as criterion 11 lists none for it.
  - _Requirements: 11, 17_
  - Depends on: lyrics-plus#15
  - Wave: 2
  - Checks: golden tests per language; a Korean golden test records the missing sound-change rules (for example 신라) so a later fix shows up.
  - Steering: `tech.md` row `transliteratio` =0.1.0 (MIT OR Apache-2.0).

- [ ] 18. Romanize Mandarin (Pinyin) and Cantonese (Jyutping)
  - Crate and files: `nemo-lyrics`: new `src/romanize/chinese.rs`.
  - Does: `pinyin` =0.11.0 with tone marks (most common reading for polyphones); `rust-canto` =0.3.4 for Jyutping. `RomanOptions.han` picks one, default Mandarin.
  - _Requirements: 11, 17_
  - Depends on: lyrics-plus#15
  - Wave: 2
  - Checks: golden tests for both; `cargo deny check` (the `zstd` 0.13 duplicate). Needs the owner to check the `rust-canto` data licence.
  - Steering: `tech.md` rows `pinyin` =0.11.0 (MIT) and `rust-canto` =0.3.4 (MIT).

- [ ] 19. Romanize Arabic, Persian, Hebrew and Hindi as Partial
  - Crate and files: `nemo-lyrics`: new `src/romanize/arabic.rs`, `src/romanize/hebrew.rs`, `src/romanize/hindi.rs`.
  - Does: `transliteratio` tables `bgn` (ar, fa), `heb` and `hunterian`, all `RomanQuality::Partial`; the footer says "Romanized: BGN/PCGN (letters only)" and the like. Hindi compares `hunterian` with `lipilekhika` =1.2.0 on schwa deletion and keeps the better one.
  - _Requirements: 11, 17_
  - Depends on: lyrics-plus#17
  - Wave: 2
  - Checks: golden tests per language; label test for Partial.
  - Steering: `tech.md` row `lipilekhika` =1.2.0 (MIT) only if Hindi picks it.

- [ ] 20. Use own lyrics before the provider chain
  - Crate and files: `nemo-lyrics`: `src/service.rs`, `src/chain.rs`.
  - Does: `LyricsHub::lookup` returns own lyrics at once, labelled "Your lyrics", and sends no request; deleting them returns the song to the chain.
  - _Requirements: 8_
  - Depends on: lyrics-plus#10, lyrics-plus#11 (service.rs order)
  - Wave: 2
  - Checks: counting fake provider sees zero calls when own lyrics exist.
  - Steering: ADR 0002 gets the own-lyrics rule.

- [ ] 21. Put YouTube captions last in the chain
  - Crate and files: `nemo-lyrics`: `src/chain.rs`; `nemo-app` wiring of the `nemo-ytm` captions provider.
  - Does: "YouTube captions" runs only when no lyrics provider has timed lyrics, is never chosen over one, keeps its own 20 s timeout, and shows "Not lyrics-grade" for auto-generated captions. No word timing is made up. Off by default.
  - _Requirements: 31, 35, 36_
  - Depends on: lyrics-plus#20, youtube-extras#? (captions `LyricsProvider` and `LyricsProvider::timeout`, row 22)
  - Wave: 2
  - Checks: chain test with fake providers: captions lose to every provider and run last; line-timed captions stay line-timed.
  - Steering: ADR 0002 gets the YouTube captions row (default off, risk note).

- [ ] 22. Sign in to Spotify through `nemo-signin --site spotify` or cookie paste
  - Crate and files: `nemo-signin`: `src/main.rs` (`--site youtube|spotify`), `src/cookies.rs` (Spotify URL, keep only `sp_dc`); `nemo-spotify`: new `src/account.rs` (`SpotifyAccount` impl).
  - Does: `sign_in_window` runs the helper on Spotify's own page; `sign_in_cookie` takes a pasted `sp_dc`. The secret goes to `SecretStore` only, never a file or log. `sign_out` calls `wipe`.
  - _Requirements: 33_
  - Depends on: lyrics-plus#8, lyrics-plus#14
  - Wave: 2
  - Checks: cookie filter test keeps only `sp_dc`; `--site youtube` unchanged; log capture shows no cookie. Manual sign-in with the owner's test account.
  - Steering: README egress `accounts.spotify.com`, `open.spotify.com` (only when Spotify lyrics is on); `structure.md`: `nemo-signin` serves YouTube Music and Spotify; ADR 0004 records `nemo-signin --site`.

- [ ] 23. Get the access token and the Spotify track id
  - Crate and files: `nemo-spotify`: new `src/token.rs`, `src/track.rs`.
  - Does: Exchanges `sp_dc` for an access token held in memory only (about one hour, `Secret<String>`); finds the track id by title, artist and duration and caches it in `track_map`. Uses the route the spike proved.
  - _Requirements: 33_
  - Depends on: lyrics-plus#22
  - Wave: 2
  - Checks: hand-written fixtures from the spike (no real token); second lookup of a song sends one request.
  - Steering: README egress for the search host the spike confirmed (for example `api-partner.spotify.com`).

- [ ] 24. Add the Spotify lyrics provider
  - Crate and files: `nemo-spotify`: new `src/lyrics.rs` (`SpotifyLyricsProvider`).
  - Does: Fetches `color-lyrics`, maps it to a line-timed `LyricsDoc`, names "Spotify" and the third-party source it reports. 401 or 403 drops the token and retries once, a second one turns the provider off for the session; 429 keeps `Retry-After`. Returns `OptInInfo` (sends: `sp_dc` session, title and artist; risk: unofficial, may restrict the account).
  - _Requirements: 31, 32, 34_
  - Depends on: lyrics-plus#23, integrations#? (`OptInInfo`, row 43)
  - Wave: 2
  - Checks: `LINE_SYNCED` fixture parses; error mapping tests; counting fake host sees zero requests while off or unconfirmed.
  - Steering: README egress `spclient.wg.spotify.com`; ADR 0002 Spotify row (default off, risk note).

- [ ] 25. Wire Spotify into the app and the sign-out wipe
  - Crate and files: `nemo-app`: wiring module for `SpotifySession`, the provider and the `SignOutWipe` step "spotify".
  - Does: Builds the provider only when its switch is on and `confirmed_version` matches. Spotify sign-out and NemoMusic sign-out delete `sp_dc`, the track map and every cached Spotify lyric, pinned or not. Own lyrics stay.
  - _Requirements: 9, 33_
  - Depends on: lyrics-plus#12, lyrics-plus#24, account-actions#? (`SignOutWipe`, row 26)
  - Wave: 2
  - Checks: wipe test after NemoMusic sign-out finds no Spotify data and keeps `own_lyrics`.

- [ ] 26. Implement the lyrics-translation hook
  - Crate and files: `nemo-lyrics`: `src/translate.rs` (`LyricsTranslation` impl over `Translator`).
  - Does: `availability` reports `NoEndpoint` when no endpoint is set; `prepare` does no I/O; `translate` sends all lines in one `Translator::translate` call, stores the answer and returns it. No retry loop; a failure leaves the cache unchanged.
  - _Requirements: 37, 39, 40_
  - Depends on: lyrics-plus#13, ai-features#? (`Translator`, row 33)
  - Wave: 2
  - Checks: fake `Translator`: zero calls while off, unconfirmed or with no endpoint; one call per song and language; failure keeps the cache.
  - Steering: README egress line "the endpoint you set" names lyrics translation (BETA), if ai-features did not already list it.

- [ ] 27. Wire own lyrics, the romanizer and translation into the UI backend
  - Crate and files: `nemo-app`: backend wiring; `nemo-ui` backend fields for `OwnLyrics`, `Romanizer`, `LyricsTranslation`.
  - Does: `nemo-app` builds them and passes them through `nemo-core` traits only. The UI never depends on `nemo-lyrics` or `nemo-ai`.
  - _Requirements: none (groundwork)_
  - Depends on: lyrics-plus#10, lyrics-plus#19, lyrics-plus#26, ai-features#? (`Backend.translator`, row 33)
  - Wave: 2
  - Checks: `cargo tree -p nemo-ui --edges normal` shows only `nemo-core` among `nemo-*` crates.

- [ ] 28. Build the lyrics editor model
  - Crate and files: `nemo-ui`: new `src/editor.rs` (`EditorModel`).
  - Does: Pure state for the tap pass (start = `PlayerSnapshot.position` at the key event, then next line), step back, re-tap, 50 ms nudge, and the optional word pass. Saving names the first line with no start.
  - _Requirements: 2, 3, 4, 6_
  - Depends on: lyrics-plus#2
  - Wave: 2
  - Checks: pure state tests for tap, step back, re-tap and nudge; no even spreading.

- [ ] 29. Preview the draft with the lyrics-view line finder
  - Crate and files: `nemo-ui`: `src/editor.rs`, `src/lyrics_view.rs` (shared line finding).
  - Does: The preview feeds the unsaved draft through the same code as the lyrics view, with the track offset applied.
  - _Requirements: 5, 29_
  - Depends on: lyrics-plus#28
  - Wave: 2
  - Checks: preview and lyrics view give the same active line for the same position and offset.

- [ ] 30. Close the timing-offset gaps
  - Crate and files: `nemo-ui`: `src/state.rs`, `src/lyric_pieces.rs`.
  - Does: The per-track offset (-10 to +10 s, 50 ms steps, `[` and `]`, `lyrics_offset`) stays when the source changes and is shown; it shifts word timing, romanization and translation lines. No per-source offset.
  - _Requirements: 27, 28, 29_
  - Depends on: lyrics-plus#3, lyrics-plus#4
  - Wave: 2
  - Checks: source switch keeps the offset; romanized and translated lines follow their source line with the offset.

- [ ] 31. Render the share card
  - Crate and files: `nemo-ui`: new `src/share.rs` (`ShareCard`, `render_card`, `ShareError`).
  - Does: Draws a fixed 1080 x 1080 card (lines, title, artist, provider credit, the Unison submitter or "Lyrics via Better Lyrics" when asked) with cover colors. A fifth line is refused before rendering.
  - _Requirements: 18, 19, 22_
  - Depends on: lyrics-plus#9
  - Wave: 2
  - Checks: every ink keeps 4.5:1 on the synthetic covers of the `canvas.rs` tests; fixed size; `TooManyLines` on 5 lines.
  - Steering: `tech.md` rows `tiny-skia` =0.11.4 and `cosmic-text` =0.15.0 as direct `nemo-ui` dependencies.

- [ ] 32. Save the card as PNG or copy it to the clipboard
  - Crate and files: `nemo-ui`: `src/share.rs` (`save_png`, `copy_to_clipboard`).
  - Does: Runs only after an explicit click; the file goes where the user picks. No network.
  - _Requirements: 20_
  - Depends on: lyrics-plus#31
  - Wave: 2
  - Checks: PNG round-trip test; clipboard checked by hand on Windows and Linux.
  - Steering: `tech.md` rows `arboard` =3.7.0 and `rfd` =0.18.0 (shared with other file pickers; skip a row another task already added).

- [ ] 33. Add the lyrics editor screen
  - Crate and files: `nemo-ui`: new `src/gui/views/lyrics_editor.rs`, `src/gui/views/lyrics.rs` ("Edit lyrics" and the no-lyrics entry).
  - Does: Text area for pasted lyrics, tap button and key, step back, re-tap, nudge, word pass, preview, save labelled "Your lyrics" with "Synced by line" or "Synced by word", and delete. Editing fetched lyrics saves a copy.
  - _Requirements: 1, 2, 3, 4, 7, 8, 10_
  - Depends on: lyrics-plus#20, lyrics-plus#27, lyrics-plus#29
  - Wave: 3
  - Checks: `design.md` checklist with screenshot; every control by keyboard; no export or copy-all action.

- [ ] 34. Show romanized lines, the toggle and the settings
  - Crate and files: `nemo-ui`: `src/gui/views/lyrics.rs`, `src/gui/views/settings_lyrics.rs`, `src/lyrics_view.rs`.
  - Does: Romanizes on `spawn_blocking` when a document arrives and caches by track key, provider and scheme; the UI thread only reads. Shows the line under its source line in a smaller, quieter style, timed with it; footer "Romanized: <scheme>" or the provider's label; default off, switch in Settings > Lyrics with per-language choice and the Han option, toggle in the lyrics tools.
  - _Requirements: 13, 14, 15, 17_
  - Depends on: lyrics-plus#15, lyrics-plus#27, lyrics-plus#30
  - Wave: 3
  - Checks: contrast 4.5:1 for the romanized ink; `design.md` checklist with screenshot; no work on the audio thread.

- [ ] 35. Add translation to the lyrics view and Settings > Lyrics
  - Crate and files: `nemo-ui`: `src/gui/views/settings_lyrics.rs`, `src/gui/views/lyrics.rs`.
  - Does: `OptInRow` for "AI translation (BETA)" naming the lyric text, target language and endpoint host; nothing sent before "Turn on". With no endpoint the toggle is hidden and the entry says "Set up an AI endpoint first" with a link to the AI provider entry. Language setting defaults to the UI language. Lines show "AI translation (BETA)" and the language; a failure keeps the lyrics and shows one short line.
  - _Requirements: 37, 38, 39, 40, 42_
  - Depends on: lyrics-plus#27, lyrics-plus#30, integrations#? (`OptInRow`, row 43)
  - Wave: 3
  - Checks: counting fake translator sees zero calls until confirmed; `design.md` checklist with screenshot.

- [ ] 36. Add the Spotify and YouTube captions rows in Settings > Lyrics
  - Crate and files: `nemo-ui`: `src/gui/views/settings_lyrics.rs`.
  - Does: Both rows use `OptInRow` with hosts, risk and finest timing level, default off. The Spotify panel warns: own sign-in, unofficial, the account may be restricted. Sign in by window or cookie paste, sign out, one status line.
  - _Requirements: 31, 32, 34_
  - Depends on: lyrics-plus#21, lyrics-plus#25, integrations#? (`OptInRow`, row 43)
  - Wave: 3
  - Checks: keyboard order and focus; `design.md` checklist with screenshot. If the spike failed, the Spotify row shows "Unavailable".

- [ ] 37. Add line selection and the share preview
  - Crate and files: `nemo-ui`: new `src/gui/views/share_preview.rs`, `src/gui/views/lyrics.rs` (selection).
  - Does: Select up to 4 lines of one song; a fifth shows a short message. "Share as image" opens the preview with Save and Copy. No full-lyrics export, copy-all or own-lyrics text share.
  - _Requirements: 18, 19, 20, 21, 22_
  - Depends on: lyrics-plus#32
  - Wave: 3
  - Checks: keyboard-only flow; `design.md` checklist with screenshot.

- [ ] 38. Add the landscape full-screen lyrics layout
  - Crate and files: `nemo-ui`: `src/gui/views/now_playing.rs` (landscape branch), `src/layout.rs`.
  - Does: When wider than tall: large lyrics with cover, title and artist beside them, offset control and provider footer visible, offset shown as `+0.30 s` when not 0. Active line anchored as in `design.md` section 10, scales up to 200% text; reduced motion switches lines with no scroll; `Esc` leaves and keeps playback and position.
  - _Requirements: 23, 24, 25, 26, 30_
  - Depends on: lyrics-plus#30
  - Wave: 3
  - Checks: snapshots at 1280 x 720 and 1920 x 1080, text 100% and 200%, reduced motion on and off; `design.md` checklist.

- [ ] 39. Romanize Thai (Later)
  - Crate and files: `nemo-lyrics`: new `src/romanize/thai.rs`.
  - Does: Starts only when a readable offline RTGS romanization exists (a crate or a tested rule set with a word list). Until then `RomanLang::Thai` returns no line and Settings shows it as "Later".
  - _Requirements: 11_
  - Depends on: lyrics-plus#15
  - Wave: 3
  - Checks: golden tests; owner approves the crate (`workflow.md` section 8).
  - Steering: `tech.md` row for the chosen crate.

- [ ] 40. Check egress, docs and the live Spotify path
  - Crate and files: `README.md` review, `docs/` notes; no code.
  - Does: Confirms every host from tasks 21 to 26 is in the README egress list with "only when <feature> is on" and in the `OptInInfo` hosts, and runs the app end to end with the owner's test account: sign in, lyrics, sign out wipes Spotify data and keeps own lyrics.
  - _Requirements: 9, 41_
  - Depends on: lyrics-plus#33, lyrics-plus#34, lyrics-plus#35, lyrics-plus#36, lyrics-plus#37, lyrics-plus#38
  - Wave: 3
  - Checks: the `nemo-app` egress-list test (integrations) passes; manual check needs the owner's Spotify test account.

## Definition of Done

See section 4 of [`.kiro/steering/workflow.md`](../../steering/workflow.md#4-definition-of-done). Also for this spec:
- Audio is not touched: `cargo test -p nemo-audio --test bit_exact` stays green. No real-time path is added, so no counting-allocator test.
- No lyrics text leaves the computer except the share image the user saves and the translation the user confirmed; fixtures hold no real lyrics, tokens or cookies.
- Every opt-in source (Spotify, YouTube captions, AI translation) is off on a new install and sends nothing until confirmed.
