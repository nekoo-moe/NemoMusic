## Overview

A new crate, `nemo-ai`, is a small client for one OpenAI-style chat endpoint that the user sets. It does three jobs: song suggestions, a taste reading, and lyrics translation for the lyrics-plus spec. The crate has no preset provider, no default key and no background call. Every request goes through two steps that the UI cannot skip: `prepare` builds the exact messages and shows them with the host, and only after the user confirms does `send` transmit those same messages.

`nemo-core` gets the contracts (`ai.rs`): `AiService` for the UI and a narrow `Translator` trait for `nemo-lyrics` and lyrics-plus, so no other crate depends on `nemo-ai` (ADR 0004). `nemo-ai` needs no new third-party crate. The user's API key lives only in the OS credential store.

## Affected Crates

- `nemo-core` (new `ai.rs`): `AiFeature`, `AiRequest`, `PayloadPreview`, `AiAnswer`, `AiError`, `AiService`, `Translator`, `NoAi` and `NoTranslator` defaults; `Backend.ai` and `Backend.translator`.
- `nemo-ai` (new): `endpoint.rs` (URL rules), `client.rs` (HTTP), `prompts.rs`, `parse.rs` (strict answer parsing), `store.rs` (SQLite: provider, switches, saved readings), `service.rs`.
- `nemo-ui`: Settings entry "AI provider" in the shared opt-in pattern (endpoint, model, key, test, list models, remove); the "Suggest songs" and "Read my taste" actions; the preview dialog (messages, host, model, scope, a LAN warning); the result lists. Every AI control is built only when `AiService::provider()` is `Some` (criterion 1). Suggestion lookup uses `Catalog::search` and a pure matcher in `nemo-ui` (`suggest_match`).
- `nemo-app`: builds `nemo-ai` with the `SecretStore`, the tokio handle and `paths.ai`; passes the same object as `Backend.ai` and `Backend.translator`; wires the taste sources (stats, likes, playlist) as data.
- `nemo-lyrics` and the lyrics-plus UI: call `Translator` only. They need from this design `available()`, `prepare` and `translate`; the lyrics-plus spec owns the warning text and where translations are stored.
- Needs from other designs: `SecretStore` (nemo-secrets, ADR 0004); `StatsService::top_artists` (listening-stats design, same author); a sign-out wipe step (the ADR does not name the trait yet).
- `nemo-audio`, `nemo-library`, `nemo-ytm`: no change.

## Data Flow

Everything is user-triggered. There is no timer, no prefetch and no call at start-up (criterion 15).

1. The UI collects data (seed songs, taste input, lyric lines of the song on screen) and calls `prepare(&AiRequest)` on the UI thread. `prepare` does no I/O. It checks the feature switch, caps (50 songs, 200 lyric lines, 64 KiB), strips control characters, and returns `PayloadPreview { messages, host, model, scope, plain_http, song_count }`.
2. The UI shows the exact messages and host. The user confirms. The "do not ask again" switch hides this step for Suggestions and Lyrics translation only; the taste reading always shows it (the owner wants the stats data visible).
3. `send(&preview)` runs on the app's tokio runtime. The HTTP client posts `{model, messages, stream: false, max_tokens}` to `<base>/v1/chat/completions` (a base that already ends in `/v1` gets `/chat/completions`). It builds the body from `preview.messages` and nothing else.
4. The answer is read through a counter that stops at 256 KiB, then parsed strictly (see Interfaces). Parsed values return as owned data to the UI. Nothing is executed, fetched or queued from an answer.
5. Suggestions: the UI searches each suggested "artist title" through `Catalog::search`, one at a time with a 250 ms gap, at most 30, and shows "not found" when no result matches both normalized fields. Songs reach the queue only on the user's action (criterion 19).
6. A taste reading is saved in `ai.sqlite3` (`readings`: time, source, `from_account`, model, host, text) until the user deletes it. Sign-out deletes rows with `from_account = 1` (liked songs, account playlists); readings from stats or local playlists stay (criterion 10).

Threads and buffers: no audio buffer is touched. HTTP runs on the existing `nemo-io` runtime. Queries on the SQLite file use the `nemo-lyrics` pattern (`Mutex<Connection>`, WAL, parameterized).

