//! Play a local FLAC or WAV file on the default output device (shared mode).
//!
//! `cargo run -p nemo-audio --example play -- <path>`

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use nemo_audio::decoder::decode_file;
use nemo_audio::engine::Engine;
use nemo_audio::sink::CpalSink;

fn run(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let audio = decode_file(path)?;
    println!("Source: {}", audio.info.badge());
    let sink = CpalSink::open(audio.format)?;
    // Two seconds of ring; the feeder thread keeps it topped up.
    let ring = usize::from(audio.format.channels) * audio.format.sample_rate as usize * 2;
    let mut engine = Engine::start(audio, sink, ring)?;
    println!("Signal path: {:#?}", engine.signal_path());
    while !engine.is_finished() {
        std::thread::sleep(Duration::from_millis(50));
    }
    // Let the device drain what it already holds.
    std::thread::sleep(Duration::from_millis(500));
    println!("Underruns: {}", engine.underruns());
    engine.stop();
    Ok(())
}

fn main() -> ExitCode {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: play <path to .flac or .wav>");
        return ExitCode::from(2);
    };
    match run(&path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}
