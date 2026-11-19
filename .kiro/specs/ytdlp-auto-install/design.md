## Overview

`nemo-ytm` gains `ytdlp::managed`, the policy for the app-managed copy in `<data>/NemoMusic/bin/yt-dlp` (the first place `locate_yt_dlp` looks). `ytdlp_install.rs` keeps the download and checksum code and gains the Linux one-file install. `nemo-app` calls `managed::start_in_background` once at startup with the setting, whether the user set a path, and the program the lookup found. The resolver gets a `ProgramSlot` (`Arc<OnceLock<PathBuf>>`) that a finished install fills, so every resolver clone switches to the new program at once with lock-free reads.

## Affected Crates

- `nemo-ytm`: `ytdlp/managed.rs` (new, with `managed/tests.rs`), `ytdlp_install.rs` (split into client, checksum list, verified fetch, one-file placement and onedir unpack; `install_onefile`), `ytdlp.rs` (`ProgramSlot`, `YtDlpResolver::program_slot`), `ytdlp/locate.rs` (`managed_dir`), `examples/install_yt_dlp.rs` (Linux too).
- `nemo-app`: `config.rs` (`yt_dlp_auto_install`, default true), `backend.rs` (start the worker).
- Docs: `README.md` (egress rows, build text), `tech.md` (run-time download note), release notes text in `.github/workflows/release.yml`.

## Data Flow

Startup thread -> `start_in_background` spawns the `yt-dlp-install` std thread and returns. The worker: platform build -> `plan` (reads the check-time file) -> blocking `reqwest` client -> `SHA2-256SUMS` (max 64 KiB) -> compare with the recorded hash -> 1 s pause -> the asset (max 200 MB, held in memory, hashed) -> place it (Linux: `yt-dlp.partial`, chmod 0o755, rename; Windows: staging folder, `tar.exe`, folder swap) -> write `nemo-asset.sha256` and `nemo-last-check` next to the program -> `ProgramSlot::set`. A process-wide `AtomicBool` keeps it to one worker at a time. The audio thread and the UI thread are never involved; resolves read the slot with `OnceLock::get`.

## Interfaces

```rust
// nemo-ytm/src/ytdlp.rs
#[derive(Debug, Clone, Default)]
pub struct ProgramSlot(/* Arc<OnceLock<PathBuf>> */);
impl ProgramSlot { pub fn set(&self, program: PathBuf) -> bool; pub fn get(&self) -> Option<&PathBuf>; }
impl YtDlpResolver { pub fn program_slot(&self) -> ProgramSlot; }

// nemo-ytm/src/ytdlp/locate.rs
pub fn managed_dir() -> Option<PathBuf>;

// nemo-ytm/src/ytdlp_install.rs
pub const LINUX_ASSET: &str = "yt-dlp_linux";
pub const LINUX_ZIP_ASSET: &str = "yt-dlp_linux.zip"; // fast-start
pub fn install_linux_onedir(target_dir: &Path) -> Result<PathBuf, InstallError>; // unpacks with `unzip`
pub fn unzip_on_path() -> bool;
pub const SUMS: &str = "SHA2-256SUMS";
pub fn install_onefile(target_dir: &Path) -> Result<PathBuf, InstallError>;

// nemo-ytm/src/ytdlp/managed.rs
pub const CHECK_INTERVAL: Duration; // 24 h
pub enum Build { WindowsOnedir, LinuxOnefile, LinuxOnedir } // LinuxOnedir when `unzip` is on PATH (fast-start)
pub struct ManagedYtDlp { /* dir, build */ }
pub struct Startup<'a> { pub auto_install: bool, pub user_set: bool, pub found: Option<&'a Path> }
pub enum Plan { Leave, Install, CheckForUpdate }
pub fn plan(start: &Startup<'_>, managed: &ManagedYtDlp, last_check: Option<SystemTime>, now: SystemTime) -> Plan;
pub fn check_due(last: Option<SystemTime>, now: SystemTime) -> bool;
pub fn update_needed(installed: Option<&str>, published: &str) -> bool;
pub fn start_in_background(auto_install: bool, user_set: bool, found: Option<PathBuf>, slot: ProgramSlot);

// nemo-app/src/config.rs
pub yt_dlp_auto_install: bool, // default true
```

## Error Handling

- Every failure is an `InstallError` with a fixed text (no URL). The worker logs one `warn` and stops; the check time is not written, so the next start tries again.
- A checksum mismatch discards the download before anything touches the folder. The Linux rename and the Windows folder swap keep the old copy until the new one is in place.
- A missing or unreadable hash file means "update needed"; an unreadable check-time file means "check due". A check time in the future (clock moved back) also counts as due.

## Testing Strategy

No real network in tests.
- `ytdlp_install`: checksum lines (both assets), hash mismatch refused, one-file placement through a temp file with mode 0o755 and replacement.
- `managed`: install when nothing is found or only a copy on `PATH` is (that copy is left as it is); a settings path left alone; the 24 h throttle (never checked, just checked, exactly 24 h, clock moved back); update decision (same hash, different hash, file missing); program path per build; platform build and folder agree with `locate_yt_dlp`; hash and check-time files round trip and reject garbage; a filled slot switches every resolver clone.
- `nemo-app`: the switch defaults to on, an old settings file reads as on, off round-trips.
- Manual (Linux, once): `cargo run -p nemo-ytm --example install_yt_dlp -- <scratch>/yt-dlp` (2 requests), then `yt-dlp --version` from that copy.
- No audio code changes; no bit-exact test needed.

## Risks and Alternatives

- Download trust: only TLS and the published `SHA2-256SUMS` list protect the download, and both come from the same GitHub release. The `.sig` GPG signature is not checked (no OpenPGP crate in tech.md). A compromised release would be installed.
- Redistribution: yt-dlp is downloaded at run time, not bundled, so the release archive does not redistribute it (tech.md).
- A Windows onedir copy a developer installed by hand with the example sits in the same folder and counts as app-managed, so it is updated too.
- Windows: a reinstall while `yt-dlp.exe` is running fails the folder swap; it is logged and retried at the next start.
- The asset is held in memory while hashed (about 41 MB for the 2026.10.26 Linux build).
- Alternative: rebuilding the resolver after install. Rejected: the resolver is moved into the player chain at startup; a set-once slot is simpler and lock-free.
