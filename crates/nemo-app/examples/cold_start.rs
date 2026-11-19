//! Cold start of the first song (cold-start spec): what a FRESH app process does between
//! "process start" and "first sample", with every stage timed. Development only, real network,
//! human-paced (about 8 requests per run), no sound device needed (a null sink counts the first
//! non-zero sample).
//!
//! ```text
//! XDG_DATA_HOME=<dir> cargo run -p nemo-app --example cold_start -- <id>[,<id>] [options]
//! ```
//!
//! Options: `--query "words"` runs one catalog search first, as the Home screen does before a
//! click; `--wait-secs N` waits N seconds between the start and the click (the splash and the
//! first screen; default 0); `--no-install` switches the yt-dlp auto install off; `--user-ytdlp
//! PATH` uses a program as the settings do; `--native-refuses` replaces the native request with
//! one that refuses every video after 100 ms (what a bot check looks like), so the yt-dlp
//! fallback is measured; `--no-warm-up` leaves out the start-up warm-up the app runs one second
//! after its services are up; `--no-stream-cache` leaves out the file of kept streams
//! (`cache/streams.json` in the data folder), so every run resolves; `--no-hint-last` leaves
//! out the start-up resolve of the song that played last (the example records it after each
//! click, as the app does). `NEMO_PROBE_IDS` replaces the positional ids. The
//! first id is the cold song; the others play after it, as warm comparisons.
//!
//! The data folder is `$XDG_DATA_HOME/NemoMusic` (the same as the app): an empty folder is the
//! first run (the auto install downloads yt-dlp), a folder with `bin/yt-dlp` is a later run.
//! It prints a line per stage, from the process start. The `nemo::start` lines come from the
//! app's own debug stages; only `nemo*` events are shown, so no URL and no id reach the output.
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "an example reports what it measured"
)]

use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use nemo_audio::decoder::supported_stream_codecs;
use nemo_audio::player::{Player, PlayerDeps};
use nemo_audio::sink::{AudioSink, RenderCallback, SinkError, SinkFactory};
use nemo_core::BoxFuture;
use nemo_core::account::AccountService;
use nemo_core::audio::{AudioFormat, Origin, OutputInfo, OutputMode};
use nemo_core::catalog::Catalog;
use nemo_core::player::{PlayState, PlayerCommand};
use nemo_core::services::PlayerControl;
use nemo_core::stream::{ResolvedStream, StreamError, StreamRequest, StreamResolver};
use nemo_core::track::{Artwork, Track, TrackId, VideoId};
use nemo_ytm::account::{AccountConfig, GoogleAccount, SessionSlot};
use nemo_ytm::catalog::YtmCatalog;
use nemo_ytm::resolver::{
    CachedResolver, FallbackResolver, HttpUrlCheck, StreamStore, persisted_chain,
};
use nemo_ytm::session::YtmSessionBackend;
use nemo_ytm::sponsorblock::{ReqwestHttp, SponsorBlockSource};
use nemo_ytm::stream::HttpStreamOpener;
use nemo_ytm::warmup::WarmUp;
use nemo_ytm::ytdlp::{YtDlpResolver, locate_yt_dlp, managed};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Metadata, Subscriber};

/// Longest wait for the first sample of one song.
const GIVE_UP: Duration = Duration::from_secs(12);

/// Time since the process (the example's `main`) started.
#[derive(Clone, Copy)]
struct Clock(Instant);

impl Clock {
    fn say(self, label: &str, since: Option<Instant>) {
        let took = since.map_or_else(String::new, |s| {
            format!("  ({} ms)", s.elapsed().as_millis())
        });
        println!("{:>7} ms  {label}{took}", self.0.elapsed().as_millis());
    }
}

/// Prints the `nemo*` tracing events with the time since the start. Nothing else is shown, so
/// no HTTP library line (which can carry a URL) reaches the output.
struct Stages(Clock);

struct Fields(String);

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let _ = write!(self.0, " {}={value:?}", field.name());
    }
}

impl Subscriber for Stages {
    fn enabled(&self, meta: &Metadata<'_>) -> bool {
        *meta.level() <= Level::DEBUG && meta.target().starts_with("nemo")
    }
    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, event: &Event<'_>) {
        let mut fields = Fields(String::new());
        event.record(&mut fields);
        let text = fields.0.replace(" message=\"start stage\"", "");
        self.0
            .say(&format!("    {}{text}", event.metadata().target()), None);
    }
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
}

/// A native request that always answers "not available" after 100 ms, like a bot check does.
struct Refuses;

impl StreamResolver for Refuses {
    fn resolve<'a>(
        &'a self,
        _video: &'a VideoId,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        Box::pin(async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            Err(StreamError::NotAvailable)
        })
    }
}

/// When the null sink saw its first non-zero sample.
type FirstSample = Arc<Mutex<Option<Instant>>>;

struct NullFactory(FirstSample);

