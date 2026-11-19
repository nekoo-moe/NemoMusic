//! Build the real [`Backend`] the UI talks to.

use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nemo_audio::decoder::supported_stream_codecs;
use nemo_audio::player::{Player, PlayerDeps};
use nemo_audio::sink::{DeviceSinkFactory, OutputSelection};
use nemo_core::account::{AccountService, NoAccount, SignOutWipe};
use nemo_core::ai::{NoAi, NoTranslator};
use nemo_core::hooks::PlayObserver;
use nemo_core::integrations::NoIntegrations;
use nemo_core::notify::NoNotifier;
use nemo_core::player::PlayerCommand;
use nemo_core::segment::SegmentSource;
use nemo_core::services::{Backend, StreamOpener};
use nemo_core::stats::NoStats;
use nemo_core::stream::StreamRequest;
use nemo_library::catalog::LOCAL_ALBUM_PREFIX;
use nemo_library::{LocalCatalog, LocalLibrary};
use nemo_lyrics::LyricsHub;
use nemo_ui::StepReporter;
use nemo_ui::splash::StartStep;
use nemo_ytm::account::{AccountCatalog, AccountConfig, GoogleAccount, SessionSlot};
use nemo_ytm::catalog::YtmCatalog;
use nemo_ytm::images::ImageCache;
use nemo_ytm::offline::{OfflineCache, SystemClock};
use nemo_ytm::resolver::{StreamStore, persisted_chain};
use nemo_ytm::session::YtmSessionBackend;
use nemo_ytm::sponsorblock::{ReqwestHttp, SponsorBlockSource};
use nemo_ytm::stream::HttpStreamOpener;
use nemo_ytm::warmup::WarmUp;
use nemo_ytm::ytdlp::{YtDlpResolver, locate_yt_dlp, managed};
use tokio::runtime::Handle;

use crate::config::{Settings, StoredDevice};
use crate::merged::MergedCatalog;
use crate::paths::AppPaths;
use crate::play_hub::{self, PlayHub};
use crate::soundcloud::{self, RoutingOpener};
use crate::start_warm::{LastPlayed, StartupWarm, WarmSlot};
use crate::ui_bridge::SettingsMsg;

/// Starting a service failed. The message is user-safe.
#[derive(Debug, thiserror::Error)]
#[error("cannot start {service}: {reason}")]
pub struct StartError {
    service: &'static str,
    reason: String,
}

fn fail(service: &'static str) -> impl FnOnce(String) -> StartError {
    move |reason| StartError { service, reason }
}

/// Everything the app keeps alive besides the backend.
pub struct Services {
    /// The UI-facing services.
    pub backend: Backend,
    /// The `yt-dlp` resolver of the play path, kept for the start-up warm-up.
    pub ytdlp: YtDlpResolver,
    /// The streams kept between runs; the last played song is read from it at start-up.
    pub streams: Arc<StreamStore>,
    /// The start-up resolve-ahead budget (the last played song, the first songs of Home).
    pub start_warm: Arc<StartupWarm>,
    /// The local library, rescanned on a worker thread.
    pub local: LocalCatalog,
}

/// Everything the start function needs once the window and the splash are up (startup-flow).
pub struct Startup {
    /// The app's folders.
    pub paths: AppPaths,
    /// The stored settings.
    pub settings: Settings,
    /// The network runtime, alive for the whole app.
    pub handle: Handle,
    /// The settings saver, so the output watcher can store device changes.
    pub saver: Sender<SettingsMsg>,
    /// Where the play hub goes, so `main` can stop it after the window closes.
    pub hub: Arc<Mutex<Option<PlayHub>>>,
    /// Where the running library goes, so the folder dialog can add a folder to it.
    pub library: crate::folder_picker::LibrarySlot,
    /// Where the start-up warm-up goes, so `main` can end it after the window closes.
    pub warm: Arc<Mutex<Option<WarmUp>>>,
}

/// How long after the services are up the warm-up starts: the first frame is long drawn by then.
const WARM_UP_PAUSE: Duration = Duration::from_secs(1);

