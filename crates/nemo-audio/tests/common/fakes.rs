//! Fakes for the player tests: resolver, opener, segment source and a threaded test device.
//! No network, no audio device.

use std::collections::HashMap;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use nemo_audio::device_watch::DeviceEnumerator;
use nemo_audio::player::{Player, PlayerDeps};
use nemo_audio::replay_gain::ReplayGainSwitch;
use nemo_audio::sink::{AudioSink, OutputSelection, RenderCallback, SinkError, SinkFactory};
use nemo_audio::source::MemorySource;
use nemo_core::BoxFuture;
use nemo_core::audio::{AudioFormat, Codec, Origin, OutputInfo, OutputMode, StreamInfo};
use nemo_core::downloads::{CachedAudio, DownloadError, OfflineStore};
use nemo_core::library::LocalTrackLookup;
use nemo_core::output::{DeviceId, OutputDevice};
use nemo_core::player::{PlayerCommand, PlayerEvent};
use nemo_core::secret::Secret;
use nemo_core::segment::{Segment, SegmentError, SegmentSource};
use nemo_core::services::{ByteSource, EventReceiver, PlayerControl, StreamOpener};
use nemo_core::stream::{ResolvedStream, StreamError, StreamRequest, StreamResolver, Transport};
use nemo_core::track::{Artwork, Track, TrackId, VideoId};

/// Serves a video id from a map of in-memory files. Unknown ids are "not available".
pub struct FakeResolver {
    pub known: Vec<String>,
}

/// A [`FakeResolver`] that counts every resolve and warm hint, for the offline-cache tests.
pub struct CountingResolver {
    pub inner: FakeResolver,
    pub calls: Arc<AtomicUsize>,
}

impl StreamResolver for CountingResolver {
    fn resolve<'a>(
        &'a self,
        video: &'a VideoId,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.resolve(video)
    }

    fn warm(&self, _video: &VideoId) {
        self.calls.fetch_add(1, Ordering::SeqCst);
    }
}

/// An offline cache holding saved files by video id. It states Opus at 160 kbps for every
/// file, as the index would, so a test can tell the stated facts from the measured ones.
/// Records the songs the player marks unsaved.
pub struct FakeOffline {
    pub files: HashMap<String, PathBuf>,
    pub unsaved: Mutex<Vec<String>>,
}

impl FakeOffline {
    pub fn new(files: Vec<(String, PathBuf)>) -> Arc<Self> {
        Arc::new(Self {
            files: files.into_iter().collect(),
            unsaved: Mutex::new(Vec::new()),
        })
    }
}

impl OfflineStore for FakeOffline {
    fn open(&self, video: &VideoId) -> Option<CachedAudio> {
        self.files.get(video.as_str()).map(|path| CachedAudio {
            path: Secret::new(path.clone()),
            info: StreamInfo {
                origin: Origin::YoutubeMusic,
                codec: Codec::Opus,
                bitrate_kbps: Some(160),
                format: None,
            },
        })
    }

    fn segments(&self, _video: &VideoId) -> Option<Vec<Segment>> {
        None
    }

    fn mark_unsaved(&self, video: &VideoId) {
        self.unsaved
            .lock()
            .expect("unsaved lock")
            .push(video.as_str().to_owned());
    }

    fn record_play(&self, _video: &VideoId) {}

    fn wipe(&self) -> Result<u64, DownloadError> {
        Ok(0)
    }
}

impl StreamResolver for FakeResolver {
    fn resolve<'a>(
        &'a self,
        video: &'a VideoId,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        Box::pin(async move {
            if !self.known.iter().any(|k| k == video.as_str()) {
                return Err(StreamError::NotAvailable);
            }
            Ok(ResolvedStream {
                url: Secret::new(format!("mem://{}", video.as_str())),
                info: StreamInfo {
                    origin: Origin::YoutubeMusic,
                    codec: Codec::Pcm,
                    bitrate_kbps: Some(256),
                    format: None,
                },
                expires_at: None,
                transport: Transport::File,
                preview: false,
                audio_language: None,
                audio_tracks: Vec::new(),
            })
        })
    }
}

/// Opens `mem://<id>` URLs from a map. With `chunk` set, reads return at most that many bytes
/// and each costs a short busy wait, like a slow network.
pub struct FakeOpener {
    pub files: HashMap<String, Vec<u8>>,
    pub chunk: Option<usize>,
}

