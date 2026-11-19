//! `NemoMusic` application binary. Wiring only; no business logic (see `structure.md`).

#![windows_subsystem = "windows"]

mod backend;
#[cfg(windows)]
mod blur;
mod config;
mod folder_picker;
mod logging;
mod merged;
mod opt_in_gate;
mod os_motion;
mod paths;
mod play_hub;
mod restart;
mod soundcloud;
mod start_warm;
mod ui_bridge;

use std::sync::{Arc, Mutex};

use paths::AppPaths;

fn main() {
    // `nemo-app --signin --data-dir <folder>`: this process is the sign-in window that
    // `nemo-ytm` started. It runs before paths, logging, tokio or iced, so tao's event loop is
    // the only one in the process (tech.md, Sign-in window).
    let mut args = std::env::args_os().skip(1);
    if args
        .next()
        .is_some_and(|first| first == nemo_signin::SIGNIN_ARG)
    {
        std::process::exit(nemo_signin::run(args));
    }

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
    // The window and the splash open at once; the services open on their own thread behind the
    // first frame (startup-flow spec).
    let (mut options, saver) = ui_bridge::options(settings.clone(), paths.root.clone());
    // `Add a folder`: the native dialog, storing the folder and adding it to the running library.
    let library = folder_picker::LibrarySlot::default();
    options.on_add_music_folder = Some(folder_picker::native(saver.clone(), Arc::clone(&library)));
    let hub = Arc::new(Mutex::new(None));
    let warm = Arc::new(Mutex::new(None));
    let startup = backend::Startup {
        paths,
        settings,
        handle: runtime.handle().clone(),
        saver,
        hub: Arc::clone(&hub),
        library,
        warm: Arc::clone(&warm),
    };
    if let Err(err) =
        nemo_ui::run_with_startup(Box::new(move |report| startup.run(report)), options)
    {
        tracing::error!(error = %err, "the window closed with an error");
    }
    // End the warm-up, if it is still running.
    drop(warm.lock().ok().and_then(|mut slot| slot.take()));
    // End the last play, so observers see its TrackEnded.
    if let Some(hub) = hub.lock().ok().and_then(|mut slot| slot.take()) {
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
