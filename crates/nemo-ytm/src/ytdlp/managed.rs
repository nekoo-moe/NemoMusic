//! The app-managed `yt-dlp` copy (ytdlp-auto-install spec).
//!
//! When the user set no path and [`locate_yt_dlp`](super::locate_yt_dlp) finds nothing, the
//! app installs its own copy into [`managed_dir`](super::managed_dir) on a worker thread. That
//! copy, and only that copy, is checked for updates at most once per [`CHECK_INTERVAL`]: the
//! small `SHA2-256SUMS` list is downloaded and the asset is fetched again only when its hash
//! differs from the one recorded at install time. A `yt-dlp` the user installed (`PATH`,
//! winget, a path in the settings) is never touched. A failure logs a warning and is retried at
//! the next start.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::ProgramSlot;
use crate::timing;
use crate::ytdlp_install::{
    self, ASSET, InstallError, LINUX_ASSET, LINUX_ZIP_ASSET, checksum_for, fetch_asset, fetch_sums,
    unzip_on_path,
};

/// Shortest time between two update checks of the app-managed copy.
pub const CHECK_INTERVAL: Duration = Duration::from_hours(24);
/// File next to the program holding the SHA-256 of the installed release asset.
const HASH_FILE: &str = "nemo-asset.sha256";
/// File next to the program holding the time of the last finished check (Unix seconds).
const CHECK_FILE: &str = "nemo-last-check";

/// Set while a worker runs, so there is only ever one download at a time.
static RUNNING: AtomicBool = AtomicBool::new(false);

/// Set from the start of the app until the startup worker ends, when the app had no `yt-dlp`
/// to run and the worker may install one (cold-start spec): a resolve that finds no program
/// waits for it instead of failing.
static PENDING: AtomicBool = AtomicBool::new(false);

/// Longest a resolve waits for the installer.
pub const INSTALL_WAIT: Duration = Duration::from_secs(90);
/// How often a waiting resolve looks at the installer.
const WAIT_STEP: Duration = Duration::from_millis(100);

/// Clears a pending flag when the worker ends, whichever way it ends.
struct Pending<'a>(&'a AtomicBool);

impl Drop for Pending<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Wait for the installer to hand over a program, for at most [`INSTALL_WAIT`]. `None` when no
/// install is running (or it ended without a program, or the time ran out).
pub async fn wait_for_install(slot: &ProgramSlot) -> Option<PathBuf> {
    wait_until(slot, &PENDING, INSTALL_WAIT).await
}

async fn wait_until(
    slot: &ProgramSlot,
    pending: &AtomicBool,
    longest: Duration,
) -> Option<PathBuf> {
    let began = Instant::now();
    loop {
        // The flag is read first: the worker sets the slot before it clears the flag, so a
        // cleared flag means the slot is final.
        let running = pending.load(Ordering::Acquire);
        if let Some(program) = slot.get() {
            return Some(program.clone());
        }
        if !running || began.elapsed() >= longest {
            return None;
        }
        tokio::time::sleep(WAIT_STEP).await;
    }
}

/// The release build this platform installs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Build {
    /// Windows x64: the onedir zip, unpacked into the folder.
    WindowsOnedir,
    /// Linux `x86_64` without `unzip`: the one-file program (it unpacks itself on every run).
    LinuxOnefile,
    /// Linux `x86_64` with `unzip`: the onedir zip, unpacked into the folder (starts faster).
    LinuxOnedir,
}