impl StreamOpener for FakeOpener {
    fn open(&self, stream: &ResolvedStream) -> Result<Box<dyn ByteSource>, StreamError> {
        let id = stream.url.expose().trim_start_matches("mem://");
        let bytes = self.files.get(id).ok_or(StreamError::NotAvailable)?.clone();
        Ok(match self.chunk {
            Some(chunk) => Box::new(SmallReads::new(bytes, chunk)),
            None => Box::new(MemorySource::new(bytes)),
        })
    }
}

/// A byte source that returns short reads and spends a little time on each, never blocking
/// on a timer (so Windows timer granularity does not matter).
pub struct SmallReads {
    inner: MemorySource,
    chunk: usize,
}

impl SmallReads {
    pub fn new(bytes: Vec<u8>, chunk: usize) -> Self {
        Self {
            inner: MemorySource::new(bytes),
            chunk,
        }
    }
}

impl Read for SmallReads {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let end = buf.len().min(self.chunk);
        let spin = Instant::now();
        while spin.elapsed() < Duration::from_micros(20) {
            std::hint::spin_loop();
        }
        self.inner.read(&mut buf[..end])
    }
}

impl Seek for SmallReads {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

impl ByteSource for SmallReads {
    fn byte_len(&self) -> Option<u64> {
        self.inner.byte_len()
    }
}

/// Returns fixed segments for every video.
pub struct FakeSegments {
    pub segments: Vec<Segment>,
    pub calls: AtomicUsize,
}

impl SegmentSource for FakeSegments {
    fn segments<'a>(
        &'a self,
        _video: &'a VideoId,
        _duration: Duration,
    ) -> BoxFuture<'a, Result<Vec<Segment>, SegmentError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.segments.clone())
        })
    }
}

/// A test output device. Its thread calls the render callback like a sound card would and
/// records what it gets, starting at the first non-zero sample. `interval` is the real time
/// between callbacks of `BLOCK` frames: shorter than the audio they hold means faster than
/// real time (a fast simulated clock).
pub struct ThreadSink {
    info: OutputInfo,
    capture: Arc<Mutex<Vec<f32>>>,
    interval: Duration,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// Frames per callback of the test device.
pub const BLOCK: usize = 480;

impl AudioSink for ThreadSink {
    fn output_info(&self) -> &OutputInfo {
        &self.info
    }

