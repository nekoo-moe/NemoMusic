//! The in-process tier with fakes: the answer it builds, its place between the native request
//! and `yt-dlp`, and what a refused address does to the player table.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, PoisonError};
use std::time::Duration;

use nemo_core::BoxFuture;
use nemo_core::audio::{Codec, Origin, StreamInfo};
use nemo_core::secret::Secret;
use nemo_core::stream::{ResolvedStream, StreamError, StreamResolver, Transport};
use nemo_core::track::VideoId;

use super::fakes::{ANSWER, FakeRequest, FakeSolver, FakeSource};
use crate::cipher::{CipherResolver, PlayerStore};
use crate::resolver::FallbackResolver;

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_time()
        .build()
        .expect("runtime")
}

fn vid() -> VideoId {
    VideoId::parse("abcdefghijk").expect("valid")
}

struct Parts {
    resolver: Arc<CipherResolver>,
    source: Arc<FakeSource>,
    solver: Arc<FakeSolver>,
    request: Arc<FakeRequest>,
}

fn parts(rt: &tokio::runtime::Runtime, answer: &str) -> Parts {
    let source = FakeSource::new("aaaaaaaa", Duration::ZERO);
    let solver = Arc::new(FakeSolver::default());
    let request = FakeRequest::new(answer);
    let store = PlayerStore::new(source.clone(), solver.clone(), rt.handle().clone(), None);
    let resolver = Arc::new(CipherResolver::with_parts(
        store,
        solver.clone(),
        request.clone(),
        &[Codec::Opus, Codec::Aac],
    ));
    Parts {
        resolver,
        source,
        solver,
        request,
    }
}

#[test]
fn the_best_format_is_solved_and_put_together() {
    let rt = rt();
    let p = parts(&rt, ANSWER);
    let stream = rt.block_on(p.resolver.resolve(&vid())).expect("resolved");
    assert_eq!(
        stream.url.expose(),
        "https://r1.example.invalid/videoplayback?expire=1900000000&n=Nabc&itag=251&sig=SENC"
    );
    assert_eq!(
        stream.info.codec,
        Codec::Opus,
        "the protected 999 kbps format is skipped"
    );
    assert_eq!(stream.info.bitrate_kbps, Some(130));
    assert!(stream.expires_at.is_some());
    assert_eq!(
        *p.request.sts.lock().unwrap_or_else(PoisonError::into_inner),
        Some(20522)
    );
    assert_eq!(
        p.solver.solves.load(Ordering::SeqCst),
        1,
        "n and signature in one job"
    );
}

#[test]
fn a_plain_address_with_an_n_value_gets_only_the_n_solved() {
    let rt = rt();
    let plain = r#"{"playabilityStatus":{"status":"OK"},"streamingData":{"adaptiveFormats":[
      {"mimeType":"audio/mp4; codecs=\"mp4a.40.2\"","bitrate":128000,
       "url":"https://r1.example.invalid/videoplayback?expire=1900000000&n=zzz&itag=140"}]}}"#;
    let p = parts(&rt, plain);
    let stream = rt.block_on(p.resolver.resolve(&vid())).expect("resolved");
    assert_eq!(
        stream.url.expose(),
        "https://r1.example.invalid/videoplayback?expire=1900000000&n=Nzzz&itag=140"
    );
}