Prompt injection: titles and lyric lines go into the user message inside a JSON `data` block, escaped by `serde_json`, and the system message says the block is data. The model may still follow it, so the safety does not rely on the model: the answer type is fixed (a list of `{title, artist}`, a list of strings, or plain text), unknown fields are ignored, and no answer can trigger an action.

Endpoint rules (`endpoint.rs`, pure, via the `url` crate): `https` is always allowed. `http` is allowed only for host `localhost`, a loopback IP, or a private IP written as a number (10/8, 172.16/12, 192.168/16, 169.254/16, `fc00::/7`, `fe80::/10`; an IPv4-mapped IPv6 address is judged by its IPv4 part). `url` already turns forms like `http://2130706433` into dotted IPs. A name such as `nas.local` needs `https`. Userinfo, query and fragment are rejected. For `localhost` the client pins the address to `127.0.0.1` and `::1` with `resolve_to_addrs`, so a hosts-file trick cannot send it elsewhere, and loopback skips the system proxy.

## Interfaces

```rust
// nemo-core/src/ai.rs  (no I/O)
pub enum AiFeature { Suggestions, TasteReading, LyricsTranslation }
pub enum EndpointScope { Loopback, PrivateLan, Remote }
pub struct AiProviderInfo { pub host: String, pub model: String, pub scope: EndpointScope, pub plain_http: bool }
pub struct AiProviderConfig { pub endpoint: String, pub model: String, pub api_key: Option<Secret<String>>,
    pub confirm_plain_http: bool }   // the user accepted the LAN warning
pub struct SongRef { pub title: String, pub artist: String, pub album: Option<String>, pub plays: Option<u32> }
pub enum TasteSource { Stats, LikedSongs, Playlist { account: bool } }
impl TasteSource { pub fn from_account(&self) -> bool; }
pub enum AiRequest {
    Suggest { seeds: Vec<SongRef> },                                           // at most 50
    Taste { source: TasteSource, artists: Vec<(String, u32)>, songs: Vec<SongRef>, reply_language: String },
    Translate { lines: Vec<String>, target_language: String },                 // at most 200 lines
}
pub struct PreviewMessage { pub role: &'static str, pub content: String }      // "system" or "user"
pub struct PayloadPreview { pub feature: AiFeature, pub messages: Vec<PreviewMessage>, pub host: String,
    pub model: String, pub scope: EndpointScope, pub plain_http: bool, pub song_count: usize }
pub struct SuggestedSong { pub title: String, pub artist: String }
pub enum AiAnswer { Songs(Vec<SuggestedSong>), Reading(String), Translated(Vec<String>) }
pub struct AiResult { pub answer: AiAnswer, pub model: String, pub host: String }
pub struct SavedReading { pub id: i64, pub created_at: i64, pub source: TasteSource, pub model: String, pub host: String, pub text: String }
pub enum AiError { NoProvider, FeatureOff, InvalidEndpoint(&'static str), TooLarge, Offline, Timeout, Unreachable,
    KeyRejected, ModelMissing, RateLimited, ServerError, BadAnswer, Empty, Storage, SecretStore }  // thiserror, short texts

pub trait AiService: Send + Sync {
    fn provider(&self) -> Option<AiProviderInfo>;                  // None: hide every AI control
    fn save_provider(&self, cfg: AiProviderConfig) -> Result<AiProviderInfo, AiError>;
    fn remove_provider(&self, delete_readings: bool) -> Result<(), AiError>;   // endpoint, model, key
    fn test_connection(&self) -> BoxFuture<'_, Result<(), AiError>>;           // fixed text, no user data
    fn list_models(&self) -> BoxFuture<'_, Result<Vec<String>, AiError>>;      // GET /v1/models, optional
    fn feature_enabled(&self, f: AiFeature) -> bool;
    fn set_feature_enabled(&self, f: AiFeature, on: bool);
    fn skip_preview(&self, f: AiFeature) -> bool;                  // always false for TasteReading
    fn set_skip_preview(&self, f: AiFeature, on: bool);
    fn offline_hint(&self) -> bool;                                // a Remote request failed on the network in the last 60 s
    fn prepare(&self, req: &AiRequest) -> Result<PayloadPreview, AiError>;
    fn send<'a>(&'a self, preview: &'a PayloadPreview) -> BoxFuture<'a, Result<AiResult, AiError>>;
    fn readings(&self) -> Vec<SavedReading>;
    fn save_reading(&self, source: TasteSource, result: &AiResult) -> Result<i64, AiError>;
    fn delete_reading(&self, id: i64);
    fn wipe_account_derived(&self) -> usize;                       // sign-out step
}

/// What lyrics-plus sees. `available()` is false until a provider is saved and translation is switched on.
pub trait Translator: Send + Sync {
    fn available(&self) -> bool;
    fn prepare(&self, lines: &[String], target_language: &str) -> Result<PayloadPreview, AiError>;
    fn translate<'a>(&'a self, preview: &'a PayloadPreview, expected_lines: usize)
        -> BoxFuture<'a, Result<Vec<String>, AiError>>;             // BadAnswer unless exactly expected_lines strings
}
pub struct NoAi;  pub struct NoTranslator;                          // available() false, provider() None

// nemo-ai
pub struct AiClient { /* reqwest::Client, Mutex<Connection>, Arc<dyn SecretStore>, tokio Handle */ }
impl AiClient { pub fn open(dir: &Path, secrets: Arc<dyn SecretStore>, handle: Handle) -> Result<Self, AiError>; }
impl AiService for AiClient {}  impl Translator for AiClient {}
pub fn validate_endpoint(text: &str) -> Result<Endpoint, AiError>;   // pure
pub fn parse_songs(content: &str) -> Result<Vec<SuggestedSong>, AiError>;
pub fn parse_lines(content: &str, expected: usize) -> Result<Vec<String>, AiError>;
pub fn parse_reading(content: &str) -> Result<String, AiError>;
```

