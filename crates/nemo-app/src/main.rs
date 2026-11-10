//! `NemoMusic` application binary. Wiring only; no business logic (see `structure.md`).

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
    println!(
        "NemoMusic {} (data: {})",
        env!("CARGO_PKG_VERSION"),
        paths.root.display()
    );
}
