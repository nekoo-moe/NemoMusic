## Tasks

Cross-spec ids written `<spec>#?` name the other spec's task by the ADR 0004 Decision 4 row it delivers; the number is filled in once that spec's `tasks.md` is merged. The design names no spike; every task here builds on existing pins.

- [x] 1. Add the `AiService` contract
  - Crate and files: `nemo-core` (`src/ai.rs` new, `src/lib.rs`); touches nemo-core/src/ai.rs.
  - Does: Adds `AiFeature`, `EndpointScope`, `AiProviderInfo`, `AiProviderConfig` (key as `Secret<String>`), `SongRef`, `TasteSource`, `AiRequest`, `PreviewMessage`, `PayloadPreview`, `SuggestedSong`, `AiAnswer`, `AiResult`, `SavedReading`, `AiError`, `AiService` and `NoAi` (`provider()` is `None`). No I/O.
  - _Requirements: none (groundwork)_
  - Depends on: none
  - Wave: 0
  - Checks: `Debug` of `AiProviderConfig` prints the key as `[redacted]`; `TasteSource::from_account` table.
  - Steering: `structure.md`, add the `ai` module to the `nemo-core` entry.

- [ ] 2. Add the `Translator` contract and the `Backend` fields
  - Crate and files: `nemo-core` (`src/ai.rs`, `src/services.rs`); touches nemo-core/src/ai.rs and nemo-core/src/services.rs.
  - Does: Adds `Translator` (`available`, `prepare`, `translate` with an expected line count), `NoTranslator`, and `Backend.ai` and `Backend.translator` defaulting to `NoAi` and `NoTranslator`. lyrics-plus calls only `Translator`.
  - _Requirements: none (groundwork)_
  - Depends on: ai-features#1, listening-stats#1 (same `services.rs`, serialize)
  - Wave: 0
  - Checks: `NoTranslator::available()` is false. lyrics-plus translation tasks depend on this task.

- [ ] 3. Create the `nemo-ai` crate skeleton
  - Crate and files: `nemo-ai` (new: `Cargo.toml`, `src/lib.rs`, `src/service.rs`); workspace `Cargo.toml` members.
  - Does: Adds the crate with an `AiClient` stub that implements `AiService` and `Translator` as "no provider", and its `OptInInfo` with hosts "the endpoint you set".
  - _Requirements: 9_
  - Depends on: ai-features#2, integrations#1
  - Wave: 0
  - Checks: a fresh `AiClient` has no provider, no preset endpoint and no key.
  - Steering: `structure.md`, new crate `nemo-ai` (depends on `nemo-core` only) and edge `nemo-app -> nemo-ai`.

- [ ] 4. Validate the endpoint URL
  - Crate and files: `nemo-ai` (`src/endpoint.rs` new).
  - Does: `validate_endpoint` allows `https` always; `http` only for `localhost`, a loopback IP or a private IP written as a number (IPv4-mapped IPv6 judged by its IPv4 part), marked as needing the LAN warning; rejects other names, userinfo, query and fragment. Pure, via `url`.
  - _Requirements: 5_
  - Depends on: ai-features#3
  - Wave: 1
  - Checks: the design's table (`http://localhost:11434`, `http://[::1]`, `http://192.168.1.5`, `http://[::ffff:10.0.0.1]`, `http://2130706433`; rejected `http://example.com`, `http://localhost.example.com`, `http://nas.local`, `http://user:pw@127.0.0.1`, `ftp://`, a query string).

- [ ] 5. Add the AI settings and readings database
  - Crate and files: `nemo-ai` (`src/store.rs` new); `nemo-app` (`src/paths.rs`, `paths.ai`).
  - Does: Opens `ai.sqlite3` (WAL, `user_version` migrations, parameterized) with `provider` (endpoint, model; never the key), `features(feature, enabled, skip_preview)` and `readings` (time, source, `from_account`, model, host, text). Every feature starts off.
  - _Requirements: 8, 11, 22_
  - Depends on: ai-features#3
  - Wave: 1
  - Checks: migrations from empty and twice; readings round trip; a fresh file has every feature off; the key column does not exist.