impl Startup {
    /// Open the services behind the first frame, reporting each real step. The library scan, the
    /// play hub and the output watcher start here too, so the window never waits for them.
    ///
    /// # Errors
    /// A plain sentence for the splash when a service cannot start.
    pub fn run(self, report: StepReporter<'_>) -> Result<Backend, String> {
        let services = build(&self.paths, &self.settings, &self.handle, report)
            .map_err(|err| format!("NemoMusic could not start. {err}"))?;
        rescan_in_background(&services.local);
        if let Ok(mut slot) = self.library.lock() {
            *slot = Some(services.local.clone());
        }
        // The song that played last is resolved ahead now, before Home brings its own hints.
        services.start_warm.hint(services.streams.last_played());
        match play_hub::spawn(
            Arc::clone(&services.backend.player),
            play_observers(&services.streams),
        ) {
            Ok(hub) => {
                if let Ok(mut slot) = self.hub.lock() {
                    *slot = Some(hub);
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, "cannot start the play hub; play events are off");
            }
        }
        crate::ui_bridge::spawn_output_watcher(&services.backend.player, self.saver);
        // After the first frame (this runs behind the splash): the HTTP client of the stream
        // opener and one `yt-dlp --version`, so the first song does not pay for them.
        let warm = WarmUp::start(
            &self.handle,
            services.ytdlp,
            HttpStreamOpener::new(),
            WARM_UP_PAUSE,
        );
        if let Ok(mut slot) = self.warm.lock() {
            *slot = Some(warm);
        }
        Ok(services.backend)
    }
}

/// The SponsorBlock source, or `None` (with a warning) when its HTTP client cannot start.
fn sponsorblock_source(handle: &Handle) -> Option<Arc<dyn SegmentSource>> {
    match ReqwestHttp::new(handle.clone()) {
        Ok(http) => Some(Arc::new(SponsorBlockSource::new(http))),
        Err(err) => {
            tracing::warn!(error = %err, "SponsorBlock disabled");
            None
        }
    }
}

/// Wire the services. Must be called inside the tokio runtime that `handle` belongs to stays
/// alive for the whole app. `report` is told as each step finishes.
///
/// # Errors
/// Returns a [`StartError`] when the image cache or the lyrics store cannot start.
pub fn build(
    paths: &AppPaths,
    settings: &Settings,
    handle: &Handle,
    report: StepReporter<'_>,
) -> Result<Services, StartError> {
    let local = LocalCatalog::new(LocalLibrary::new(settings.music_folders.clone()));
    let account_slot: SessionSlot = SessionSlot::default();
    let offline = open_offline(paths);
    // Streams resolved in the last hours, kept between runs (cold-start spec).
    let streams = Arc::new(StreamStore::open(paths.streams.clone()));
    report(StartStep::Library);
    let account = build_account(paths, handle, &account_slot, offline, &streams);
    report(StartStep::CredentialStore);
    // Signed out: the anonymous catalog. Signed in: the account's session, switched live.
    let remote = AccountCatalog::new(Arc::new(YtmCatalog::new(handle.clone())), account_slot);
    // The first songs of Home are resolved ahead once the resolver exists (start_warm).
    let warm_slot = WarmSlot::default();
    let home_slot = Arc::clone(&warm_slot);
    let catalog = MergedCatalog::new(
        Arc::new(local.clone()),
        Arc::new(remote),
        LOCAL_ALBUM_PREFIX,
    )
    .with_home_hint(Arc::new(move |feed| {
        if let Some(warm) = home_slot.get() {
            warm.hint_home(feed);
        }
    }));

    let user_set = settings.yt_dlp_path.is_some();
    let program = settings.yt_dlp_path.clone().or_else(locate_yt_dlp);
    if program.is_none() {
        tracing::warn!("yt-dlp not found; YouTube Music playback will use the native path only");
    }
    let ytdlp = program
        .clone()
        .map_or_else(YtDlpResolver::new, YtDlpResolver::with_program)
        .with_allowed_codecs(supported_stream_codecs())
        .with_handle(handle.clone());
    // Install or update the app's own copy on a worker thread; startup does not wait. A new
    // install reaches playback through the slot, without a restart.
    managed::start_in_background(
        settings.yt_dlp_auto_install,
        user_set,
        program,
        ytdlp.program_slot(),
    );
    // Cache -> native player request -> yt-dlp (fast arguments, then the full extraction).
    let cipher = build_cipher(paths, handle);
    let resolver = persisted_chain_with(
        ytdlp.clone(),
        supported_stream_codecs(),
        handle,
        Some(Arc::clone(&streams)),
        cipher.clone(),
    );
    let start_warm = StartupWarm::new(Arc::clone(&resolver));
    // Already set only if `build` ran twice on one slot, which it does not.
    let _ = warm_slot.set(Arc::clone(&start_warm));

    let segments = sponsorblock_source(handle);

    // SoundCloud exists only when the switch is on and the notice confirmed (soundcloud-full).
    let sc = soundcloud::build(&settings.opt_ins, handle, || soundcloud::real_http(handle));
    let files: Arc<dyn StreamOpener> = Arc::new(HttpStreamOpener::default());
    let opener: Arc<dyn StreamOpener> = match &sc {
        Some(parts) => Arc::new(RoutingOpener::new(files, Arc::new(parts.opener.clone()))),
        None => files,
    };

    let player = Player::spawn(PlayerDeps {
        resolver,
        // Automatic language, no Premium, no codec hint until the account settings build it.
        request: StreamRequest::default(),
        // No library store yet: every local file plays whole.
        local: None,
        // No offline cache wired yet (offline-cache): every song streams.
        offline: None,
        url_resolver: sc.as_ref().map(|parts| Arc::clone(&parts.resolver)),
        opener,
        segments,
        // The shared-mode factory opens by ID through `DeviceSinkFactory` (output-devices task 7).
        sink_factory: Arc::new(DeviceSinkFactory::new(OutputSelection {
            device: settings
                .output_device
                .as_ref()
                .and_then(StoredDevice::saved)
                .map(|s| s.id),
            options: settings.output_options.to_core(),
        })),
        device_enumerator: None,
        device_watch_interval: None,
    });
    for command in start_commands(settings) {
        player.send(command);
    }
    report(StartStep::Player);

    let lyrics = LyricsHub::new(&paths.lyrics, handle.clone(), Vec::new())
        .map_err(|e| fail("lyrics")(e.to_string()))?;
    let images = ImageCache::new(paths.images.clone(), handle.clone())
        .map_err(|e| fail("covers")(e.to_string()))?;

    Ok(Services {
        ytdlp,
        streams,
        start_warm,
        backend: Backend {
            catalog: Arc::new(catalog),
            player,
            lyrics: Arc::new(lyrics),
            images: Arc::new(images),
            account,
            integrations: Arc::new(NoIntegrations),
            notifier: Arc::new(NoNotifier),
            soundcloud: sc.map(|parts| parts.catalog),
            stats: Arc::new(NoStats),
            comments: None,
            ratings: None,
            ai: Arc::new(NoAi),
            translator: Arc::new(NoTranslator),
        },
        local,
    })
}

