//! Install the official yt-dlp build by hand (development aid). The app does the same on its
//! own when it finds no yt-dlp (`nemo_ytm::ytdlp::managed`).
//!
//! `cargo run -p nemo-ytm --example install_yt_dlp [-- <target folder>]`
//!
//! Windows x64: the onedir build; Linux `x86_64`: the onedir `yt-dlp_linux.zip` when `unzip` is
//! installed, else the one-file `yt-dlp_linux`. Default target:
//! `<data>/NemoMusic/bin/yt-dlp`, the first place `locate_yt_dlp` looks. Downloads two files
//! from the latest GitHub release and checks the SHA-256 before anything is put in place.
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "an example reports what it did"
)]

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use nemo_ytm::ytdlp::managed::Build;
use nemo_ytm::ytdlp::{
    install_linux_onedir, install_onedir, install_onefile, locate_yt_dlp, managed_dir,
};

fn main() -> ExitCode {
    let Some(build) = Build::for_this_platform() else {
        eprintln!("no yt-dlp build is installed on this platform; use your package manager");
        return ExitCode::FAILURE;
    };
    let target = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .or_else(managed_dir);
    let Some(target) = target else {
        eprintln!("the data folder is not set; pass a target folder");
        return ExitCode::FAILURE;
    };
    let start = Instant::now();
    let installed = match build {
        Build::WindowsOnedir => install_onedir(&target),
        Build::LinuxOnefile => install_onefile(&target),
        Build::LinuxOnedir => install_linux_onedir(&target),
    };
    match installed {
        Ok(program) => {
            println!(
                "installed {} in {:.1} s (checksum verified)",
                program.display(),
                start.elapsed().as_secs_f64()
            );
            for _ in 0..2 {
                let run = Instant::now();
                let out = std::process::Command::new(&program)
                    .arg("--version")
                    .output();
                match out {
                    Ok(o) => println!(
                        "yt-dlp --version: {} in {} ms",
                        String::from_utf8_lossy(&o.stdout).trim(),
                        run.elapsed().as_millis()
                    ),
                    Err(e) => println!("could not run it: {}", e.kind()),
                }
            }
            println!(
                "locate_yt_dlp now finds: {}",
                locate_yt_dlp().map_or_else(|| "nothing".to_owned(), |p| p.display().to_string())
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("install failed: {e}");
            ExitCode::FAILURE
        }
    }
}
