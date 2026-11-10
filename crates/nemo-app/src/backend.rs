//! Build the real [`Backend`] the UI talks to.

use std::sync::Arc;

use nemo_audio::decoder::supported_stream_codecs;
use nemo_audio::player::{Player, PlayerDeps};
use nemo_audio::sink::CpalSinkFactory;
use nemo_core::player::PlayerCommand;
use nemo_core::segment::SegmentSource;
use nemo_core::services::Backend;
use nemo_library::catalog::LOCAL_ALBUM_PREFIX;
use nemo_library::{LocalCatalog, LocalLibrary};
use nemo_lyrics::LyricsHub;
use nemo_ytm::catalog::YtmCatalog;
use nemo_ytm::images::ImageCache;
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
    let catalog = MergedCatalog::new(
        Arc::new(local.clone()),
        Arc::new(YtmCatalog::new(handle.clone())),
        LOCAL_ALBUM_PREFIX,
    );

    let program = settings.yt_dlp_path.clone().or_else(locate_yt_dlp);
    if program.is_none() {
        tracing::warn!("yt-dlp not found; YouTube Music playback will report it");
    }
    let resolver = program
        .map_or_else(YtDlpResolver::new, YtDlpResolver::with_program)
        .with_allowed_codecs(supported_stream_codecs())
        .with_handle(handle.clone());

    let segments: Option<Arc<dyn SegmentSource>> = match ReqwestHttp::new(handle.clone()) {
        Ok(http) => Some(Arc::new(SponsorBlockSource::new(http))),
        Err(err) => {
            tracing::warn!(error = %err, "SponsorBlock disabled");
            None
        }
    };

    let player = Player::spawn(PlayerDeps {
        resolver: Arc::new(resolver),
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
            account: Arc::new(nemo_core::account::NoAccount),
        },
        local,
    })
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