impl Build {
    /// The build for this operating system and architecture; `None` where there is no install.
    #[must_use]
    pub fn for_this_platform() -> Option<Self> {
        if cfg!(all(windows, target_arch = "x86_64")) {
            Some(Self::WindowsOnedir)
        } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Some(if unzip_on_path() {
                Self::LinuxOnedir
            } else {
                Self::LinuxOnefile
            })
        } else {
            None
        }
    }

    /// The release asset name, as listed in `SHA2-256SUMS`.
    #[must_use]
    pub fn asset(self) -> &'static str {
        match self {
            Self::WindowsOnedir => ASSET,
            Self::LinuxOnefile => LINUX_ASSET,
            Self::LinuxOnedir => LINUX_ZIP_ASSET,
        }
    }

    /// The program's file name inside the folder.
    #[must_use]
    pub fn program_name(self) -> &'static str {
        match self {
            Self::WindowsOnedir => "yt-dlp.exe",
            Self::LinuxOnefile | Self::LinuxOnedir => "yt-dlp",
        }
    }

    fn place(self, dir: &Path, bytes: &[u8]) -> Result<PathBuf, InstallError> {
        match self {
            Self::WindowsOnedir => ytdlp_install::unpack_onedir(dir, bytes),
            Self::LinuxOnefile => ytdlp_install::place_onefile(dir, bytes),
            Self::LinuxOnedir => ytdlp_install::unpack_linux_onedir(dir, bytes),
        }
    }
}

/// The app-managed copy: its folder and the build that goes there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedYtDlp {
    dir: PathBuf,
    build: Build,
}

impl ManagedYtDlp {
    /// A managed copy in `dir` (tests use a temp folder).
    #[must_use]
    pub fn new(dir: PathBuf, build: Build) -> Self {
        Self { dir, build }
    }

    /// The managed copy in [`managed_dir`](super::managed_dir); `None` on platforms without an
    /// install or when the data folder is unknown.
    #[must_use]
    pub fn for_this_platform() -> Option<Self> {
        Some(Self::new(
            super::managed_dir()?,
            Build::for_this_platform()?,
        ))
    }

    /// Where the program is (or will be) installed.
    #[must_use]
    pub fn program(&self) -> PathBuf {
        self.dir.join(self.build.program_name())
    }

    /// The SHA-256 recorded at install time; `None` when missing or unreadable.
    #[must_use]
    pub fn installed_hash(&self) -> Option<String> {
        let text = std::fs::read_to_string(self.dir.join(HASH_FILE)).ok()?;
        let hash = text.trim();
        (hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| hash.to_ascii_lowercase())
    }

    /// When the last check finished; `None` when it never did.
    #[must_use]
    pub fn last_check(&self) -> Option<SystemTime> {
        let text = std::fs::read_to_string(self.dir.join(CHECK_FILE)).ok()?;
        let secs = text.trim().parse::<u64>().ok()?;
        UNIX_EPOCH.checked_add(Duration::from_secs(secs))
    }

    /// Record a finished check at `now`, and the new asset hash after an install. Best effort:
    /// a missing record only means the next start checks again.
    fn record(&self, installed: Option<&str>, now: SystemTime) {
        if let Some(hash) = installed
            && let Err(e) = std::fs::write(self.dir.join(HASH_FILE), format!("{hash}\n"))
        {
            tracing::warn!(error = %e.kind(), "cannot record the yt-dlp hash");
        }
        let secs = now.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        if let Err(e) = std::fs::write(self.dir.join(CHECK_FILE), format!("{secs}\n")) {
            tracing::warn!(error = %e.kind(), "cannot record the yt-dlp check time");
        }
    }
}

/// What the app knows about `yt-dlp` at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Startup<'a> {
    /// The `yt_dlp_auto_install` setting.
    pub auto_install: bool,
    /// The user set a `yt-dlp` path in the settings.
    pub user_set: bool,
    /// The program in use: the user's path or what [`locate_yt_dlp`](super::locate_yt_dlp)
    /// found.
    pub found: Option<&'a Path>,
}

/// What the startup worker does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    /// Nothing: switched off, the user's own copy, or a check ran recently.
    Leave,
    /// No `yt-dlp` anywhere: install the managed copy.
    Install,
    /// The managed copy is in use and due for a check.
    CheckForUpdate,
}

/// Decide what to do. `last_check` is [`ManagedYtDlp::last_check`].
#[must_use]
pub fn plan(
    start: &Startup<'_>,
    managed: &ManagedYtDlp,
    last_check: Option<SystemTime>,
    now: SystemTime,
) -> Plan {
    if !start.auto_install || start.user_set {
        return Plan::Leave;
    }
    match start.found {
        Some(found) if found == managed.program() => {
            if check_due(last_check, now) {
                Plan::CheckForUpdate
            } else {
                Plan::Leave
            }
        }
        // Nothing found, or a copy on `PATH` or from winget: distribution packages lag behind
        // YouTube by months (Ubuntu 24.04 ships 2024.04.09, which YouTube now refuses), so the
        // app installs and uses its own copy. The other copy is left as it is.
        None | Some(_) => Plan::Install,
    }
}