- [ ] 6. Build the exact payload in `prepare`
  - Crate and files: `nemo-ai` (`src/prompts.rs` new, `src/service.rs`).
  - Does: `prepare` checks the feature switch, caps (50 songs, 200 lines, 64 KiB), strips control characters, and puts titles and lines in a JSON `data` block escaped by `serde_json`, with a system message that says the block is data. Sends only titles, artists, album, play count; no cookies, tokens, ids, paths, links or device ids. No I/O.
  - _Requirements: 12, 13, 14, 16, 20_
  - Depends on: ai-features#4, ai-features#5
  - Wave: 2
  - Checks: 51 songs, 201 lines and a 65 KiB payload give `TooLarge`; feature off gives `FeatureOff`; a seed with a file path or account id puts neither in the messages; snapshot of each prompt.

- [ ] 7. Add the HTTP client for the chat endpoint
  - Crate and files: `nemo-ai` (`src/client.rs` new).
  - Does: Posts `{model, messages, stream: false, max_tokens}` to `<base>/v1/chat/completions` (or `/chat/completions` after `/v1`), built from `preview.messages` only. native-tls, redirects off, connect 10 s, total 120 s, one attempt, answer cut at 256 KiB; `localhost` pinned to loopback with no proxy; the key only in a sensitive `Authorization: Bearer` header. Errors map by status and kind and hold no URL, key or body; nothing logs prompts, answers or keys.
  - _Requirements: 3, 4, 23, 26, 28_
  - Depends on: ai-features#4
  - Wave: 2
  - Checks: local test server on `tokio::net::TcpListener`: posted bytes equal `preview.messages`; key only in the header and in no `Debug`, `Display` or `tracing` capture; 302 not followed; over 256 KiB fails; slow reply times out; 401, 403, 404, 429, 500 map to the right errors.
  - Steering: `tech.md` row "AI client" naming `reqwest`, `url`, `rusqlite`, `serde_json`, `tokio` (existing pins) and the endpoint rules; README egress line "the endpoint you set" (only when an AI provider is saved).

- [ ] 8. Parse answers strictly
  - Crate and files: `nemo-ai` (`src/parse.rs` new).
  - Does: Reads `choices[0].message.content` as a string and strips one code fence. `parse_songs` takes at most 30 `{title, artist}` of 1 to 200 characters; `parse_reading` takes plain text up to 3,000 characters; `parse_lines` needs exactly the expected count. Anything else is `BadAnswer`; an empty content is `Empty`.
  - _Requirements: 23, 25_
  - Depends on: ai-features#3
  - Wave: 2
  - Checks: valid and fenced JSON, extra fields ignored, 31 songs, an overlong title, wrong line count, control characters, a title saying "ignore previous instructions" stays plain data.

- [ ] 9. Save, test and remove the provider
  - Crate and files: `nemo-ai` (`src/service.rs`).
  - Does: `save_provider` validates the endpoint, refuses plain-HTTP LAN unless `confirm_plain_http`, stores the key under `ai.provider.key` in `SecretStore` and returns host and scope. `test_connection` sends a fixed text with no user data; `list_models` reads `/v1/models` and is optional. `remove_provider` deletes the key first, then endpoint and model, and reports a key-delete failure.
  - _Requirements: 2, 4, 5, 6, 7, 8, 9_
  - Depends on: ai-features#5, ai-features#7, account-actions#? (row 24, `SecretStore`)
  - Wave: 2
  - Checks: fake `SecretStore`: key never in `ai.sqlite3`; LAN plain HTTP without confirm is refused; test message has no user data; after remove `provider()` is `None`.

- [ ] 10. Send confirmed requests and implement `Translator`
  - Crate and files: `nemo-ai` (`src/service.rs`).
  - Does: `send` runs on the tokio runtime and returns `AiResult` with model and host. Feature switches and `skip_preview` (always false for the taste reading) are stored per feature. A network failure to a `Remote` endpoint sets `offline_hint` for 60 s. `Translator` is `available()` only with a provider and translation on, and `translate` checks the line count. No call happens without a user action.
  - _Requirements: 11, 12, 15, 23, 24_
  - Depends on: ai-features#6, ai-features#8, ai-features#9
  - Wave: 2
  - Checks: a counting test server sees zero calls with no provider, a feature off, or at start-up; `offline_hint` set only for `Remote`; `skip_preview(TasteReading)` stays false after setting it.

- [ ] 11. Wire `nemo-ai` and its sign-out step
  - Crate and files: `nemo-ai` (`src/service.rs`, `SignOutWipe` impl named "ai"); `nemo-app` (`src/backend.rs`: one `AiClient` as `Backend.ai` and `Backend.translator`, register the wipe step with `GoogleAccount`).
  - Does: Sign-out deletes readings with `from_account = 1` and keeps the provider and readings from stats or local playlists. A failed open wires `NoAi` and `NoTranslator`.
  - _Requirements: 10_
  - Depends on: ai-features#10, account-actions#? (row 26, `SignOutWipe` and its registration)
  - Wave: 2
  - Checks: wipe deletes only `from_account` rows; provider survives sign-out; open failure gives `NoAi`.

