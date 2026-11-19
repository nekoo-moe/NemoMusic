# SoundCloud fixtures

Recorded from the web API v2 on 2026-11-19 (anonymous, a few requests), then rewritten: every id, title,
name, address and signature is invented. The field names and shapes are the real ones. They hold no
`client_id`, no token, no cookie and no real account or track. Do not add real values; a test in
`src/fixtures.rs` fails on a `client_id`, `oauth`, a signature or a host that is not in `HOSTS`.