The key sits under the `SecretStore` name `ai.provider.key` (one provider, so no lookup table; criterion 4). The endpoint and model are not secret and live in `ai.sqlite3` (`provider`, `features(feature, enabled, skip_preview)`, `readings`). The HTTP client: native-tls, redirects off, connect timeout 10 s, total 120 s (local models are slow), `max_tokens` 800 for suggestions, 600 for a reading, 4096 for translation, one attempt, no retry. The key goes in a sensitive `Authorization: Bearer` header only when set.

Parsing is strict: `choices[0].message.content` must be a string. A surrounding code fence is stripped. Suggestions are a JSON array of at most 30 objects with `title` and `artist`, each 1 to 200 characters after control characters are removed. A reading is plain text up to 3,000 characters, drawn as plain text (no markdown, no links). Translation is a JSON array of strings with exactly the expected length. Anything else is `BadAnswer`, and nothing from it is shown (criterion 25).

## Error Handling

- Each `AiError` has one short text. HTTP 401 or 403 is `KeyRejected`, 404 is `ModelMissing` (Ollama and OpenAI both answer 404 for an unknown model), 429 `RateLimited`, 5xx `ServerError`, a closed connection `Unreachable`, an empty `content` `Empty` (criterion 23). Playback never depends on any of them.
- Error values hold no URL, no key and no body text. `reqwest` errors are mapped by kind and `without_url` is used before any logging.
- Nothing logs prompts, answers or keys (criterion 26); `tracing` lines carry only the error kind and the host class (`loopback`, `lan`, `remote`).
- Offline: a network failure to a `Remote` endpoint sets `offline_hint` for 60 s and the UI disables AI actions with the reason (criterion 24). Loopback and LAN endpoints never set it. There is no connectivity probe in the app today, so actions are enabled until the first failure.
- `save_provider` refuses a plain-HTTP LAN endpoint unless `confirm_plain_http` is true. The UI sets it only after it shows the warning and the user agrees.
- `remove_provider` deletes the key first, then the rows, and returns `SecretStore` if the key delete failed so the UI does not claim success.

## Testing Strategy