    fn start(&mut self, mut callback: RenderCallback) -> Result<(), SinkError> {
        let capture = Arc::clone(&self.capture);
        let stop = Arc::clone(&self.stop);
        let interval = self.interval;
        let samples = BLOCK * usize::from(self.info.format.channels);
        self.thread = Some(thread::spawn(move || {
            let mut buf = vec![0.0_f32; samples];
            let mut next = Instant::now();
            while !stop.load(Ordering::SeqCst) {
                callback(&mut buf);
                if let Ok(mut c) = capture.lock()
                    && (!c.is_empty() || buf.iter().any(|s| *s != 0.0))
                {
                    c.extend_from_slice(&buf);
                }
                next += interval;
                while Instant::now() < next && !stop.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_micros(200));
                }
            }
        }));
        Ok(())
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for ThreadSink {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Opens [`ThreadSink`]s that all record into one buffer.
pub struct ThreadSinkFactory {
    /// The device format, or `None` to follow the source.
    pub fixed: Option<AudioFormat>,
    pub capture: Arc<Mutex<Vec<f32>>>,
    pub opened: Arc<AtomicUsize>,
    pub interval: Duration,
}

impl SinkFactory for ThreadSinkFactory {
    fn open(&self, source: AudioFormat) -> Result<Box<dyn AudioSink>, SinkError> {
        self.opened.fetch_add(1, Ordering::SeqCst);
        let format = self.fixed.unwrap_or(AudioFormat {
            bit_depth: None,
            ..source
        });
        Ok(Box::new(ThreadSink {
            info: OutputInfo {
                device_name: "Test device".into(),
                mode: OutputMode::Shared,
                format,
            },
            capture: Arc::clone(&self.capture),
            interval: self.interval,
            stop: Arc::new(AtomicBool::new(false)),
            thread: None,
        }))
    }

    fn can_follow_source(&self) -> bool {
        self.fixed.is_none()
    }
}

/// Opens [`ThreadSink`]s for chosen devices and tracks selections.
pub struct SelectableSinkFactory {
    pub selection: Mutex<OutputSelection>,
    pub capture: Arc<Mutex<Vec<f32>>>,
    pub opened: Arc<AtomicUsize>,
    pub interval: Duration,
    pub devices: Mutex<Vec<OutputDevice>>,
    pub fail_id: Mutex<Option<DeviceId>>,
    pub missing_fallback: AtomicBool,
}

impl SelectableSinkFactory {
    pub fn new(
        devices: Vec<OutputDevice>,
        capture: Arc<Mutex<Vec<f32>>>,
        opened: Arc<AtomicUsize>,
        interval: Duration,
    ) -> Self {
        Self {
            selection: Mutex::new(OutputSelection::default()),
            capture,
            opened,
            interval,
            devices: Mutex::new(devices),
            fail_id: Mutex::new(None),
            missing_fallback: AtomicBool::new(false),
        }
    }
}

impl SinkFactory for SelectableSinkFactory {
    fn open(&self, source: AudioFormat) -> Result<Box<dyn AudioSink>, SinkError> {
        let sel = self.selection.lock().unwrap().clone();
        if let Some(ref fail) = *self.fail_id.lock().unwrap()
            && sel.device.as_ref() == Some(fail)
        {
            return Err(SinkError::Backend("mock device open failed".into()));
        }
        self.opened.fetch_add(1, Ordering::SeqCst);
        let devices = self.devices.lock().unwrap();
        let device_name = sel
            .device
            .as_ref()
            .and_then(|id| devices.iter().find(|d| &d.id == id).map(|d| d.name.clone()))
            .or_else(|| {
                devices
                    .iter()
                    .find(|d| d.is_default)
                    .map(|d| d.name.clone())
            })
            .unwrap_or_else(|| "Default Test Device".into());

        Ok(Box::new(ThreadSink {
            info: OutputInfo {
                device_name,
                mode: OutputMode::Shared,
                format: AudioFormat {
                    bit_depth: None,
                    ..source
                },
            },
            capture: Arc::clone(&self.capture),
            interval: self.interval,
            stop: Arc::new(AtomicBool::new(false)),
            thread: None,
        }))
    }

    fn select(&self, selection: OutputSelection) {
        if let Some(ref chosen) = selection.device {
            let devices = self.devices.lock().unwrap();
            let exists = devices.iter().any(|d| &d.id == chosen);
            self.missing_fallback.store(!exists, Ordering::SeqCst);
        } else {
            self.missing_fallback.store(false, Ordering::SeqCst);
        }
        *self.selection.lock().unwrap() = selection;
    }

    fn list_devices(&self) -> Result<Vec<OutputDevice>, SinkError> {
        Ok(self.devices.lock().unwrap().clone())
    }

    fn is_saved_missing(&self) -> bool {
        self.missing_fallback.load(Ordering::SeqCst)
    }
}

/// A player wired to fakes.
pub struct Rig {
    pub player: Arc<dyn PlayerControl>,
    pub capture: Arc<Mutex<Vec<f32>>>,
    pub opened: Arc<AtomicUsize>,
    pub events: Box<dyn EventReceiver>,
    pub seen: Vec<PlayerEvent>,
    /// Resolve calls and warm hints the player made.
    pub resolver_calls: Arc<AtomicUsize>,
}

/// Device and source settings for [`rig`].
pub struct RigOptions {
    pub fixed: Option<AudioFormat>,
    /// Real time per 480-frame callback.
    pub interval: Duration,
    /// Short reads of this many bytes for streamed files.
    pub chunk: Option<usize>,
    pub segments: Option<Arc<FakeSegments>>,
    /// Span and gain of local tracks (CUE tracks).
    pub local: Option<Arc<dyn LocalTrackLookup>>,
    /// The `ReplayGain` setting the test changes. `None` keeps it off.
    pub replay_gain: Option<ReplayGainSwitch>,
    /// Saved songs (offline cache).
    pub offline: Option<Arc<dyn OfflineStore>>,
    pub sink_factory: Option<Arc<dyn SinkFactory>>,
    pub capture: Option<Arc<Mutex<Vec<f32>>>>,
    pub opened: Option<Arc<AtomicUsize>>,
    pub device_enumerator: Option<Arc<dyn DeviceEnumerator>>,
    pub device_watch_interval: Option<Duration>,
}

impl Default for RigOptions {
    fn default() -> Self {
        Self {
            fixed: Some(AudioFormat {
                sample_rate: 48_000,
                channels: 2,
                bit_depth: None,
            }),
            interval: Duration::from_micros(2500),
            chunk: None,
            segments: None,
            local: None,
            replay_gain: None,
            offline: None,
            sink_factory: None,
            capture: None,
            opened: None,
            device_enumerator: None,
            device_watch_interval: None,
        }
    }
}

/// Start a player. `streams` are served as YouTube tracks by video id.
pub fn rig(streams: Vec<(String, Vec<u8>)>, options: RigOptions) -> Rig {
    let capture = options
        .capture
        .unwrap_or_else(|| Arc::new(Mutex::new(Vec::new())));
    let opened = options
        .opened
        .unwrap_or_else(|| Arc::new(AtomicUsize::new(0)));
    let resolver_calls = Arc::new(AtomicUsize::new(0));
    let sink_factory = options.sink_factory.unwrap_or_else(|| {
        Arc::new(ThreadSinkFactory {
            fixed: options.fixed,
            capture: Arc::clone(&capture),
            opened: Arc::clone(&opened),
            interval: options.interval,
        })
    });
    let deps = PlayerDeps {
        resolver: Arc::new(CountingResolver {
            inner: FakeResolver {
                known: streams.iter().map(|(id, _)| id.clone()).collect(),
            },
            calls: Arc::clone(&resolver_calls),
        }),
        request: StreamRequest::default(),
        local: options.local,
        offline: options.offline,
        opener: Arc::new(FakeOpener {
            files: streams.into_iter().collect(),
            chunk: options.chunk,
        }),
        segments: options.segments.map(|s| -> Arc<dyn SegmentSource> { s }),
        sink_factory,
        device_enumerator: options.device_enumerator,
        device_watch_interval: options.device_watch_interval,
    };
    let player = Player::spawn_with_replay_gain(deps, options.replay_gain.unwrap_or_default());
    let events = player.subscribe();
    Rig {
        player,
        capture,
        opened,
        events,
        seen: Vec::new(),
        resolver_calls,
    }
}

impl Rig {
    pub fn send(&self, command: PlayerCommand) {
        self.player.send(command);
    }

    /// Frames recorded so far (stereo assumed by the caller through `channels`).
    pub fn captured(&self) -> Vec<f32> {
        self.capture.lock().expect("capture lock").clone()
    }

    /// Move new events into `seen` and return everything seen so far.
    pub fn drain(&mut self) -> &[PlayerEvent] {
        while let Some(e) = self.events.try_next() {
            self.seen.push(e);
        }
        &self.seen
    }

    /// Wait until `done` holds, polling every few ms. Panics after `limit`.
    pub fn wait(&mut self, what: &str, limit: Duration, mut done: impl FnMut(&mut Rig) -> bool) {
        let start = Instant::now();
        while !done(self) {
            assert!(start.elapsed() < limit, "timed out waiting for {what}");
            thread::sleep(Duration::from_millis(2));
        }
    }

    /// Wait until the capture holds at least `samples` samples.
    pub fn wait_samples(&mut self, samples: usize, limit: Duration) {
        self.wait("captured audio", limit, |r| {
            r.capture.lock().expect("capture lock").len() >= samples
        });
    }

    /// Wait until an event matching `pred` has been seen.
    pub fn wait_event(&mut self, what: &str, limit: Duration, pred: impl Fn(&PlayerEvent) -> bool) {
        self.wait(what, limit, |r| r.drain().iter().any(&pred));
    }
}

/// A YouTube-style track with a synthetic id (`TestTrack01`).
pub fn video_track(n: u32) -> Track {
    Track {
        id: TrackId(format!("t{n}")),
        title: format!("Track {n}"),
        artists: vec!["Test".into()],
        album: None,
        duration: None,
        origin: Origin::YoutubeMusic,
        video_id: Some(VideoId::parse(&video_id(n)).expect("invariant: 11 chars")),
        local_path: None,
        artwork: Artwork::None,
        source_url: None,
    }
}

/// The synthetic video id for track `n`.
pub fn video_id(n: u32) -> String {
    format!("TestTrack{n:02}")
}

/// A local-file track.
pub fn local_track(n: u32, path: std::path::PathBuf) -> Track {
    Track {
        origin: Origin::Local,
        video_id: None,
        local_path: Some(path),
        ..video_track(n)
    }
}
