//! Build the real [`Backend`] the UI talks to.

use std::sync::Arc;

use nemo_audio::decoder::supported_stream_codecs;
use nemo_audio::player::{Player, PlayerDeps};
use nemo_audio::sink::CpalSinkFactory;
use nemo_core::account::{AccountService, NoAccount, SignOutWipe};
use nemo_core::player::PlayerCommand;
use nemo_core::segment::SegmentSource;
use nemo_core::services::Backend;
use nemo_core::stream::StreamRequest;
use nemo_library::catalog::LOCAL_ALBUM_PREFIX;
use nemo_library::{LocalCatalog, LocalLibrary};
use nemo_lyrics::LyricsHub;
use nemo_ytm::account::{
    AccountCatalog, AccountConfig, GoogleAccount, SessionSlot, default_helper_path,
};
use nemo_ytm::catalog::YtmCatalog;
use nemo_ytm::credentials::PlatformStore;
use nemo_ytm::images::ImageCache;
use nemo_ytm::resolver::default_chain;
use nemo_ytm::session::YtmSessionBackend;
use nemo_ytm::sponsorblock::{ReqwestHttp, SponsorBlockSource};
use nemo_ytm::stream::HttpStreamOpener;
use nemo_ytm::ytdlp::{YtDlpResolver, locate_yt_dlp};
use tokio::runtime::Handle;

use crate::config::Settings;
use crate::merged::MergedCatalog;
use crate::paths::AppPaths;

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
    /// The local library, rescanned on a worker thread.
    pub local: LocalCatalog,
}

/// Wire the services. Must be called inside the tokio runtime that `handle` belongs to stays
/// alive for the whole app.
///
/// # Errors
/// Returns a [`StartError`] when the image cache or the lyrics store cannot start.
pub fn build(
    paths: &AppPaths,
    settings: &Settings,
    handle: &Handle,
) -> Result<Services, StartError> {
    let local = LocalCatalog::new(LocalLibrary::new(settings.music_folders.clone()));
    let account_slot: SessionSlot = SessionSlot::default();
    let account = build_account(paths, handle, &account_slot);
    // Signed out: the anonymous catalog. Signed in: the account's session, switched live.
    let remote = AccountCatalog::new(Arc::new(YtmCatalog::new(handle.clone())), account_slot);
    let catalog = MergedCatalog::new(
        Arc::new(local.clone()),
        Arc::new(remote),
        LOCAL_ALBUM_PREFIX,
    );

    let program = settings.yt_dlp_path.clone().or_else(locate_yt_dlp);
    if program.is_none() {
        tracing::warn!("yt-dlp not found; YouTube Music playback will use the native path only");
    }
    let ytdlp = program
        .map_or_else(YtDlpResolver::new, YtDlpResolver::with_program)
        .with_allowed_codecs(supported_stream_codecs())
        .with_handle(handle.clone());
    // Cache -> native player request -> yt-dlp (fast arguments, then the full extraction).
    let resolver = default_chain(ytdlp, supported_stream_codecs(), handle);

    let segments: Option<Arc<dyn SegmentSource>> = match ReqwestHttp::new(handle.clone()) {
        Ok(http) => Some(Arc::new(SponsorBlockSource::new(http))),
        Err(err) => {
            tracing::warn!(error = %err, "SponsorBlock disabled");
            None
        }
    };

    let player = Player::spawn(PlayerDeps {
        resolver,
        // Automatic language, no Premium, no codec hint until the account settings build it.
        request: StreamRequest::default(),
        // No library store yet: every local file plays whole.
        local: None,
        opener: Arc::new(HttpStreamOpener::default()),
        segments,
        sink_factory: Arc::new(CpalSinkFactory::new(settings.output_device.clone())),
    });
    player.send(PlayerCommand::SetVolume(settings.volume_percent));
    player.send(PlayerCommand::SetSkipSegments(settings.sponsorblock));

    let lyrics = LyricsHub::new(&paths.lyrics, handle.clone(), Vec::new())
        .map_err(|e| fail("lyrics")(e.to_string()))?;
    let images = ImageCache::new(paths.images.clone(), handle.clone())
        .map_err(|e| fail("covers")(e.to_string()))?;

    Ok(Services {
        backend: Backend {
            catalog: Arc::new(catalog),
            player,
            lyrics: Arc::new(lyrics),
            images: Arc::new(images),
            account,
        },
        local,
    })
}

/// The Google account service, or `NoAccount` when the OS credential store or the helper is missing.
fn build_account(paths: &AppPaths, handle: &Handle, slot: &SessionSlot) -> Arc<dyn AccountService> {
    let store = match PlatformStore::new() {
        Ok(store) => store,
        Err(err) => {
            tracing::warn!(error = %err, "sign-in disabled");
            return Arc::new(NoAccount);
        }
    };
    let helper =
        default_helper_path().unwrap_or_else(|| paths.bin.join(nemo_ytm::account::HELPER_NAME));
    let config = AccountConfig {
        helper,
        webview_dir: paths.webview.clone(),
        cache_file: paths.account.join("account.json"),
    };
    let account = GoogleAccount::new(
        config,
        Arc::new(store),
        Arc::new(YtmSessionBackend::new(handle.clone())),
        slot.clone(),
        handle.clone(),
    );
    for step in sign_out_steps() {
        account.register_wipe(step);
    }
    Arc::new(account)
}

/// The one place where crates that keep account data register their sign-out step
/// (ADR 0004, Decision 3). Sign-out runs them in this order, then removes the session.
fn sign_out_steps() -> Vec<Arc<dyn SignOutWipe>> {
    Vec::new()
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
