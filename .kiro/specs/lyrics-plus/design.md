## Overview

Six pieces, split so each lives in the crate ADR 0004 gives it. Own lyrics, romanization and the translation hook go into `nemo-lyrics`, behind new traits in `nemo-core` that the UI calls. The Spotify sign-in, the Spotify lyrics provider and Canvas go into the new `nemo-spotify` crate. YouTube captions are a `nemo-ytm` provider. The share image, the editor and the landscape full-screen layout go into `nemo-ui`.

Three facts about today's code shape the design:
- `LyricsDoc` and `LyricLine` have no field for a romanized or translated line, and many crates build them as struct literals. So extra per-line data travels beside the document (`LineMeta`, `Romanization`, `Translation`, all indexed by line number), never inside it.
- The TTML parser keeps singer and background-vocal facts in `TtmlDoc.meta` and skips `x-roman` and `x-translation` spans. The cache and `LyricsService` drop `meta`. This design stores it, because the Apple Music style (`now-playing-styles`) needs it and provider romanization needs the `x-roman` text.
- Own lyrics must beat the chain and never be cleared. `LyricsCache::clear` already keeps offsets, so a new table next to `lyrics_offset` follows the same rule.

Spotify sign-in uses the existing `nemo-signin` helper (a webview on Spotify's own page, which hands back the `sp_dc` cookie), with cookie paste as the fallback, the same two routes as YouTube Music. Reasons: the helper and its WebView2 and WebKitGTK setup already ship, the app never sees a password, and a second route costs one `--site` argument.

## Affected Crates

- `nemo-core`: new `lyrics_plus.rs` (own lyrics, romanizer, translation, `LineMeta`), `spotify.rs` (`SpotifyAccount`), `animated_art.rs` (owned here, used by `now-playing-styles`). `LyricsService` gains one default method, `line_meta`. No I/O, no async runtime.
- `nemo-lyrics`: `own.rs` (table `own_lyrics` in the existing lyrics database, created in `LyricsCache::init` like the other tables), `romanize/` (one file per script family), `translate.rs` (hook, table `lyrics_translation`), `ttml.rs` and `netease.rs` keep provider romanization, `service.rs` asks own lyrics first, `cache.rs` stores `LineMeta` and implements `LyricsService::keep_offline` (offline-cache design) with a `pinned` flag that skips expiry. Spotify rows are deleted on sign-out even when pinned; own lyrics are never cleared.
- `nemo-spotify` (new): session, `SpotifyLyricsProvider`, `SpotifyCanvasSource`, its own SQLite file `spotify.sqlite`.
- `nemo-signin`: `--site youtube|spotify`, one more sign-in URL and cookie filter in `cookies.rs`.
- `nemo-ytm`: the captions provider (`CaptionsLyricsProvider`, id "YouTube captions") and its host list. Designed in youtube-extras; this design only puts it last in the chain, after every lyrics provider (criterion 36).
- `nemo-ui`: lyrics editor, share card, landscape full-screen lyrics, translation and romanization rows under the lyrics, Settings > Lyrics entries.
- `nemo-app`: builds `SpotifySession`, the translation hook (given the `Translator` of ai-features, through `nemo-core` only) and the romanizer, and registers `SpotifySession` as a `SignOutWipe` step named "spotify" (it calls `wipe`).
- `nemo-audio`: not touched.

Romanization crates, one module per family behind the `Romanizer` trait:

| Language | Crate (exact pin) | Scheme | Verdict |
|---|---|---|---|
| Japanese | `sabiyomi` =0.1.0 (MIT, MSRV 1.88, 2026-09-21; embeds `lindera` 6 and IPADIC) | Hepburn, particles as pronounced | Good. About 49 MB larger program. |
| Korean | `transliteratio` =0.1.0, table `rr` | Revised Romanization | Partial. Letter-wise with jamo position rules, no sound-change rules (so 신라 is likely `sinra`, not `silla`; unverified). `korean-romanize` is GPL-3.0 and is rejected. |
| Mandarin | `pinyin` =0.11.0 (MIT, 2026-01-01) | Hanyu Pinyin with tone marks | Good. Polyphones use the most common reading. |
| Cantonese | `rust-canto` =0.3.4 (MIT, 2026-03-06, 2.7 MB data, `zstd` build needs a C compiler, already needed by bundled SQLite) | Jyutping | Good for common words. Data licence unverified. |
| Russian | `transliteratio` table `bgn` (ru) | BGN/PCGN | Good |
| Ukrainian | `transliteratio` table `uk` | National system 2010 | Good |
| Greek | `transliteratio` table `elot` | ELOT 743 | Good |
| Hindi | `transliteratio` table `hunterian` | Hunterian | Partial. Schwa deletion unverified; `lipilekhika` =1.2.0 (MIT, released 2026-11-13) is the alternative to test. |
| Arabic | `transliteratio` table `bgn` (ar) | BGN/PCGN | Partial. Lyrics have no short vowels, so output is consonants. No good offline option. |
| Persian | `transliteratio` table `bgn` (fa) | BGN/PCGN | Partial, same reason. |
| Hebrew | `transliteratio` table `heb` | Academy 2006 | Partial, same reason. |
| Thai | none. `transliteratio` has only ISO 11940 (reversible, not readable aloud); `kham-core` only segments words | RTGS needs a word list | No good offline option. Ships off, "Later" until a crate or a tested rule set exists. |

`unicode-script` =0.5.9 (MIT OR Apache-2.0, already in `Cargo.lock`) finds the script of each run. A song is Japanese when any line has kana. A Han-only song uses `RomanOptions.han` (Settings > Lyrics, default Mandarin). Cyrillic is Ukrainian when it has і, ї, є or ґ. Arabic script is Persian when it has پ, چ, ژ or گ. Mixed lines: only runs of an enabled script change (criterion 16). Partial schemes show "Romanized: BGN/PCGN (letters only)" in the footer.


## Data Flow

- Lookup stays on the app's async runtime. `LyricsHub::lookup` checks `own_lyrics` first. A hit returns at once and sends no request (criterion 8). Otherwise the chain runs as in ADR 0002. `SpotifyLyricsProvider` joins it as an opt-in entry.
- Spotify, one lookup: access token (kept in memory only, about one hour) from `sp_dc`, then the Spotify track id (cached in `spotify.sqlite`, table `track_map`), then the lyrics request. After the first song it is one request. The chain's 4 s per-provider limit (ADR 0002) and one request per host in flight still apply, except that the captions provider declares its own `LyricsProvider::timeout` of 20 s (youtube-extras), because it runs `yt-dlp`.
- Romanization is pure CPU. `nemo-ui` calls `Romanizer::romanize` on a worker thread (`spawn_blocking`) when a document arrives, and keeps the result in an in-memory map keyed by track key, provider and scheme. The UI thread only reads it. Never on the audio thread.
- Translation: UI message, then `LyricsTranslation::prepare` (no I/O), the BETA warning and the preview of ai-features when they are due, then `LyricsTranslation::translate` on the runtime, which sends all lines of the song in one `Translator::translate` call, writes the answer to `lyrics_translation`, and returns it. One request per song and language; no retry loop (criterion 40).
- Editor: the UI thread owns `EditorModel`. A tap stores `PlayerSnapshot.position` at the moment of the key event. The preview feeds the unsaved draft through the same line-finding code as the lyrics view, so the highlight matches (criterion 5). Audio buffers are not involved.
- Share image: one worker renders RGBA, then the UI saves a PNG or sets the clipboard after the click. No network.

## Interfaces

```rust
// nemo-core/src/lyrics_plus.rs
pub const OWN_LYRICS: ProviderId = ProviderId::new("Your lyrics");

/// Side data of one line, same index as `LyricsDoc.lines`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LineMeta { pub singer: Option<u8>, pub background: bool, pub roman: Option<String> }
// nemo-core/src/services.rs: LyricsService
fn line_meta(&self, track_key: &str, provider: ProviderId) -> Vec<LineMeta> { Vec::new() }

pub struct DraftLine { pub text: String, pub start_ms: Option<u64>, pub words: Vec<LyricWord> }
pub struct OwnLyricsDraft { pub lines: Vec<DraftLine> }
#[derive(Debug, thiserror::Error)]
pub enum OwnLyricsError { #[error("line {line} has no start time")] MissingStart { line: usize }, #[error("no lyrics")] Empty,
    #[error("words of line {line} are out of order")] WordOrder { line: usize }, #[error("store: {0}")] Store(String) }
pub trait OwnLyrics: Send + Sync {
    fn get(&self, track_key: &str) -> Option<LyricsDoc>;
    fn save(&self, track_key: &str, draft: &OwnLyricsDraft) -> Result<Granularity, OwnLyricsError>; // Line, or Word when every line has words
    fn save_copy(&self, track_key: &str, doc: &LyricsDoc) -> Result<(), OwnLyricsError>;            // criterion 10
    fn delete(&self, track_key: &str) -> Result<(), OwnLyricsError>;
}

#[non_exhaustive] #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RomanLang { Japanese, Korean, Mandarin, Cantonese, Russian, Ukrainian, Greek, Arabic, Hebrew, Hindi, Thai, Persian }
pub enum RomanQuality { Good, Partial }                       // Partial: vowels or readings are missing
pub struct RomanScheme { pub lang: RomanLang, pub name: &'static str, pub quality: RomanQuality }
pub struct RomanOptions { pub enabled: Vec<RomanLang>, pub han: RomanLang }  // han: Mandarin or Cantonese
pub struct Romanization { pub scheme: Option<RomanScheme>, pub lines: Vec<Option<String>>, pub from_provider: bool }
pub trait Romanizer: Send + Sync {
    fn schemes(&self) -> &'static [RomanScheme];
    fn romanize(&self, doc: &LyricsDoc, meta: &[LineMeta], opts: &RomanOptions) -> Romanization;
}

pub struct LangTag(pub String);                                // BCP 47, for example "vi"
pub enum TranslationAvailability { Ready, NoEndpoint, NotConfirmed }
pub struct Translation { pub lang: LangTag, pub lines: Vec<Option<String>>, pub endpoint_host: String }
#[derive(Debug, thiserror::Error)]
pub enum TranslationError { #[error("no AI endpoint is set")] NoEndpoint, #[error("not confirmed")] NotConfirmed,
    #[error("{0}")] Ai(AiError) }   // AiError of ai-features: Offline, Timeout, KeyRejected, BadAnswer and the rest keep their short texts
pub trait LyricsTranslation: Send + Sync {
    /// `NoEndpoint` when `Translator::available()` is false: the lyrics view hides the toggle, Settings > Lyrics shows "Set up an AI endpoint first".
    fn availability(&self) -> TranslationAvailability;
    fn cached(&self, track_key: &str, lang: &LangTag) -> Option<Translation>;
    /// No I/O. Calls `Translator::prepare`; the UI shows the preview unless the user turned it off (ai-features).
    fn prepare(&self, doc: &LyricsDoc, lang: &LangTag) -> Result<PayloadPreview, TranslationError>;
    /// One `Translator::translate(preview, doc.lines.len())` call, then the answer is stored in `lyrics_translation`.
    fn translate<'a>(&'a self, track_key: &'a str, doc: &'a LyricsDoc, lang: &'a LangTag, preview: &'a PayloadPreview) -> BoxFuture<'a, Result<Translation, TranslationError>>;
    fn forget(&self, track_key: &str);
}

// nemo-core/src/spotify.rs
pub enum SpotifyState { SignedOut, SigningIn, SignedIn, Failed(String) }
pub trait SpotifyAccount: Send + Sync {
    fn state(&self) -> SpotifyState;
    fn sign_in_window<'a>(&'a self) -> BoxFuture<'a, Result<(), SpotifyError>>;                 // runs nemo-signin --site spotify
    fn sign_in_cookie<'a>(&'a self, sp_dc: Secret<String>) -> BoxFuture<'a, Result<(), SpotifyError>>;
    fn sign_out(&self);                                                                         // deletes secret, lyrics, track map, clips
}

// nemo-core/src/animated_art.rs: the animated-art source (used by now-playing-styles)
pub struct ArtSourceId(&'static str);
pub struct ArtQuery { pub title: String, pub artists: Vec<String>, pub album: Option<String>, pub duration_ms: Option<u64> }
pub struct AnimatedArt { pub source: ArtSourceId, pub clip: PathBuf, pub width: u32, pub height: u32, pub label: &'static str }
pub enum ArtLookup { Found(AnimatedArt), NoneAvailable }
pub struct ArtSourceInfo { pub label: &'static str, pub sends: &'static str, pub risk: &'static str }
pub trait AnimatedArtSource: Send + Sync {
    fn id(&self) -> ArtSourceId;
    fn hosts(&self) -> &'static [&'static str];
    fn info(&self) -> ArtSourceInfo;
    fn find<'a>(&'a self, q: &'a ArtQuery) -> BoxFuture<'a, Result<ArtLookup, AnimatedArtError>>;
    fn clear_cache(&self);
}

// nemo-spotify/src/lib.rs
pub struct SpotifySession { /* SecretStore, reqwest::Client, signin helper path, spotify.sqlite */ }
impl SpotifySession {
    pub fn open(dir: &Path, secrets: Arc<dyn SecretStore>, helper: PathBuf) -> Result<Self, SpotifyError>;
    pub async fn access_token(&self) -> Result<Secret<String>, SpotifyError>;
    pub async fn track_id(&self, q: &LyricsQuery) -> Result<Option<String>, SpotifyError>;
    pub fn wipe(&self);
}
pub struct SpotifyLyricsProvider; // impl LyricsProvider: id "Spotify", info = ProviderInfo::opt_in(..), best_granularity Line
pub struct SpotifyCanvasSource;   // impl AnimatedArtSource; see now-playing-styles

// nemo-ui/src/share.rs
pub struct ShareCard { pub lines: Vec<String>, pub title: String, pub artist: String, pub credit: String, pub cover: Option<Vec<u8>> } // at most 4 lines
pub fn render_card(card: &ShareCard) -> Result<image::RgbaImage, ShareError>;   // 1080 x 1080, one style
pub fn save_png(img: &image::RgbaImage, path: &Path) -> Result<(), ShareError>;
pub fn copy_to_clipboard(img: &image::RgbaImage) -> Result<(), ShareError>;
```

Needed from other designs (not defined here): `Translator` (ai-features, `nemo-core/src/ai.rs`): `available() -> bool`, `prepare(&[String], target_language: &str) -> Result<PayloadPreview, AiError>`, `translate(&PayloadPreview, expected_lines) -> BoxFuture<Result<Vec<String>, AiError>>`; `AiService::provider()` gives the endpoint host shown in `Translation.endpoint_host`. `SecretStore` and `SignOutWipe` (account-actions). `OwnLyrics` needs nothing more. The `nemo-ytm` captions provider (youtube-extras) is a `LyricsProvider` with id "YouTube captions" and `ProviderInfo::opt_in`.

## Error Handling

- `OwnLyricsError::MissingStart` names the first line and blocks saving (criterion 6). Nothing is ever spread evenly.
- A Spotify failure (`SignedOut`, `TokenRefused`, `Http(status)`, `Parse`) maps to `LyricsError` and the chain goes on. A 401 or 403 drops the token once and retries once; a second one turns the provider off for the session with a line in Settings. A 429 keeps `Retry-After` like other providers.
- Translation errors keep the lyrics on screen, show one short line, and retry only on user action or the next song (criterion 40). `NotConfirmed` and `NoEndpoint` send nothing.
- Romanization cannot fail the view: a panic-free fallback returns `None` for the line.
- Share: `ShareError::{TooManyLines, Render, Clipboard, Write}`; the 5th line is blocked before rendering.
- All new error types use `thiserror`. No `unwrap`. `sp_dc` and the access token are `Secret<String>`; they are never in logs, errors or fixtures.

## Testing Strategy

- Golden tests per language: a short made-up phrase and its expected romanization (no real lyrics, they are copyrighted). A proptest checks that `romanize` never panics, never changes `lines.len()`, and leaves Latin text as written.
- `OwnLyrics`: save refuses a missing start; `clear()` keeps `own_lyrics`; the chain makes zero provider calls when own lyrics exist (counting fake provider, as in `chain.rs` tests).
- Editor model: tap, step back, re-tap and a 50 ms nudge, as pure state tests. The preview and the lyrics view give the same active line for the same position.
- Translation: with a fake `Translator`, no call happens while the switch is off, not confirmed, or no endpoint is set; one call per song and language; a failure keeps the cache unchanged.
- Spotify: hand-written fixtures for the token answer, the search answer and a `LINE_SYNCED` lyrics answer; a wipe test that finds no `sp_dc`, track map row or cached Spotify lyric afterwards; a test that `Debug` of every public type prints no secret.
- Share: contrast of every ink on the synthetic covers of `canvas.rs` tests stays at 4.5:1; a fifth line is refused; the image size is fixed.
- Snapshots of the full-screen landscape layout at 1280 x 720 and 1920 x 1080 with text scale 100% and 200%, with reduced motion on and off.
- Audio is not touched, so `cargo test -p nemo-audio --test bit_exact` only has to stay green. No real-time path is added, so no counting-allocator test.

## Risks and Alternatives

- Spotify endpoints are unofficial and change. Public write-ups put the lyrics at `spclient.wg.spotify.com/color-lyrics/v2/track/{id}` with a web-player token, and the token request has changed before (it may now need a computed code). All hosts and requests are unverified. The first task is a spike with an owner-supplied account; if the token route is closed, the provider ships disabled. Spotify may restrict the account (accepted by the owner). `sp_dc` is a full web session, so it lives only in `SecretStore`, and only `sp_dc` is kept from the cookie jar.
- Japanese romanization costs about 49 MB of program size (the README of `sabiyomi` reports a 49 MB binary, almost all IPADIC). `sabiyomi` 0.1.0 is new and has one author. Alternatives: `lindera` + `lindera-ipadic` + `wana_kana` 5.0.0 (MIT) directly, which the module can swap in behind `Romanizer`; kana-only `wana_kana` (small, useless for kanji lyrics). The IPADIC data licence is its own text, not on the `code.md` allowlist; it needs a notice and the owner's check.
- Four languages have weak offline output (Arabic, Persian, Hebrew, Hindi partial; Thai none). They are labelled, off unless picked, and Thai is Later. Online romanization is out of scope (requirements).
- Lyrics sent to the user's AI endpoint are copyrighted text; this is the owner's `coc.md` exception, opt-in, BETA, and the confirmation names the endpoint host.
- Share card drawing: `iced_test`'s renderer needs an environment variable and this workspace forbids `unsafe`, so the card is drawn with `tiny-skia` 0.11.4 and `cosmic-text` 0.15.0, both already in `Cargo.lock` through iced. Shaping Arabic and CJK needs system fallback fonts; if none exists the card shows boxes (spike).
- Share-card shaping spike (task 9, 2026-11-14), **Windows half: pass; Linux half not run** (needs the owner's Linux Mint machine). A throwaway probe outside the repo (`#![forbid(unsafe_code)]`, no environment variable, `cosmic-text` =0.15.0 and `tiny-skia` =0.11.4 at the versions in `Cargo.lock`) built `FontSystem::new()` (594 system faces; 2.7 s on the first, cold run, 0.15 s warm, debug build), shaped one line each of Japanese, Korean, Chinese, Arabic, Hebrew, Hindi, Vietnamese and Thai with `Family::SansSerif` and `Shaping::Advanced`, and drew them on a 1200 x 720 `Pixmap` through `SwashCache`. Windows 11 Pro build 26200 (the owner's PC, not a fresh install; every face used ships with Windows 11): no glyph id 0 in any line; Arabic and Hebrew came out right to left and joined; Devanagari conjuncts and Vietnamese stacked marks were correct when checked by eye. Fallback faces: Yu Gothic and Microsoft YaHei UI (Japanese), Malgun Gothic (Korean), Microsoft YaHei UI (Chinese), Segoe UI (Arabic, Hebrew, Vietnamese), Nirmala UI (Hindi), Leelawadee UI (Thai). One flaw: the kanji of the Japanese line were drawn from Microsoft YaHei UI (Chinese glyph forms) and the kana from Yu Gothic, also with the `FontSystem` locale set to `ja-JP`. So task 31 should name the face for a line it knows is Japanese (for example `Family::Name("Yu Gothic")` on Windows, from `LangTag`; not tested) and build the `FontSystem` once, off the UI thread. No bundled fallback font is needed on Windows.
- Departure from ADR 0004: `nemo-signin` gets a second site, and `tech.md` describes the helper for YouTube Music only. This is a process edge (`nemo-spotify` spawns the binary, as `nemo-ytm` does), not a crate edge. Toolchain: the brief says Rust 1.97, `rust-toolchain.toml` says 1.99.1; every pin above has MSRV 1.92 or lower.

## Steering and README changes

- Opt-in: the Spotify lyrics provider and AI translation (BETA) use the shared `OptInRow` and `OptInInfo` (ADR 0004, "Opt-in settings pattern"), replacing `ProviderInfo`'s risk note on the settings screen. Spotify sends the `sp_dc` session and the track title and artist; translation sends the lyric lines to the endpoint the user set, named by host (the confirmation of criterion 37 is that panel).
- `tech.md` rows: `sabiyomi` =0.1.0 (MIT); `pinyin` =0.11.0 (MIT); `rust-canto` =0.3.4 (MIT); `transliteratio` =0.1.0 (MIT OR Apache-2.0, 2026-10-19); `unicode-script` =0.5.9 (MIT OR Apache-2.0); `tiny-skia` =0.11.4 and `cosmic-text` =0.15.0 as direct `nemo-ui` dependencies (BSD-3-Clause and MIT OR Apache-2.0 per their repositories; already locked); `arboard` =3.7.0 (MIT OR Apache-2.0, MSRV 1.71, 2026-10-15) for the clipboard; `rfd` =0.18.0 (MIT, MSRV 1.92, 2026-11-07) for the save dialog, shared with any design that adds a file picker. Check each with `cargo deny` first; the `zstd` 0.13 of `rust-canto` duplicates a newer `zstd`.
- `structure.md`: `nemo-spotify` crate (depends on `nemo-core` only) and the edge `nemo-app -> nemo-spotify`; `nemo-signin` serves YouTube Music and Spotify; the new `nemo-core` files; `nemo-lyrics` modules `own`, `romanize`, `translate`.
- README egress list: `accounts.spotify.com`, `open.spotify.com`, `spclient.wg.spotify.com`, `api-partner.spotify.com` (unverified, to confirm in the spike), "the endpoint you set" for translation. The YouTube captions host is `www.youtube.com`, fetched by `yt-dlp` (youtube-extras design); it is listed already.
- ADR 0002: add the Spotify and YouTube captions rows (default off, risk notes) and the own-lyrics rule. ADR 0004: record `nemo-signin --site`.
- `coc.md` already has the translation exception; no edit.

<!-- Sources:
.kiro/specs/lyrics-plus/requirements.md; docs/adr/0004-feature-wave-crates.md; docs/adr/0002-lyrics-providers.md
crates/nemo-core/src/lyrics.rs, services.rs (LyricsService), secret.rs; crates/nemo-lyrics/src/cache.rs (lyrics_offset, encode_doc), chain.rs, service.rs, ttml.rs (TtmlLineMeta, x-roman skipped), netease.rs (romalrc); crates/nemo-signin/src/main.rs, cookies.rs; crates/nemo-ui/src/gui/headless.rs, canvas.rs, lyrics_view.rs
crates.io API 2026-11-14: sabiyomi, lindera, pinyin, rust-canto, transliteratio, korean-romanize (GPL-3.0), ukraine (AGPL-3.0), cranberry (GPL-3.0), kham-core, lipilekhika, unicode-script, arboard, rfd
https://docs.rs/crate/sabiyomi/0.1.0/source/README.md; https://docs.rs/crate/transliteratio/0.1.0/source/README.md
https://github.com/Paxsenix0/Spotify-Lyrics-API; https://community.spotify.com/t5/Spotify-for-Developers/Get-lyrics/td-p/5429280
-->