/// True when no check finished in the last [`CHECK_INTERVAL`]. A check time in the future (the
/// clock moved back) counts as due, so a wrong clock cannot stop updates for good.
#[must_use]
pub fn check_due(last: Option<SystemTime>, now: SystemTime) -> bool {
    last.is_none_or(|last| {
        now.duration_since(last)
            .map_or(true, |age| age >= CHECK_INTERVAL)
    })
}

/// True when the published asset differs from the installed one, or nothing was recorded.
#[must_use]
pub fn update_needed(installed: Option<&str>, published: &str) -> bool {
    installed.is_none_or(|hash| !hash.eq_ignore_ascii_case(published))
}

/// Run the startup plan on a worker thread named `yt-dlp-install`, never on the caller's
/// thread. After an install, `slot` points every resolver clone at the new program.
pub fn start_in_background(
    auto_install: bool,
    user_set: bool,
    found: Option<PathBuf>,
    slot: ProgramSlot,
) {
    // Nothing to run yet and an install allowed: resolves wait for the worker.
    let waiting = auto_install && !user_set && found.is_none();
    if waiting {
        PENDING.store(true, Ordering::Release);
    }
    let spawned = std::thread::Builder::new()
        .name("yt-dlp-install".into())
        .spawn(move || {
            let _pending = waiting.then(|| Pending(&PENDING));
            let start = Startup {
                auto_install,
                user_set,
                found: found.as_deref(),
            };
            run_startup(&start, &slot);
        });
    if let Err(e) = spawned {
        PENDING.store(false, Ordering::Release);
        tracing::warn!(error = %e, "cannot start the yt-dlp install worker");
    }
}

fn run_startup(start: &Startup<'_>, slot: &ProgramSlot) {
    let Some(managed) = ManagedYtDlp::for_this_platform() else {
        if start.auto_install && !start.user_set && start.found.is_none() {
            tracing::warn!("yt-dlp not found and there is no automatic install for this platform");
        }
        return;
    };
    let now = SystemTime::now();
    let plan = plan(start, &managed, managed.last_check(), now);
    if plan == Plan::Leave {
        return;
    }
    if RUNNING.swap(true, Ordering::AcqRel) {
        return;
    }
    let began = Instant::now();
    let done = run(&managed, plan, now);
    timing::stage("yt-dlp install worker", began, done.is_ok());
    match done {
        Ok(Some(program)) => {
            tracing::info!(update = plan == Plan::CheckForUpdate, "yt-dlp installed");
            // Already set only when an earlier install in this run set the same path.
            let _ = slot.set(program);
        }
        Ok(None) => tracing::debug!("yt-dlp is up to date"),
        Err(e) => tracing::warn!(error = %e, "yt-dlp install failed; retrying at the next start"),
    }
    RUNNING.store(false, Ordering::Release);
}

/// Download the checksum list, then the asset when needed. `Ok(None)`: already up to date.
fn run(
    managed: &ManagedYtDlp,
    plan: Plan,
    now: SystemTime,
) -> Result<Option<PathBuf>, InstallError> {
    let client = ytdlp_install::client()?;
    let sums = fetch_sums(&client)?;
    let asset = managed.build.asset();
    let published = checksum_for(&sums, asset).ok_or(InstallError::NoChecksum)?;
    if plan == Plan::CheckForUpdate
        && !update_needed(managed.installed_hash().as_deref(), &published)
    {
        managed.record(None, now);
        return Ok(None);
    }
    // Human-paced: one request at a time, a short pause between them.
    std::thread::sleep(Duration::from_secs(1));
    let bytes = fetch_asset(&client, asset, &published)?;
    let program = managed.build.place(&managed.dir, &bytes)?;
    managed.record(Some(&published), now);
    Ok(Some(program))
}

#[cfg(test)]
mod tests;
