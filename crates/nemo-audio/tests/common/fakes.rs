//! Fakes for the player tests: resolver, opener, segment source and a threaded test device.
//! No network, no audio device.

use std::collections::HashMap;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use nemo_audio::player::{Player, PlayerDeps};
use nemo_audio::sink::{AudioSink, RenderCallback, SinkError, SinkFactory};
use nemo_audio::source::MemorySource;
use nemo_core::BoxFuture;
use nemo_core::audio::{AudioFormat, Codec, Origin, OutputInfo, OutputMode, StreamInfo};
use nemo_core::library::LocalTrackLookup;
use nemo_core::player::{PlayerCommand, PlayerEvent};
use nemo_core::secret::Secret;
use nemo_core::segment::{Segment, SegmentError, SegmentSource};
use nemo_core::services::{ByteSource, EventReceiver, PlayerControl, StreamOpener};
use nemo_core::stream::{ResolvedStream, StreamError, StreamRequest, StreamResolver};
use nemo_core::track::{Artwork, Track, TrackId, VideoId};

/// Serves a video id from a map of in-memory files. Unknown ids are "not available".
pub struct FakeResolver {
    pub known: Vec<String>,
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

/// A player wired to fakes.
pub struct Rig {
    pub player: Arc<dyn PlayerControl>,
    pub capture: Arc<Mutex<Vec<f32>>>,
    pub opened: Arc<AtomicUsize>,
    pub events: Box<dyn EventReceiver>,
    pub seen: Vec<PlayerEvent>,
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
        }
    }
}

/// Start a player. `streams` are served as YouTube tracks by video id.
pub fn rig(streams: Vec<(String, Vec<u8>)>, options: RigOptions) -> Rig {
    let capture = Arc::new(Mutex::new(Vec::new()));
    let opened = Arc::new(AtomicUsize::new(0));
    let deps = PlayerDeps {
        resolver: Arc::new(FakeResolver {
            known: streams.iter().map(|(id, _)| id.clone()).collect(),
        }),
        request: StreamRequest::default(),
        local: options.local,
        opener: Arc::new(FakeOpener {
            files: streams.into_iter().collect(),
            chunk: options.chunk,
        }),
        segments: options.segments.map(|s| -> Arc<dyn SegmentSource> { s }),
        sink_factory: Arc::new(ThreadSinkFactory {
            fixed: options.fixed,
            capture: Arc::clone(&capture),
            opened: Arc::clone(&opened),
            interval: options.interval,
        }),
    };
    let player = Player::spawn(deps);
    let events = player.subscribe();
    Rig {
        player,
        capture,
        opened,
        events,
        seen: Vec::new(),
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