struct NullSink {
    info: OutputInfo,
    first: FirstSample,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl SinkFactory for NullFactory {
    fn open(&self, source: AudioFormat) -> Result<Box<dyn AudioSink>, SinkError> {
        Ok(Box::new(NullSink {
            info: OutputInfo {
                device_name: "Null (cold_start example)".to_owned(),
                mode: OutputMode::Shared,
                format: source,
            },
            first: Arc::clone(&self.0),
            stop: Arc::default(),
            worker: None,
        }))
    }
}

impl AudioSink for NullSink {
    fn output_info(&self) -> &OutputInfo {
        &self.info
    }

    fn start(&mut self, mut callback: RenderCallback) -> Result<(), SinkError> {
        let frames = usize::try_from(self.info.format.sample_rate / 100).unwrap_or(480);
        let mut block = vec![0.0_f32; frames * usize::from(self.info.format.channels)];
        let (first, stop) = (Arc::clone(&self.first), Arc::clone(&self.stop));
        let worker = std::thread::Builder::new()
            .name("null-sink".into())
            .spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    callback(&mut block);
                    if block.iter().any(|s| *s != 0.0) {
                        let mut slot = first.lock().unwrap_or_else(PoisonError::into_inner);
                        slot.get_or_insert_with(Instant::now);
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            })
            .map_err(|e| SinkError::Backend(e.to_string()))?;
        self.worker = Some(worker);
        Ok(())
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join(); // The worker only ends its loop; a panic is not ours to report.
        }
    }
}

/// The options of this run.
#[allow(
    clippy::struct_excessive_bools,
    reason = "one switch per command line flag"
)]
struct Args {
    ids: Vec<VideoId>,
    query: Option<String>,
    wait: Duration,
    auto_install: bool,
    user_ytdlp: Option<PathBuf>,
    native_refuses: bool,
    warm_up: bool,
    stream_cache: bool,
    hint_last: bool,
}

fn parse_args() -> Args {
    let mut args = Args {
        ids: Vec::new(),
        query: None,
        wait: Duration::ZERO,
        auto_install: true,
        user_ytdlp: None,
        native_refuses: false,
        warm_up: true,
        stream_cache: true,
        hint_last: true,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--query" => args.query = it.next(),
            "--wait-secs" => {
                let secs = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                args.wait = Duration::from_secs(secs);
            }
            "--no-install" => args.auto_install = false,
            "--native-refuses" => args.native_refuses = true,
            "--no-warm-up" => args.warm_up = false,
            "--no-stream-cache" => args.stream_cache = false,
            "--no-hint-last" => args.hint_last = false,
            "--user-ytdlp" => args.user_ytdlp = it.next().map(PathBuf::from),
            list => args
                .ids
                .extend(list.split(',').filter_map(|s| VideoId::parse(s).ok())),
        }
    }
    if args.ids.is_empty() {
        let env = std::env::var("NEMO_PROBE_IDS").unwrap_or_default();
        args.ids = env
            .split(',')
            .filter_map(|s| VideoId::parse(s).ok())
            .collect();
    }
    args
}

fn track(video: &VideoId) -> Track {
    Track {
        id: TrackId(video.as_str().to_owned()),
        title: "probe".to_owned(),
        artists: Vec::new(),
        album: None,
        duration: None,
        origin: Origin::YoutubeMusic,
        video_id: Some(video.clone()),
        local_path: None,
        artwork: Artwork::None,
        source_url: None,
    }
}

/// `$XDG_DATA_HOME/NemoMusic` (or under `~/.local/share`), the app's folder on Linux.
fn data_root() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_default();
    base.join("NemoMusic")
}

/// The account service as `backend::build_account` builds it: store, vault load, session.
fn open_account(clock: Clock, handle: &tokio::runtime::Handle, slot: &SessionSlot) {
    let began = Instant::now();
    let store = match nemo_secrets::open_platform_store() {
        Ok(store) => store,
        Err(e) => {
            clock.say(&format!("credential store: unavailable ({e})"), Some(began));
            return;
        }
    };
    clock.say("credential store opened", Some(began));
    let began = Instant::now();
    let root = data_root();
    let config = AccountConfig {
        helper: PathBuf::new(),
        helper_args: Vec::new(),
        webview_dir: root.join("webview"),
        cache_file: root.join("account").join("account.json"),
    };
    let account = GoogleAccount::new(
        config,
        store,
        Arc::new(YtmSessionBackend::new(handle.clone())),
        slot.clone(),
        handle.clone(),
    );
    clock.say(
        &format!("account built (state: {:?})", account.state()),
        Some(began),
    );
}

