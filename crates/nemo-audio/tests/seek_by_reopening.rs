//! A seek in a stream that cannot seek bytes but can be opened again at a time (an HLS stream
//! seeks by segment, soundcloud-full task 11): the decoder opens the stream again shortly before
//! the time, probes it and drops the audio before the time. The stream here is a 24-bit counter
//! WAV served in half-second units, so the frame number of the first sample after a seek can be
//! read back exactly.

mod common;

use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use common::fakes::{ThreadSinkFactory, video_track};
use common::fixtures::wav_bytes;
use nemo_audio::player::{Player, PlayerDeps};
use nemo_audio::stream_decoder::{RESTART_PRIME, StreamDecoder};
use nemo_core::BoxFuture;
use nemo_core::audio::{AudioFormat, Codec, Origin, StreamInfo};
use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent};
use nemo_core::secret::Secret;
use nemo_core::services::{ByteSource, PlayerControl, Reopen, StreamOpener};
use nemo_core::stream::{ResolvedStream, StreamError, StreamRequest, StreamResolver, Transport};
use nemo_core::track::VideoId;

const RATE: u32 = 8_000;
/// Frames in one unit (half a second).
const UNIT: usize = 4_000;
const UNITS: usize = 8;

/// The counter WAV from frame `from` on: left holds `frame + 1`, right the negative.
fn wav_from(from: usize) -> Vec<u8> {
    let mut samples = Vec::new();
    for frame in from..UNIT * UNITS {
        let v = i32::try_from(frame + 1).expect("small");
        samples.push(v);
        samples.push(-v);
    }
    wav_bytes(&samples, 24, RATE, 2)
}

/// A forward-only source that can be opened again at a unit.
struct Forward {
    bytes: Cursor<Vec<u8>>,
    reopen: Arc<Units>,
}

struct Units {
    opened: AtomicUsize,
    fail: bool,
}

impl Read for Forward {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.bytes.read(buf)
    }
}

impl Seek for Forward {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        match to {
            SeekFrom::Current(0) => self.bytes.stream_position(),
            _ => Err(io::Error::new(io::ErrorKind::Unsupported, "forward only")),
        }
    }
}

impl ByteSource for Forward {
    fn byte_len(&self) -> Option<u64> {
        None
    }

    fn is_seekable(&self) -> bool {
        false
    }

    fn duration_hint(&self) -> Option<Duration> {
        Some(Duration::from_secs(4))
    }

    fn reopener(&self) -> Option<Arc<dyn Reopen>> {
        Some(Arc::clone(&self.reopen) as Arc<dyn Reopen>)
    }
}

impl Reopen for Units {
    fn open_at(&self, to: Duration) -> io::Result<(Box<dyn ByteSource>, Duration)> {
        self.opened.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err(io::Error::other("no"));
        }
        let unit = Duration::from_millis(500);
        let index = usize::try_from(to.as_millis() / unit.as_millis())
            .expect("small")
            .min(UNITS - 1);
        let source = Forward {
            bytes: Cursor::new(wav_from(index * UNIT)),
            reopen: Arc::new(Units {
                opened: AtomicUsize::new(0),
                fail: false,
            }),
        };
        let start = unit * u32::try_from(index).expect("small");
        Ok((Box::new(source), start))
    }
}

fn forward(units: &Arc<Units>) -> Forward {
    Forward {
        bytes: Cursor::new(wav_from(0)),
        reopen: Arc::clone(units),
    }
}

fn units(fail: bool) -> Arc<Units> {
    Arc::new(Units {
        opened: AtomicUsize::new(0),
        fail,
    })
}

/// The frame number the first decoded sample carries.
fn first_frame(decoder: &mut StreamDecoder) -> usize {
    let mut block = Vec::new();
    assert!(decoder.next_block(&mut block).expect("decodes"));
    // 24-bit samples come out as `value / 2^23`.
    let value = (f64::from(block[0]) * 8_388_608.0).round();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let number = value as usize;
    number - 1
}

#[test]
fn a_seek_reopens_before_the_time_and_drops_the_lead_in() {
    let reopen = units(false);
    let mut decoder = StreamDecoder::open(Box::new(forward(&reopen)), None).expect("opens");
    assert_eq!(
        first_frame(&mut decoder),
        0,
        "a plain open starts at the start"
    );
    for (seconds, frame) in [
        (1.3_f64, 10_400_usize),
        (0.52, 4_160),
        (3.0, 24_000),
        (0.0, 0),
        (3.99, 31_920),
    ] {
        decoder
            .seek(Duration::from_secs_f64(seconds))
            .expect("seek");
        assert_eq!(first_frame(&mut decoder), frame, "{seconds} s");
    }
    assert_eq!(
        reopen.opened.load(Ordering::SeqCst),
        5,
        "one reopen per seek"
    );
}

