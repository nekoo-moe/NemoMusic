//! `NemoMusic` application binary. Wiring only; no business logic (see `structure.md`).

mod paths;

use paths::AppPaths;

fn main() {
    match AppPaths::resolve(None) {
        Ok(paths) => match paths.create_all() {
            Ok(()) => println!(
                "NemoMusic {} (data: {})",
                env!("CARGO_PKG_VERSION"),
                paths.root.display()
            ),
            Err(err) => eprintln!("NemoMusic: cannot create data folders: {err}"),
        },
        Err(err) => eprintln!("NemoMusic: {err}"),
    }
}
