//! Install the official yt-dlp onedir build for this user (development aid; the app never
//! downloads it on its own).
//!
//! `cargo run -p nemo-ytm --example install_yt_dlp [-- <target folder>]`
//!
//! Default target: `%LOCALAPPDATA%\NemoMusic\bin\yt-dlp`, the first place `locate_yt_dlp`
//! looks. Downloads two files from the latest GitHub release, checks the SHA-256, unpacks.
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "an example reports what it did"
)]

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use nemo_ytm::ytdlp::{ONEDIR_DIR, install_onedir, locate_yt_dlp};

fn main() -> ExitCode {
    let target = std::env::args().nth(1).map(PathBuf::from).or_else(|| {
        std::env::var_os("LOCALAPPDATA").map(|base| {
            PathBuf::from(base)
                .join("NemoMusic")
                .join("bin")
                .join(ONEDIR_DIR)
        })
    });
    let Some(target) = target else {
        eprintln!("LOCALAPPDATA is not set; pass a target folder");
        return ExitCode::FAILURE;
    };
    let start = Instant::now();
    match install_onedir(&target) {
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