/// Everything `backend::build` wires for the play path, with the null sink.
fn build_player(
    clock: Clock,
    handle: &tokio::runtime::Handle,
    args: &Args,
    first: &FirstSample,
) -> (
    Arc<dyn PlayerControl>,
    Option<WarmUp>,
    Option<Arc<StreamStore>>,
) {
    let began = Instant::now();
    open_account(clock, handle, &SessionSlot::default());
    let catalog_began = Instant::now();
    let _catalog = YtmCatalog::new(handle.clone());
    clock.say("catalog client built", Some(catalog_began));

    let began_locate = Instant::now();
    let program = args.user_ytdlp.clone().or_else(locate_yt_dlp);
    clock.say(
        &format!("yt-dlp located: {}", program.is_some()),
        Some(began_locate),
    );
    let ytdlp = program
        .clone()
        .map_or_else(YtDlpResolver::new, YtDlpResolver::with_program)
        .with_allowed_codecs(supported_stream_codecs())
        .with_handle(handle.clone());
    let warm_ytdlp = ytdlp.clone();
    managed::start_in_background(
        args.auto_install,
        args.user_ytdlp.is_some(),
        program,
        ytdlp.program_slot(),
    );
    let chain_began = Instant::now();
    let store = args.stream_cache.then(|| {
        Arc::new(StreamStore::open(
            data_root().join("cache").join("streams.json"),
        ))
    });
    let resolver = if args.native_refuses {
        let chain = FallbackResolver::new(Arc::new(Refuses), Arc::new(ytdlp));
        let cached = CachedResolver::new(Arc::new(chain), handle.clone());
        let cached = match (store.clone(), HttpUrlCheck::new()) {
            (Some(store), Ok(check)) => cached.with_persisted(store, Arc::new(check)),
            _ => cached,
        };
        Arc::new(cached) as Arc<dyn StreamResolver>
    } else {
        persisted_chain(ytdlp, supported_stream_codecs(), handle, store.clone())
    };
    clock.say("resolver chain built", Some(chain_began));
    // The app hints the song that played last as soon as the services are up.
    if let (true, Some(video)) = (
        args.hint_last,
        store.as_ref().and_then(|store| store.last_played()),
    ) {
        resolver.warm(&video);
        clock.say("start-up hint: the last played song", None);
    }

    let segments: Option<Arc<dyn nemo_core::segment::SegmentSource>> =
        ReqwestHttp::new(handle.clone())
            .ok()
            .map(|http| Arc::new(SponsorBlockSource::new(http)) as _);
    let warm = args.warm_up.then(|| {
        WarmUp::start(
            handle,
            warm_ytdlp,
            HttpStreamOpener::new(),
            Duration::from_secs(1),
        )
    });
    let player = Player::spawn(PlayerDeps {
        resolver,
        request: StreamRequest::default(),
        local: None,
        offline: None,
        opener: Arc::new(HttpStreamOpener::default()),
        segments,
        sink_factory: Arc::new(NullFactory(Arc::clone(first))),
        device_enumerator: None,
        device_watch_interval: None,
    });
    player.send(PlayerCommand::SetSkipSegments(true));
    clock.say("backend ready (the splash would end here)", Some(began));
    (player, warm, store)
}

/// Click on Play for `video` and wait for the first non-zero sample. Prints the time from the
/// click and from the start.
fn click(clock: Clock, player: &Arc<dyn PlayerControl>, first: &FirstSample, video: &VideoId) {
    *first.lock().unwrap_or_else(PoisonError::into_inner) = None;
    let clicked = Instant::now();
    clock.say("CLICK on Play", None);
    player.send(PlayerCommand::Play(track(video)));
    loop {
        let heard = *first.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(at) = heard {
            let ms = at.duration_since(clicked).as_millis();
            clock.say(&format!("FIRST SAMPLE: {ms} ms after the click"), None);
            return;
        }
        if clicked.elapsed() > GIVE_UP {
            let state = player.snapshot().state;
            clock.say(
                &format!("NO SAMPLE after {GIVE_UP:?} (state {state:?})"),
                None,
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn main() {
    let clock = Clock(Instant::now());
    let args = parse_args();
    if args.ids.is_empty() {
        eprintln!("give a video id (or NEMO_PROBE_IDS), see the file header");
        return;
    }
    // Another global subscriber would be a different setup; this example owns the process.
    let _ = tracing::subscriber::set_global_default(Stages(clock));
    println!(
        "cold_start: data folder exists={}, auto_install={}",
        data_root().exists(),
        args.auto_install
    );
    let began = Instant::now();
    let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("nemo-io")
        .enable_all()
        .build()
    else {
        return;
    };
    clock.say("runtime built", Some(began));
    let first = FirstSample::default();
    let (player, _warm, store) = build_player(clock, runtime.handle(), &args, &first);
    if let Some(query) = &args.query {
        let began = Instant::now();
        let found = runtime
            .block_on(YtmCatalog::new(runtime.handle().clone()).search(query))
            .map(|r| r.songs.len());
        clock.say(
            &format!("search (Home stand-in): {found:?} songs"),
            Some(began),
        );
    }
    if !args.wait.is_zero() {
        std::thread::sleep(args.wait);
    }
    for (n, video) in args.ids.iter().enumerate() {
        if n > 0 {
            std::thread::sleep(Duration::from_secs(5)); // human-paced
        }
        click(clock, &player, &first, video);
        if let Some(store) = &store {
            store.set_last_played(video);
        }
    }
    std::thread::sleep(Duration::from_secs(1));
    player.send(PlayerCommand::Stop);
    let playing = player.snapshot().state == PlayState::Playing;
    clock.say(&format!("done (still playing: {playing})"), None);
}