/// The saved player settings, sent once after the player starts: volume, SponsorBlock, and the
/// effects and transition (all off unless the user turned one on).
fn start_commands(settings: &Settings) -> [PlayerCommand; 4] {
    [
        PlayerCommand::SetVolume(settings.volume_percent),
        PlayerCommand::SetSkipSegments(settings.sponsorblock),
        PlayerCommand::SetEffects(settings.effects.to_core()),
        PlayerCommand::SetTransition(settings.transition.to_core()),
    ]
}

/// The Google account service, or `NoAccount` when the OS credential store is missing.
fn build_account(
    paths: &AppPaths,
    handle: &Handle,
    slot: &SessionSlot,
    offline: Option<Arc<OfflineCache>>,
    streams: &Arc<StreamStore>,
) -> Arc<dyn AccountService> {
    let store = match nemo_secrets::open_platform_store() {
        Ok(store) => store,
        Err(err) => {
            tracing::warn!(error = %err, "sign-in disabled");
            return Arc::new(NoAccount);
        }
    };
    let config = AccountConfig {
        helper: signin_program(),
        helper_args: vec![nemo_signin::SIGNIN_ARG.into()],
        webview_dir: paths.webview.clone(),
        cache_file: paths.account.join("account.json"),
    };
    let account = GoogleAccount::new(
        config,
        store,
        Arc::new(YtmSessionBackend::new(handle.clone())),
        slot.clone(),
        handle.clone(),
    );
    for step in sign_out_steps(offline, Arc::clone(streams)) {
        account.register_wipe(step);
    }
    Arc::new(account)
}