#[test]
fn a_video_that_is_not_playable_is_not_available() {
    let rt = rt();
    let p = parts(&rt, r#"{"playabilityStatus":{"status":"LOGIN_REQUIRED"}}"#);
    assert_eq!(
        rt.block_on(p.resolver.resolve(&vid()))
            .expect_err("refused"),
        StreamError::NotAvailable
    );
}

/// A resolver that answers a fixed way and counts its calls.
struct Fixed {
    calls: AtomicUsize,
    ok: bool,
}

impl StreamResolver for Fixed {
    fn resolve<'a>(
        &'a self,
        _video: &'a VideoId,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let ok = self.ok;
        Box::pin(async move {
            if !ok {
                return Err(StreamError::NotAvailable);
            }
            Ok(ResolvedStream {
                url: Secret::new("https://yt-dlp.example.invalid/x".to_owned()),
                info: StreamInfo {
                    origin: Origin::YoutubeMusic,
                    codec: Codec::Aac,
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

fn fixed(ok: bool) -> Arc<Fixed> {
    Arc::new(Fixed {
        calls: AtomicUsize::new(0),
        ok,
    })
}

/// The chain the app builds: native, then this tier, then yt-dlp.
fn chain(native: &Arc<Fixed>, cipher: Arc<CipherResolver>, ytdlp: &Arc<Fixed>) -> FallbackResolver {
    FallbackResolver::new(
        native.clone(),
        Arc::new(FallbackResolver::new(cipher, ytdlp.clone())),
    )
}

#[test]
fn a_song_the_native_request_refuses_is_solved_in_process_before_yt_dlp_is_asked() {
    let rt = rt();
    let p = parts(&rt, ANSWER);
    let (native, ytdlp) = (fixed(false), fixed(true));
    let chain = chain(&native, p.resolver.clone(), &ytdlp);
    let stream = rt.block_on(chain.resolve(&vid())).expect("resolved");
    assert!(stream.url.expose().contains("sig=SENC"));
    assert_eq!(native.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        ytdlp.calls.load(Ordering::SeqCst),
        0,
        "yt-dlp was not started"
    );
}

#[test]
fn when_the_tier_fails_yt_dlp_answers() {
    let rt = rt();
    let p = parts(&rt, ANSWER);
    p.solver.fail.store(true, Ordering::SeqCst);
    let (native, ytdlp) = (fixed(false), fixed(true));
    let chain = chain(&native, p.resolver.clone(), &ytdlp);
    let stream = rt
        .block_on(chain.resolve(&vid()))
        .expect("resolved by yt-dlp");
    assert!(stream.url.expose().contains("yt-dlp.example"));
    assert_eq!(ytdlp.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn a_refused_address_refreshes_the_player_table() {
    let rt = rt();
    let p = parts(&rt, ANSWER);
    assert!(rt.block_on(p.resolver.resolve(&vid())).is_ok());
    assert_eq!(p.source.ids.load(Ordering::SeqCst), 1);
    // The CDN answers 403 for the address: the opener reports it.
    p.resolver.stream_failed(&vid());
    p.source.set_id("bbbbbbbb");
    assert!(rt.block_on(p.resolver.resolve(&vid())).is_ok());
    assert_eq!(
        p.source.ids.load(Ordering::SeqCst),
        2,
        "the player id was read again"
    );
    assert_eq!(
        p.source.scripts.load(Ordering::SeqCst),
        2,
        "and the new player was downloaded"
    );
    assert_eq!(p.source.id_asked_for_script().as_deref(), Some("bbbbbbbb"));
}

#[test]
fn a_solver_that_cannot_find_its_functions_rebuilds_the_player() {
    let rt = rt();
    let p = parts(&rt, ANSWER);
    assert!(rt.block_on(p.resolver.resolve(&vid())).is_ok());
    p.solver.fail.store(true, Ordering::SeqCst);
    assert!(rt.block_on(p.resolver.resolve(&vid())).is_err());
    p.solver.fail.store(false, Ordering::SeqCst);
    p.source.set_id("bbbbbbbb");
    assert!(rt.block_on(p.resolver.resolve(&vid())).is_ok());
    assert_eq!(p.source.scripts.load(Ordering::SeqCst), 2);
}

#[test]
fn nothing_the_tier_logs_names_an_address_an_id_or_a_script() {
    let rt = rt();
    let p = parts(&rt, ANSWER);
    let lines = crate::timing::capture::lines_of(|| {
        let _ = rt.block_on(p.resolver.resolve(&vid()));
        p.resolver.stream_failed(&vid());
    });
    assert!(!lines.is_empty(), "the stages were logged");
    for line in &lines {
        for secret in [
            "example.invalid",
            "abcdefghijk",
            "ENC",
            "pre-aaaaaaaa",
            "20522",
        ] {
            assert!(!line.contains(secret), "{line}");
        }
    }
}
