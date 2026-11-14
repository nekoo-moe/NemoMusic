//! `SwitchAudioTrack`, `SetEffects` and `SetTransition` through the real control thread, with a
//! resolver that records requests and a test device. The track tests pause first, so the
//! position stands still.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use nemo_core::BoxFuture;
use nemo_core::audio::{AudioFormat, Codec, Origin, OutputInfo, OutputMode, StreamInfo};
use nemo_core::effects::{EffectsSettings, EffectsStatus, TransitionMode, TransitionSettings};
use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent};
use nemo_core::secret::Secret;
use nemo_core::services::{ByteSource, EventReceiver, PlayerControl, StreamOpener};
use nemo_core::stream::{ResolvedStream, StreamError, StreamRequest, StreamResolver, Transport};
use nemo_core::track::{Artwork, Track, TrackId, VideoId};

use crate::player::{Player, PlayerDeps};
use crate::sink::{AudioSink, RenderCallback, SinkError, SinkFactory};
use crate::source::MemorySource;

const RATE: u32 = 8_000;
const SECONDS: u32 = 10;

/// Records the audio language of every request it resolves.
#[derive(Default)]
struct Recorder {
    languages: Mutex<Vec<Option<String>>>,
}

impl Recorder {
    fn seen(&self) -> Vec<Option<String>> {
        self.languages.lock().expect("test lock").clone()
    }