- `validate_endpoint` table: https anywhere; `http://localhost:11434`; `http://127.0.0.1`; `http://[::1]`; `http://192.168.1.5`; `http://[::ffff:10.0.0.1]`; `http://2130706433` (loopback); rejected: `http://example.com`, `http://localhost.example.com`, `http://nas.local`, `http://user:pw@127.0.0.1`, `ftp://`, a query string.
- Local test server (hand-rolled on `tokio::net::TcpListener`, like `nemo-ytm/src/test_server.rs`): the bytes posted match `preview.messages` exactly; the key appears only in the `Authorization` header and in no `Debug`, `Display` or captured `tracing` output; a 302 is not followed; a response over 256 KiB is cut and fails; a slow reply times out; 401, 404, 429, 500 map to the right errors; `localhost` pins to loopback.
- Parsing: valid and fenced JSON, extra fields, 31 songs, an overlong title, wrong line count for translation, text with control characters, a song title that contains "ignore previous instructions" (it appears in the answer as plain data and triggers nothing).
- Limits: `prepare` with 51 songs, 201 lines and a 65 KiB payload each fails with `TooLarge` and sends nothing; with no provider or the feature off no request is made (a server that counts calls sees zero).
- Store: readings round trip; `wipe_account_derived` deletes only `from_account` rows; `remove_provider` removes endpoint, model and key (fake `SecretStore`).
- `suggest_match` unit tests in `nemo-ui`: not found gives `None`; a near-miss title does not match.
- UI `iced_test`: with `NoAi` no AI control exists in the widget tree; keyboard order and focus; reduced motion stops the loading animation.
- Audio is untouched; `cargo test -p nemo-audio --test bit_exact` must keep passing. No real-time path is added, so no counting-allocator test.

## Risks and Alternatives

- Lyrics leave the machine for translation by the owner's decision (`coc.md`, 2026-11-14). This design only makes `prepare` mark the payload as `LyricsTranslation`; the lyrics-plus spec owns the warning and the BETA label.
- The lyrics-plus requirements (criterion 38) show the translation toggle as unavailable when no endpoint is set, while ai-features criterion 1 hides it. `Translator::available()` serves both; the two specs need one wording from the owner.
- Plain HTTP on a LAN address sends text and the key unencrypted to any device that answers on that address. The warning and confirmation are the only guard. Names like `nas.local` are refused on purpose, because mDNS names can resolve to anything.
- A remote provider receives listening habits (top artists and counts). The preview and the switch are the control; the owner accepted this.
- Models can invent songs or return broken JSON. The "may be wrong" label, the search check and strict parsing contain this; no answer is acted on.
- Request and answer size limits (64 KiB, 256 KiB) and the 120 s timeout are choices, not measured; local models on slow hardware may need more time. Unverified: `/v1/models` support on every server (the list is optional).
- Alternative: preset providers; rejected by the owner. Alternative: the native Claude or Gemini APIs; out of scope. Alternative: an LLM crate such as `async-openai`; rejected, it adds a large dependency for one POST and lacks the endpoint rules above.
- An extra ADR 0004 point: `nemo-ai` needs `SecretStore` and the wipe step from other designs; until they land, the key store and wipe are traits with fakes.

## Steering and README changes

- `tech.md`: no new crate. `nemo-ai` uses existing pins `reqwest` =0.13.5 (`native-tls`, no default features), `url` =2.5.8, `rusqlite` =0.41.1, `serde_json`, `tokio` =1.54.1 (`rt`, `time`, `sync`), `thiserror`, `tracing`. Add one row "AI client" that names these and the endpoint rules.
- `structure.md`: new crate `nemo-ai`, edge `nemo-app -> nemo-ai`, depends on `nemo-core` only; new `nemo-core` module `ai`.
- README egress list: "the endpoint you set" for AI (no fixed host), with the risk text: what you send leaves this computer for that host, and its owner's terms apply; plain HTTP on a LAN is unencrypted.
- ADR 0004: add `ai.rs` and `Translator` to the list of shared traits, and note that `nemo-lyrics` calls only `Translator`.
- `coc.md` already holds the lyrics exception; no change.

<!-- Sources:
- docs/adr/0004-feature-wave-crates.md (nemo-ai, translator in nemo-core, SecretStore, sign-out wipe)
- .kiro/steering/coc.md (Owner Decisions 2026-11-14: lyrics translation BETA; opt-in rules)
- .kiro/specs/lyrics-plus/requirements.md (criteria 37 to 39: translation and AI endpoint)
- crates/nemo-core/src/services.rs (Backend), catalog.rs (Catalog::search, SearchResults.songs), secret.rs
- crates/nemo-lyrics/Cargo.toml (reqwest native-tls pin), src/cache.rs (SQLite pattern); crates/nemo-ytm/src/test_server.rs
- https://docs.ollama.com/api/openai-compatibility (/v1/chat/completions on a local server)
- https://crates.io/api/v1/crates/{reqwest,url} (versions and licences, queried 2026-11-14; both already in Cargo.lock)
-->