- [ ] 12. Add the "AI provider" settings entry
  - Crate and files: `nemo-ui` (Settings "AI provider", `src/copy.rs`); `nemo-app` (wiring).
  - Does: An `OptInRow` with the typed host as "To" and the risk text (what you send leaves this computer for that host, its owner's terms apply); fields for endpoint, model and an optional key; a LAN plain-HTTP warning with confirm; "Test connection", "List models", "Remove provider". A different host asks again. With no provider, no other AI control exists anywhere.
  - _Requirements: 1, 2, 5, 6, 7, 8, 27_
  - Depends on: ai-features#9, ai-features#11, integrations#2, integrations#3
  - Wave: 3
  - Checks: `iced_test`: with `NoAi` no AI control is in the widget tree; key field never echoes the key or copies it; keyboard order and focus; `design.md` checklist with a screenshot.

- [ ] 13. Add the feature switches and the payload preview dialog
  - Crate and files: `nemo-ui` (`src/gui/views/ai_preview.rs` new, `src/copy.rs`).
  - Does: Each feature turns on in a dialog that lists what it sends. Before each action the preview shows the exact messages, host, model, scope and the LAN warning, and sends only on confirm. "Do not ask again for this feature" is off by default and not offered for the taste reading.
  - _Requirements: 11, 12, 27_
  - Depends on: ai-features#12
  - Wave: 3
  - Checks: `iced_test` of the dialog; cancel sends nothing (counting fake); keyboard order; no pre-selected button.

- [ ] 14. Add "Suggest songs"
  - Crate and files: `nemo-ui` (actions on song, album, playlist and queue; `src/gui/views/ai_suggest.rs` new; `suggest_match`).
  - Does: Sends up to 50 seed titles and artists through the preview. Shows the answer as "AI suggestions, may be wrong", looks each up with `Catalog::search` (one at a time, 250 ms apart, at most 30), shows "not found" when no result matches both normalized fields, and adds to the queue or a playlist only on the user's action.
  - _Requirements: 16, 17, 18, 19, 27_
  - Depends on: ai-features#13
  - Wave: 3
  - Checks: `suggest_match` tests (not found gives `None`, a near-miss title does not match); nothing reaches the queue without a click; keyboard reach.

- [ ] 15. Add "Read my taste"
  - Crate and files: `nemo-ui` (`src/gui/views/ai_taste.rs` new); `nemo-app` (taste sources as data).
  - Does: Offers the sources: top artists and counts from `StatsService::top_artists` only while recording is on and the history has plays, liked songs, or a chosen playlist. The preview always shows the exact list. The reading shows as plain text labelled with model and host, is saved on this computer, and can be deleted.
  - _Requirements: 20, 21, 22, 27_
  - Depends on: ai-features#13, listening-stats#8
  - Wave: 3
  - Checks: with `NoStats` or recording off the stats source is absent and no stats query runs; saved readings list and delete; reading drawn without links or markdown.

- [ ] 16. Handle offline and errors in the UI, document, and run manual checks
  - Crate and files: `nemo-ui` (AI actions, loading indicator); `README.md`.
  - Does: Disables AI actions with the reason while `offline_hint` is set; shows one short message per `AiError` and keeps playing; the loading animation stops under reduced motion. README risk text: the endpoint you set, its owner's terms apply, plain HTTP on a LAN is unencrypted. Manual: Ollama on loopback, a LAN server over HTTP with the warning, and a remote HTTPS endpoint.
  - _Requirements: 23, 24, 27, 28_
  - Depends on: ai-features#14, ai-features#15
  - Wave: 3
  - Checks: `iced_test` with reduced motion; egress-list test from integrations#3 passes. A remote HTTPS check needs a key the owner provides.
  - Steering: README AI risk text.

## Definition of Done

See [workflow.md section 4](../../steering/workflow.md#4-definition-of-done). Also for this spec:
- `nemo-audio` is not changed; `cargo test -p nemo-audio --test bit_exact` keeps passing. No real-time path is added, so no counting-allocator test.
- A counting test server sees zero requests with no provider, a feature off, or no confirm, and no test output holds a prompt, answer or key.
- No default endpoint, preset provider or key exists in the code or settings files.
