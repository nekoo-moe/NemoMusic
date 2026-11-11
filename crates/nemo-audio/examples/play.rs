//! Play one or more local audio files as a queue on the default output device (shared mode).
//!
//! `cargo run -p nemo-audio --example play -- <file> [<file> ...]`
//!
//! Options (before the files): `--device <name>` picks an output device, `--volume <0-100>`,
//! `--list-devices` prints the output devices and exits.
//!
//! Audio is decoded incrementally. The next file is opened while the current one plays. Nothing
//! here proves what you hear: the program prints the signal path the player reports.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use nemo_audio::player::{Player, PlayerDeps};
use nemo_audio::sink::{CpalSinkFactory, list_output_devices};
use nemo_core::audio::Origin;
use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent};
use nemo_core::services::{ByteSource, StreamOpener};
use nemo_core::stream::{ResolvedStream, StreamError, StreamResolver};
use nemo_core::track::{Artwork, Track, TrackId, VideoId};

/// The example plays local files only, so it never resolves or opens a network stream.
struct NoNetwork;

impl StreamResolver for NoNetwork {
    fn resolve<'a>(
        &'a self,
        _video: &'a VideoId,
    ) -> nemo_core::BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        Box::pin(async { Err(StreamError::NotAvailable) })
    }
}

impl StreamOpener for NoNetwork {
    fn open(&self, _stream: &ResolvedStream) -> Result<Box<dyn ByteSource>, StreamError> {
        Err(StreamError::NotAvailable)
    }
}

struct Args {
    device: Option<String>,
    volume: u8,
    files: Vec<PathBuf>,
}

fn parse_args() -> Result<Option<Args>, String> {
    let mut args = Args {
        device: None,
        volume: 100,
        files: Vec::new(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--list-devices" => {
                for d in list_output_devices().map_err(|e| e.to_string())? {
                    let mark = if d.is_default { " (default)" } else { "" };
                    println!("{}{mark}", d.name);
                }
                return Ok(None);
            }
            "--device" => args.device = Some(it.next().ok_or("--device needs a name")?),
            "--volume" => {
                let v = it.next().ok_or("--volume needs a number")?;
                args.volume = v.parse().map_err(|_| "volume must be 0 to 100")?;
            }
            _ => args.files.push(PathBuf::from(a)),
        }
    }
    if args.files.is_empty() {
        return Err("usage: play [--device <name>] [--volume <0-100>] <file> [<file> ...]".into());
    }
    Ok(Some(args))
}

fn track(n: usize, path: PathBuf) -> Track {
    Track {
        id: TrackId(format!("file-{n}")),
        title: path
            .file_name()
            .map_or_else(String::new, |f| f.to_string_lossy().into_owned()),
        artists: Vec::new(),
        album: None,
        duration: None,
        origin: Origin::Local,
        video_id: None,
        local_path: Some(path),
        artwork: Artwork::None,
    }
}

fn run(args: Args) {
    let no_network = Arc::new(NoNetwork);
    let player = Player::spawn(PlayerDeps {
        resolver: no_network.clone(),
        opener: no_network,
        segments: None,
        sink_factory: Arc::new(CpalSinkFactory::new(args.device)),
    });
    let mut events = player.subscribe();
    player.send(PlayerCommand::SetVolume(args.volume));
    let tracks: Vec<Track> = args
        .files
        .into_iter()
        .enumerate()
        .map(|(n, p)| track(n, p))
        .collect();
    player.send(PlayerCommand::SetQueue { tracks, start: 0 });
    // Set once something played or failed, so a queue of bad files still ends the program.
    let mut started = false;
    loop {
        while let Some(event) = events.try_next() {
            match event {
                PlayerEvent::TrackChanged(id) => println!("Track: {}", id.0),
                PlayerEvent::SignalPathChanged(path) => {
                    println!("Signal path: {path:#?}");
                    println!("Source: {}", path.source.badge());
                }
                PlayerEvent::Failed { message } => {
                    started = true;
                    eprintln!("Failed: {message}");
                }
                PlayerEvent::StateChanged(state) => {
                    started |= state == PlayState::Playing;
                    println!("State: {state:?}");
                }
                _ => {}
            }
        }
        if started && player.snapshot().state == PlayState::Stopped {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    println!("Underruns: {}", player.snapshot().underruns);
}

fn main() -> ExitCode {
    match parse_args() {
        Ok(Some(args)) => {
            run(args);
            ExitCode::SUCCESS
        }
        Ok(None) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::from(2)
        }
    }
}