    fn answer(
        &self,
        language: Option<String>,
    ) -> BoxFuture<'static, Result<ResolvedStream, StreamError>> {
        self.languages.lock().expect("test lock").push(language);
        Box::pin(async move {
            Ok(ResolvedStream {
                url: Secret::new("mem://song".to_owned()),
                info: StreamInfo {
                    origin: Origin::YoutubeMusic,
                    codec: Codec::Pcm,
                    bitrate_kbps: None,
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

impl StreamResolver for Recorder {
    fn resolve<'a>(
        &'a self,
        _video: &'a VideoId,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        self.answer(None)
    }

    fn resolve_with<'a>(
        &'a self,
        _video: &'a VideoId,
        request: &'a StreamRequest,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        self.answer(request.audio_language.clone())
    }
}

/// Serves the same WAV for every stream.
struct Opener(Vec<u8>);

impl StreamOpener for Opener {
    fn open(&self, _stream: &ResolvedStream) -> Result<Box<dyn ByteSource>, StreamError> {
        Ok(Box::new(MemorySource::new(self.0.clone())))
    }
}

/// Opens sinks in the source format whose thread calls the callback every 2 ms with 16
/// frames, about real time at 8 kHz, and throws the samples away.
struct Ticking;

struct TickingSink {
    info: OutputInfo,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl SinkFactory for Ticking {
    fn open(&self, source: AudioFormat) -> Result<Box<dyn AudioSink>, SinkError> {
        Ok(Box::new(TickingSink {
            info: OutputInfo {
                device_name: "Test device".into(),
                mode: OutputMode::Shared,
                format: source,
            },
            stop: Arc::new(AtomicBool::new(false)),
            thread: None,
        }))
    }
}

impl AudioSink for TickingSink {
    fn output_info(&self) -> &OutputInfo {
        &self.info
    }

    fn start(&mut self, mut callback: RenderCallback) -> Result<(), SinkError> {
        let stop = Arc::clone(&self.stop);
        let mut buf = vec![0.0_f32; 16 * usize::from(self.info.format.channels)];
        self.thread = Some(thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                callback(&mut buf);
                thread::sleep(Duration::from_millis(2));
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

/// A 16-bit mono WAV of silence.
fn wav() -> Vec<u8> {
    let data = RATE * SECONDS * 2;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2_u16.to_le_bytes());
    out.extend_from_slice(&16_u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    out.resize(out.len() + usize::try_from(data).expect("small fixture"), 0);
    out
}

fn video_track() -> Track {
    Track {
        id: TrackId("t1".into()),
        title: "Dubbed".into(),
        artists: vec!["Test".into()],
        album: None,
        duration: None,
        origin: Origin::YoutubeMusic,
        video_id: Some(VideoId::parse("TestTrack01").expect("valid id")),
        local_path: None,
        artwork: Artwork::None,
        source_url: None,
    }
}

fn player(resolver: &Arc<Recorder>) -> Arc<dyn PlayerControl> {
    let resolver: Arc<dyn StreamResolver> = Arc::<Recorder>::clone(resolver);
    Player::spawn(PlayerDeps {
        resolver,
        request: StreamRequest::default(),
        opener: Arc::new(Opener(wav())),
        segments: None,
        sink_factory: Arc::new(Ticking),
        local: None,
    })
}

fn wait(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timed out: {what}"
        );
        thread::sleep(Duration::from_millis(2));
    }
}

/// Play `track`, pause it and seek to `to` while paused, so the position stands still.
fn paused_at(p: &Arc<dyn PlayerControl>, track: Track, to: Duration) {
    p.send(PlayerCommand::Play(track));
    wait("playing", || p.snapshot().state == PlayState::Playing);
    p.send(PlayerCommand::Pause);
    wait("the pause", || p.snapshot().state == PlayState::Paused);
    p.send(PlayerCommand::Seek(to));
    wait("the seek", || p.position() == to);
}

fn track_changes(events: &mut dyn EventReceiver) -> usize {
    std::iter::from_fn(|| events.try_next())
        .filter(|e| matches!(e, PlayerEvent::TrackChanged(_)))
        .count()
}

#[test]
fn switching_the_audio_track_reopens_at_the_same_position() {
    let resolver = Arc::new(Recorder::default());
    let p = player(&resolver);
    paused_at(&p, video_track(), Duration::from_secs(3));
    let before = p.position();

    p.send(PlayerCommand::SwitchAudioTrack(Some("ja".into())));
    wait("the second resolve", || resolver.seen().len() == 2);
    assert_eq!(resolver.seen(), vec![None, Some("ja".to_owned())]);
    wait("the reopened track loads", || {
        p.snapshot().duration.is_some()
    });
    thread::sleep(Duration::from_millis(50));
    assert_eq!(p.position(), before, "reopened where it was");
    assert_eq!(p.snapshot().state, PlayState::Paused, "still paused");

    // `None` goes back to the original track.
    p.send(PlayerCommand::SwitchAudioTrack(None));
    wait("the third resolve", || resolver.seen().len() == 3);
    assert_eq!(resolver.seen()[2], None);
}

#[test]
fn a_local_track_ignores_the_switch() {
    let path = std::env::temp_dir().join(format!(
        "nemo-audio-switch-track-{}.wav",
        std::process::id()
    ));
    std::fs::write(&path, wav()).expect("test fixture write");
    let resolver = Arc::new(Recorder::default());
    let p = player(&resolver);
    let mut events = p.subscribe();
    let local = Track {
        origin: Origin::Local,
        video_id: None,
        local_path: Some(path.clone()),
        ..video_track()
    };
    paused_at(&p, local, Duration::from_secs(2));
    assert_eq!(track_changes(events.as_mut()), 1);

    p.send(PlayerCommand::SwitchAudioTrack(Some("ja".into())));
    // A command after the switch: once it is handled, the switch was handled too.
    p.send(PlayerCommand::SetVolume(50));
    wait("the volume", || p.snapshot().volume_percent == 50);
    assert_eq!(track_changes(events.as_mut()), 0, "nothing reopened");
    assert!(resolver.seen().is_empty(), "a local track resolves nothing");
    assert_eq!(p.position(), Duration::from_secs(2));
    drop(p);
    // The file is a test fixture in the temp folder; a leftover does no harm.
    let _ = std::fs::remove_file(&path);
}

#[test]
fn effects_and_transition_settings_are_stored_and_reported() {
    let p = player(&Arc::new(Recorder::default()));
    let mut events = p.subscribe();
    let mut effects = EffectsSettings::default();
    effects.equalizer.enabled = true;
    effects.equalizer.preamp_tenth_db = -30;
    effects.delay.enabled = true;

    p.send(PlayerCommand::SetEffects(effects));
    let mut changed: Vec<EffectsStatus> = Vec::new();
    wait("EffectsChanged", || {
        changed.extend(
            std::iter::from_fn(|| events.try_next()).filter_map(|e| match e {
                PlayerEvent::EffectsChanged(s) => Some(s),
                _ => None,
            }),
        );
        !changed.is_empty()
    });
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].settings, effects);
    assert_eq!(changed[0].clipped_samples, 0);
    assert_eq!(p.snapshot().effects, changed[0]);

    let transition = TransitionSettings {
        mode: TransitionMode::Crossfade,
        seconds: 4,
        ..TransitionSettings::default()
    };
    p.send(PlayerCommand::SetTransition(transition));
    wait("the transition", || p.snapshot().transition == transition);
    assert_eq!(p.snapshot().state, PlayState::Stopped, "nothing started");
}
