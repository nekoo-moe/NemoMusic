//! `NemoMusic` application binary. Wiring only; no business logic (see `structure.md`).

mod config;
mod logging;
mod paths;

use paths::AppPaths;

fn main() {
    let paths = match AppPaths::resolve(None) {
        Ok(paths) => paths,
        Err(err) => {
            eprintln!("NemoMusic: {err}");
            std::process::exit(1);
        }
    };
    if let Err(err) = paths.create_all() {
        eprintln!("NemoMusic: cannot create data folders: {err}");
        std::process::exit(1);
    }
    logging::init(&paths.logs);
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting");
    let settings = config::Settings::load(&paths.root);
    tracing::info!(folders = settings.music_folders.len(), tier = ?settings.glass_tier, "settings loaded");
    if let Err(err) = settings.save(&paths.root) {
        tracing::warn!(error = %err, "cannot save settings");
    }
    println!(
        "NemoMusic {} (data: {})",
        env!("CARGO_PKG_VERSION"),
        paths.root.display()
    );
}
