//! `NemoMusic` application binary. Wiring only; no business logic (see `structure.md`).

#![windows_subsystem = "windows"]

mod backend;
mod config;
mod logging;
mod merged;
mod opt_in_gate;
mod paths;
mod play_hub;
mod ui_bridge;

use paths::AppPaths;

fn main() {
    let paths = match AppPaths::resolve(None) {
        Ok(paths) => paths,
        Err(err) => fatal(&err.to_string()),
    };
    if let Err(err) = paths.create_all() {
        fatal(&format!("cannot create data folders: {err}"));
    }
    logging::init(&paths.logs);
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting");
    let settings = config::Settings::load(&paths.root);
    if let Err(err) = settings.save(&paths.root) {
        tracing::warn!(error = %err, "cannot save settings");
    }

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("nemo-io")
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => fatal(&format!("cannot start the network runtime: {err}")),
    };
    let services = match backend::build(&paths, &settings, runtime.handle()) {
        Ok(s) => s,
        Err(err) => fatal(&err.to_string()),
    };
    backend::rescan_in_background(&services.local);
    let hub = match play_hub::spawn(
        std::sync::Arc::clone(&services.backend.player),
        backend::play_observers(),
    ) {
        Ok(hub) => Some(hub),
        Err(err) => {
            tracing::warn!(error = %err, "cannot start the play hub; play events are off");
            None
        }
    };

    let options = ui_bridge::options(settings, paths.root.clone());
    if let Err(err) = nemo_ui::run(services.backend, options) {
        tracing::error!(error = %err, "the window closed with an error");
    }
    // End the last play, so observers see its TrackEnded.
    if let Some(hub) = hub {
        hub.stop();
    }
    tracing::info!("stopped");
}

/// Log, show a short message and exit. The window is not up yet, so stderr is all there is.
fn fatal(message: &str) -> ! {
    tracing::error!(message, "fatal");
    eprintln!("NemoMusic: {message}");
    std::process::exit(1);
}
