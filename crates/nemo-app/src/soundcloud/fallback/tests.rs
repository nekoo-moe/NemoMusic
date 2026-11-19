//! Tests for the fallback chain: when `yt-dlp` is asked and when it never is.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use nemo_core::audio::{Codec, Origin, StreamInfo};
use nemo_core::secret::Secret;
use nemo_core::stream::Transport;

use super::*;

const HOSTS: &[&str] = &["soundcloud.com"];

fn page() -> MediaUrl {
    MediaUrl::parse("https://soundcloud.com/fixture-artist/track", HOSTS).expect("page")
}

fn policy() -> FormatPolicy {
    FormatPolicy {
        playable: vec![Codec::Aac],
        allow_hls: true,
    }
}

fn stream(label: &str) -> ResolvedStream {
    ResolvedStream {
        url: Secret::new(format!("https://example.invalid/{label}")),
        info: StreamInfo {
            origin: Origin::SoundCloud,
            codec: Codec::Aac,
            bitrate_kbps: Some(160),
            format: None,
        },
        expires_at: None,
        transport: Transport::File,
        preview: false,
        audio_language: None,
        audio_tracks: Vec::new(),
    }
}

/// Answers with a fixed result and counts its calls.
struct Fixed {
    answer: Result<ResolvedStream, StreamError>,
    resolves: AtomicUsize,
    log: Mutex<Vec<&'static str>>,
}

impl Fixed {
    fn new(answer: Result<ResolvedStream, StreamError>) -> Arc<Self> {
        Arc::new(Self {
            answer,
            resolves: AtomicUsize::new(0),
            log: Mutex::new(Vec::new()),
        })
    }
}

impl UrlResolver for Fixed {
    fn resolve<'a>(
        &'a self,
        _url: &'a MediaUrl,
        _policy: &'a FormatPolicy,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        self.resolves.fetch_add(1, Ordering::SeqCst);
        let answer = self.answer.clone();
        Box::pin(async move { answer })
    }

    fn list<'a>(
        &'a self,
        _query: &'a MediaQuery,
    ) -> BoxFuture<'a, Result<Vec<MediaEntry>, StreamError>> {
        Box::pin(async { Err(StreamError::NotAvailable) })
    }

    fn state(&self) -> ResolverState {
        ResolverState::Missing
    }

    fn warm(&self, _url: &MediaUrl) {
        self.log.lock().expect("log").push("warm");
    }

    fn stream_failed(&self, _url: &MediaUrl) {
        self.log.lock().expect("log").push("failed");
    }
}

fn run(native: &Arc<Fixed>, fallback: &Arc<Fixed>) -> Result<ResolvedStream, StreamError> {
    let chain = FallbackResolver::new(Arc::clone(native) as _, Arc::clone(fallback) as _);
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime");
    rt.block_on(chain.resolve(&page(), &policy()))
}

#[test]
fn a_native_answer_is_never_checked_with_yt_dlp() {
    let native = Fixed::new(Ok(stream("native")));
    let fallback = Fixed::new(Ok(stream("fallback")));
    let got = run(&native, &fallback).expect("stream");
    assert!(got.url.expose().ends_with("native"));
    assert_eq!(fallback.resolves.load(Ordering::SeqCst), 0);
}

#[test]
fn an_answer_the_native_client_cannot_read_goes_to_yt_dlp() {
    let native = Fixed::new(Err(StreamError::Parse("changed".into())));
    let fallback = Fixed::new(Ok(stream("fallback")));
    let got = run(&native, &fallback).expect("stream");
    assert!(got.url.expose().ends_with("fallback"));
    assert_eq!(fallback.resolves.load(Ordering::SeqCst), 1);
}

#[test]
fn refusals_and_failures_are_never_sent_to_yt_dlp() {
    for error in [
        StreamError::NotAvailable,
        StreamError::ResolverMissing,
        StreamError::Network("too many requests; try again later".into()),
    ] {
        let native = Fixed::new(Err(error.clone()));
        let fallback = Fixed::new(Ok(stream("fallback")));
        assert_eq!(run(&native, &fallback), Err(error));
        assert_eq!(
            fallback.resolves.load(Ordering::SeqCst),
            0,
            "an encrypted-only, blocked or limited track is not tried elsewhere"
        );
    }
}

#[test]
fn when_both_fail_the_native_error_is_reported() {
    let native = Fixed::new(Err(StreamError::Parse("native".into())));
    let fallback = Fixed::new(Err(StreamError::NotAvailable));
    assert_eq!(
        run(&native, &fallback),
        Err(StreamError::Parse("native".into()))
    );
}

#[test]
fn hints_go_to_the_native_resolver_and_a_failed_stream_to_both() {
    let native = Fixed::new(Ok(stream("native")));
    let fallback = Fixed::new(Ok(stream("fallback")));
    let chain = FallbackResolver::new(Arc::clone(&native) as _, Arc::clone(&fallback) as _);
    chain.warm(&page());
    chain.stream_failed(&page());
    assert_eq!(*native.log.lock().expect("log"), ["warm", "failed"]);
    assert_eq!(*fallback.log.lock().expect("log"), ["failed"]);
    assert!(matches!(chain.state(), ResolverState::Missing));
}