#[test]
fn the_restart_starts_early_enough_for_the_decoder_to_settle() {
    // 1.0 s is on a unit boundary: the unit before it is opened, not the one at 1.0 s.
    let reopen = units(false);
    let mut decoder = StreamDecoder::open(Box::new(forward(&reopen)), None).expect("opens");
    decoder.seek(Duration::from_secs(1)).expect("seek");
    assert_eq!(first_frame(&mut decoder), 8_000);
    assert!(
        RESTART_PRIME > Duration::from_millis(20),
        "at least one AAC frame"
    );
}

#[test]
fn a_failed_reopen_is_an_error_and_the_decoder_stays_usable() {
    let reopen = units(true);
    let mut decoder = StreamDecoder::open(Box::new(forward(&reopen)), None).expect("opens");
    assert_eq!(first_frame(&mut decoder), 0);
    assert!(decoder.seek(Duration::from_secs(1)).is_err());
    let mut block = Vec::new();
    assert!(
        decoder.next_block(&mut block).expect("still decodes"),
        "the old stream goes on"
    );
}

#[test]
fn the_duration_hint_survives_a_seek() {
    let reopen = units(false);
    let mut decoder = StreamDecoder::open(Box::new(forward(&reopen)), None).expect("opens");
    decoder.seek(Duration::from_secs(2)).expect("seek");
    assert_eq!(decoder.info().duration, Some(Duration::from_secs(4)));
    assert_eq!(decoder.info().codec, Codec::Pcm);
}

/// Resolves every video to `mem://song` and the opener serves the forward-only source.
struct Resolver;

impl StreamResolver for Resolver {
    fn resolve<'a>(
        &'a self,
        _video: &'a VideoId,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        Box::pin(async {
            Ok(ResolvedStream {
                url: Secret::new("mem://song".to_owned()),
                info: StreamInfo {
                    origin: Origin::SoundCloud,
                    codec: Codec::Aac,
                    bitrate_kbps: Some(160),
                    format: None,
                },
                expires_at: None,
                transport: Transport::Hls { live: false },
                preview: false,
                audio_language: None,
                audio_tracks: Vec::new(),
            })
        })
    }
}

struct Opener(Arc<Units>);

impl StreamOpener for Opener {
    fn open(&self, _stream: &ResolvedStream) -> Result<Box<dyn ByteSource>, StreamError> {
        Ok(Box::new(forward(&self.0)))
    }
}

fn wait(what: &str, mut done: impl FnMut() -> bool) {
    let begin = Instant::now();
    while !done() {
        assert!(
            begin.elapsed() < Duration::from_secs(20),
            "timed out waiting for {what}"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn the_player_seeks_a_forward_only_stream_and_keeps_playing() {
    let reopen = units(false);
    let player: Arc<dyn PlayerControl> = Player::spawn(PlayerDeps {
        resolver: Arc::new(Resolver),
        request: StreamRequest::default(),
        local: None,
        offline: None,
        url_resolver: None,
        opener: Arc::new(Opener(Arc::clone(&reopen))),
        segments: None,
        sink_factory: Arc::new(ThreadSinkFactory {
            fixed: Some(AudioFormat {
                sample_rate: RATE,
                channels: 2,
                bit_depth: None,
            }),
            capture: Arc::default(),
            opened: Arc::default(),
            interval: Duration::from_micros(2500),
        }),
        device_enumerator: None,
        device_watch_interval: None,
    });
    let mut events = player.subscribe();
    player.send(PlayerCommand::Play(video_track(1)));
    wait("playing", || player.snapshot().state == PlayState::Playing);
    player.send(PlayerCommand::Pause);
    wait("the pause", || player.snapshot().state == PlayState::Paused);
    player.send(PlayerCommand::Seek(Duration::from_millis(2_300)));
    wait("the stream to be opened again", || {
        reopen.opened.load(Ordering::SeqCst) == 1
    });
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(
        reopen.opened.load(Ordering::SeqCst),
        1,
        "once, for one seek"
    );
    assert_eq!(player.position(), Duration::from_millis(2_300));
    assert_eq!(
        player.snapshot().state,
        PlayState::Paused,
        "the seek keeps the pause"
    );
    let failed =
        std::iter::from_fn(|| events.try_next()).any(|e| matches!(e, PlayerEvent::Failed { .. }));
    assert!(!failed, "a seek must not fail the track");
}