/// The sign-in window is this same executable started again with `--signin` (see `main`).
/// Without a known path, starting the window fails with a clear message and cookie paste still
/// works.
fn signin_program() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|err| {
        tracing::warn!(error = %err, "cannot find the app executable; the sign-in window is off");
        PathBuf::new()
    })
}

/// The offline cache in `AppPaths.offline` (offline-cache), or `None` when it cannot be
/// opened; then nothing is saved offline this session.
fn open_offline(paths: &AppPaths) -> Option<Arc<OfflineCache>> {
    match OfflineCache::open(&paths.offline, Arc::new(SystemClock)) {
        Ok(cache) => Some(Arc::new(cache)),
        Err(err) => {
            tracing::warn!(error = %err, "the offline cache is off this session");
            None
        }
    }
}

/// The one place where crates that keep account data register their sign-out step
/// (ADR 0004, Decision 3). Sign-out runs them in this order, then removes the session.
/// The offline cache is wiped whole, whichever account signs out; so is the file of kept
/// streams (their addresses can carry the listener's IP).
fn sign_out_steps(
    offline: Option<Arc<OfflineCache>>,
    streams: Arc<StreamStore>,
) -> Vec<Arc<dyn SignOutWipe>> {
    offline
        .into_iter()
        .map(|cache| cache as Arc<dyn SignOutWipe>)
        .chain(std::iter::once(streams as Arc<dyn SignOutWipe>))
        .collect()
}

/// The one place where services that watch playback register as play observers (integrations,
/// listening stats). The play hub hands each of them every play event.
pub fn play_observers(streams: &Arc<StreamStore>) -> Vec<Arc<dyn PlayObserver>> {
    vec![Arc::new(LastPlayed(Arc::clone(streams)))]
}

/// Rescan the local library on its own thread so the window opens at once.
pub fn rescan_in_background(local: &LocalCatalog) {
    let library = local.library();
    let spawned = std::thread::Builder::new()
        .name("library-scan".into())
        .spawn(move || {
            let mut fresh = match library.read() {
                Ok(lib) => LocalLibrary::new(lib.folders().to_vec()),
                Err(_) => return,
            };
            fresh.rescan();
            let count = fresh.tracks().len();
            if let Ok(mut lib) = library.write() {
                *lib = fresh;
            }
            tracing::info!(tracks = count, "local library scanned");
        });
    if let Err(err) = spawned {
        tracing::warn!(error = %err, "cannot start the library scan");
    }
}

#[cfg(test)]
mod tests {
    use nemo_core::effects::{EffectsSettings, TransitionSettings};

    use super::*;
    use crate::config::effects::StoredTransitionMode;

    #[test]
    fn the_player_gets_the_saved_effects_at_start_and_the_defaults_are_off() {
        let commands = start_commands(&Settings::default());
        assert!(commands.contains(&PlayerCommand::SetEffects(EffectsSettings::default())));
        assert!(commands.contains(&PlayerCommand::SetTransition(TransitionSettings::default())));

        let mut settings = Settings::default();
        settings.effects.reverb.enabled = true;
        settings.transition.mode = StoredTransitionMode::Crossfade;
        let commands = start_commands(&settings);
        assert!(commands.contains(&PlayerCommand::SetEffects(settings.effects.to_core())));
        assert!(commands.contains(&PlayerCommand::SetTransition(settings.transition.to_core())));
    }

    #[test]
    fn sign_out_wipes_the_offline_cache_folder() {
        use nemo_core::account::{AccountId, SignOutReport, WipeScope};

        let root = std::env::temp_dir().join(format!("nemo-app-wipe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root); // A leftover from an earlier run is fine to drop.
        let paths = AppPaths::under(&root);
        let streams = Arc::new(StreamStore::open(paths.streams.clone()));
        let steps = sign_out_steps(open_offline(&paths), Arc::clone(&streams));
        let names: Vec<_> = steps.iter().map(|s| s.name()).collect();
        assert_eq!(names, ["offline", "streams"]);
        assert!(paths.offline.join("offline.db").is_file());
        let mut report = SignOutReport::default();
        for step in &steps {
            report.run_step(step.as_ref(), WipeScope::Account(AccountId(1)));
        }
        assert!(report.failed_steps.is_empty() && report.items_removed >= 1);
        assert!(!paths.offline.exists());
        assert_eq!(sign_out_steps(None, streams).len(), 1);
        let _ = std::fs::remove_dir_all(root); // Cleanup only.
    }
}
