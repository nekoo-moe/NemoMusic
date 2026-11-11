---
inclusion: always
---

# Workflow

How work flows in NemoMusic. Rules about product, code style, tech, layout and design live in sibling files; this file only covers process.

## 1. Spec-driven loop

Every feature goes through `.kiro/specs/<feature>/` (kebab-case folder), in this order. Do not start the next file until the previous one is approved by the owner.

`requirements.md` must contain these headings, in order:
- `## Goal` (one paragraph, user-visible outcome)
- `## User Stories` (As a / I want / so that)
- `## Acceptance Criteria` (numbered, each testable, EARS style: WHEN ... THE SYSTEM SHALL ...)
- `## Out of Scope`
- `## Open Questions`

`design.md` must contain:
- `## Overview`
- `## Affected Crates` (list of `nemo-*` crates)
- `## Data Flow` (threads, channels, who owns audio buffers)
- `## Interfaces` (public types and traits, signatures only)
- `## Error Handling`
- `## Testing Strategy` (name the bit-exact test if audio is touched)
- `## Risks and Alternatives`

`tasks.md` must contain:
- `## Tasks` as a checkbox list `- [ ] 1. ...`, each task small enough for one PR, each citing the acceptance criteria it satisfies (`_Requirements: 2, 3_`)
- `## Definition of Done` link back to section 4 of this file

Skip the spec only when ALL hold: change touches one crate, under about 50 lines, no public API change, no audio path change, no UI change. Examples: typo, dependency patch bump, clippy fix, log wording. State "spec skipped: tiny fix" in the PR description.

## 2. Steering files and precedence

- `product.md`: what NemoMusic is and its priorities (sound quality, beauty, calm use).
- `tech.md`: stack, dependencies, platform targets, build tooling.
- `structure.md`: workspace layout, crate responsibilities, dependency direction.
- `code.md`: Rust style, error handling, testing, real-time audio rules.
- `design.md`: visual and interaction design, plus the design checklist.
- `coc.md`: code of conduct and hard conduct limits.
- `workflow.md`: this file, process only.
On conflict, the higher one wins: 1) `coc.md`, 2) `product.md`, 3) `code.md`, 4) `tech.md` and `structure.md` (equal; raise to the owner if they disagree), 5) `design.md`. This file never overrides any of them. If a conflict is found, say which file won and why in the PR description, and propose a fix to the losing file.

## 3. Git

- Never push to `main` directly. Every change goes through a PR from a branch.
- Branch names: `<type>/<crate-or-area>-<short-slug>`, lowercase, e.g. `feat/nemo-audio-gapless`, `fix/nemo-ui-seek-jitter`, `docs/workflow-hooks`. Types: `feat`, `fix`, `perf`, `refactor`, `test`, `docs`, `build`, `ci`, `chore`.
- Commits follow Conventional Commits: `type(scope): imperative summary` (max 72 chars). Scope is the full crate name including the `nemo-` prefix, e.g. `feat(nemo-audio): add gapless transition`. Cross-crate changes use the main crate as scope. Non-crate changes use `repo`, `docs` or `ci`.
- Breaking change: add `!` after scope and a `BREAKING CHANGE:` footer.
- Small PRs: one task from `tasks.md` per PR, ideally under 400 changed lines. Split otherwise.
- PR description: link to the spec, list the checked Definition of Done items, attach a screenshot for UI changes.
- Stage files by name, never `git add .`. Never commit secrets, cookies or YouTube Music auth tokens.

## 4. Definition of Done

A task is done only when every applicable box is true. Run from the repo root in PowerShell:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
# Only when nemo-audio or any code in the audio path changed:
cargo test -p nemo-audio --test bit_exact
```

- [ ] fmt, clippy (`-D warnings`) and `cargo test --workspace` all clean
- [ ] Bit-exact audio test passes when audio code changed (output samples equal decoded source unless the spec says otherwise)
- [ ] Design checklist in `design.md` completed when UI changed, with a screenshot
- [ ] Docs updated: `README.md`, rustdoc on public items, and the spec files if behavior differs from them
- [ ] Task ticked in `tasks.md`
- [ ] No new `unwrap()` or `expect()` outside tests, no allocation or locking in the audio callback

## 5. Subagents

Use subagents only for independent tasks that can run in parallel. Do the work yourself if it is sequential or small.
- One owner per file. Two agents never edit the same file; serialize if they would.
- Brief each agent with: goal, exact file paths it owns, files it must not touch, inputs already decided, the steering files to read, the output format, and a stop condition.
- Agents must not create other agents or edit workspace `Cargo.toml` unless it is their assigned file.
- The parent reads the actual result (diff, test output) before calling the work done. A success claim is not evidence.

## 6. Proposed hooks (not created; owner decides)

Proposals only; adjust field names to the installed Kiro version.

```json
{ "name": "clippy-on-save", "when": { "type": "fileEdited", "patterns": ["crates/**/*.rs"] }, "then": { "type": "runCommand", "command": "cargo clippy --workspace --all-targets -- -D warnings" } }
{ "name": "bit-exact-reminder", "when": { "type": "fileEdited", "patterns": ["crates/nemo-audio/**/*.rs"] }, "then": { "type": "askAgent", "prompt": "nemo-audio changed. Run cargo test -p nemo-audio --test bit_exact and report before finishing." } }
{ "name": "doc-sync-reminder", "when": { "type": "fileEdited", "patterns": ["crates/*/src/lib.rs", ".kiro/specs/**/design.md"] }, "then": { "type": "askAgent", "prompt": "Public API or design changed. Check README.md, rustdoc and spec files still agree." } }
```

## 7. Release basics (Windows installer)

- Version in the workspace `Cargo.toml` follows SemVer; tag `vX.Y.Z` on `main` after the release PR merges.
- Build with `cargo build --release --workspace` using the pinned toolchain; run the full Definition of Done first.
- Package an installer (MSI via WiX or an NSIS setup) that installs per-user, with uninstall and no admin prompt.
- Sign the binary and installer; verify with `signtool verify /pa`. Never store the certificate in the repo.
- Smoke test on a clean Windows VM: install, play a lossless file, upgrade, uninstall. Notes go in `CHANGELOG.md`.

## 8. Stop and ask the owner

Stop and ask before doing any of these:
- Anything that could alter audio output (resampling, dithering, normalization, EQ default, volume path, output API).
- Adding a dependency not in `tech.md`, or one with a non-permissive license.
- YouTube Music auth, scraping, rate limits, terms of service, or user credentials.
- Public API breaks, crate boundary changes, or edits to another steering file.
- Deleting files, rewriting history, force-pushing, or touching `LICENSE` or `CODE_OF_CONDUCT.md`.
- Ambiguous requirements, same-level steering conflicts, or an unmeetable acceptance criterion.
- Visual changes that depart from `design.md`, or any telemetry or data collection.
- A test or clippy failure you cannot fix within the task's scope.
