//! Tests for building the SoundCloud services: a closed gate builds nothing and contacts
//! nothing, an open one starts the client id look-up, and the router sends each stream to its
//! reader.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nemo_core::BoxFuture;
use nemo_core::audio::{Codec, Origin, StreamInfo};
use nemo_core::secret::Secret;
use nemo_core::services::ByteSource;
use nemo_core::stream::{ResolvedStream, StreamError, Transport};
use nemo_core::url_resolver::UrlResolver;
use nemo_soundcloud::http::{ScAnswer, ScHttpError, ScRequest};

use super::*;

/// Counts every request and answers 404 to all of them.
#[derive(Default)]
struct CountingHost {
    urls: Mutex<Vec<String>>,
}

impl ScHttp for CountingHost {
    fn send(&self, request: ScRequest) -> BoxFuture<'_, Result<ScAnswer, ScHttpError>> {
        self.urls.lock().expect("lock").push(request.url);
        Box::pin(async {
            Ok(ScAnswer {
                status: 404,
                body: Vec::new(),
                retry_after: None,
            })
        })
    }
}

fn choice(enabled: bool, confirmed_version: u32) -> BTreeMap<String, OptInChoice> {
    BTreeMap::from([(
        "soundcloud".to_owned(),
        OptInChoice {
            enabled,
            confirmed_version,
        },
    )])
}

/// Build with `choices` and report whether the HTTP layer was asked for, and what it was sent.
async fn run(choices: &BTreeMap<String, OptInChoice>) -> (bool, Vec<String>) {
    let host = Arc::new(CountingHost::default());
    let asked = Arc::new(AtomicUsize::new(0));
    let (counter, shared) = (Arc::clone(&asked), Arc::clone(&host));
    let parts = build(
        choices,
        &Handle::current(),
        move || {
            counter.fetch_add(1, Ordering::SeqCst);
            Some(shared as Arc<dyn ScHttp>)
        },
        None,
    );
    let began = Instant::now();
    // A started client looks up the client id in the background; give it time to ask.
    while parts.is_some()
        && host.urls.lock().expect("lock").is_empty()
        && began.elapsed() < Duration::from_secs(5)
    {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
    let urls = host.urls.lock().expect("lock").clone();
    assert_eq!(parts.is_some(), asked.load(Ordering::SeqCst) == 1);
    (parts.is_some(), urls)
}

#[tokio::test]
async fn nothing_is_built_and_nothing_requested_while_the_switch_is_off_or_unconfirmed() {
    let version = OPT_IN.version;
    let closed = [
        BTreeMap::new(),
        choice(false, 0),
        choice(true, 0),
        choice(false, version),
        choice(true, version - 1),
        choice(true, version + 1),
    ];
    for choices in &closed {
        let (built, urls) = run(choices).await;
        assert!(!built && urls.is_empty(), "{choices:?}");
    }
}

#[tokio::test]
async fn an_open_gate_builds_the_services_and_asks_for_the_home_page_first() {
    let (built, urls) = run(&choice(true, OPT_IN.version)).await;
    assert!(built);
    assert_eq!(
        urls,
        ["https://soundcloud.com/"],
        "only the client id look-up, on a SoundCloud host"
    );
}

#[tokio::test]
async fn no_http_layer_means_no_services() {
    let parts = build(
        &choice(true, OPT_IN.version),
        &Handle::current(),
        || None,
        None,
    );
    assert!(parts.is_none());
}

/// An opener that says which reader it is.
struct Named(&'static str);

impl StreamOpener for Named {
    fn open(&self, _stream: &ResolvedStream) -> Result<Box<dyn ByteSource>, StreamError> {
        Err(StreamError::Network(self.0.to_owned()))
    }
}

fn stream(origin: Origin, transport: Transport) -> ResolvedStream {
    ResolvedStream {
        url: Secret::new("https://example.invalid/a".to_owned()),
        info: StreamInfo {
            origin,
            codec: Codec::Aac,
            bitrate_kbps: None,
            format: None,
        },
        expires_at: None,
        transport,
        preview: false,
        audio_language: None,
        audio_tracks: Vec::new(),
    }
}

fn reader(router: &RoutingOpener, origin: Origin, transport: Transport) -> String {
    match router.open(&stream(origin, transport)) {
        Err(StreamError::Network(name)) => name,
        _ => String::from("unexpected"),
    }
}

#[test]
fn each_stream_goes_to_its_reader() {
    let router = RoutingOpener::new(Arc::new(Named("files")), Arc::new(Named("soundcloud")));
    let hls = Transport::Hls { live: false };
    assert_eq!(reader(&router, Origin::SoundCloud, hls), "soundcloud");
    assert_eq!(
        reader(&router, Origin::SoundCloud, Transport::File),
        "files",
        "progressive MP3"
    );
    assert_eq!(
        reader(&router, Origin::YoutubeMusic, Transport::File),
        "files"
    );
    assert_eq!(
        reader(&router, Origin::YoutubeMusic, hls),
        "files",
        "not SoundCloud's to read"
    );
    assert_eq!(reader(&router, Origin::Local, Transport::File), "files");
}

#[test]
fn the_switch_is_in_the_gate_list_so_its_hosts_are_checked_against_the_readme() {
    assert!(
        crate::opt_in_gate::OPT_INS
            .iter()
            .any(|info| info.id == "soundcloud")
    );
}

#[tokio::test]
async fn with_yt_dlp_behind_it_the_resolver_is_still_the_native_one_and_the_gate_still_holds() {
    let ytdlp: Arc<dyn UrlResolver> = Arc::new(nemo_ytm::url_resolver::YtDlpUrlResolver::new(
        nemo_ytm::ytdlp::YtDlpResolver::with_program("nemo-no-such-yt-dlp-xyz"),
    ));
    let host = Arc::new(CountingHost::default());
    let closed = build(
        &choice(true, 0),
        &Handle::current(),
        || Some(Arc::clone(&host) as Arc<dyn ScHttp>),
        Some(Arc::clone(&ytdlp)),
    );
    assert!(
        closed.is_none(),
        "an unconfirmed switch builds nothing, fallback or not"
    );
    let open = build(
        &choice(true, OPT_IN.version),
        &Handle::current(),
        || Some(Arc::clone(&host) as Arc<dyn ScHttp>),
        Some(ytdlp),
    )
    .expect("built");
    assert!(matches!(
        open.resolver.state(),
        nemo_core::url_resolver::ResolverState::Ready { version } if version == "native"
    ));
}

/// Parts built with a counting host, so a test can see whether the account asks for anything.
fn open_parts() -> (SoundcloudParts, Arc<CountingHost>) {
    let host = Arc::new(CountingHost::default());
    let shared = Arc::clone(&host);
    let parts = build(
        &choice(true, OPT_IN.version),
        &Handle::current(),
        move || Some(shared as Arc<dyn ScHttp>),
        None,
    )
    .expect("open gate");
    (parts, host)
}

#[tokio::test]
async fn the_account_is_built_with_a_credential_store_and_starts_signed_out_without_a_request() {
    use nemo_core::secret_store::MemoryStore;
    use nemo_core::source_account::SourceAccountState;

    let (parts, host) = open_parts();
    let root = std::env::temp_dir().join(format!("nemo-app-sc-account-{}", std::process::id()));
    let paths = AppPaths::under(&root);
    let store: Arc<dyn SecretStore> = Arc::new(MemoryStore::default());
    let account = build_account(&parts, Some(store), &paths, &Handle::current()).expect("account");
    assert_eq!(account.state(), SourceAccountState::SignedOut);
    // Only the client id look-up from `build` may have asked; the account asked for nothing.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let urls = host.urls.lock().expect("lock").clone();
    assert!(
        urls.iter().all(|u| u == "https://soundcloud.com/"),
        "{urls:?}"
    );
    assert!(!root.exists(), "nothing was written before a sign-in");
}

#[tokio::test]
async fn without_a_credential_store_there_is_no_account_and_so_no_place_for_a_token() {
    let (parts, _) = open_parts();
    let paths = AppPaths::under(&std::env::temp_dir().join("nemo-app-sc-nostore"));
    assert!(build_account(&parts, None, &paths, &Handle::current()).is_none());
}

#[test]
fn the_two_sign_in_windows_use_their_own_profile_folders() {
    let paths = AppPaths::under(std::path::Path::new("C:/data/NemoMusic"));
    assert_ne!(paths.webview, paths.webview_soundcloud);
}

#[tokio::test]
async fn switching_off_in_settings_stops_the_client_at_once_and_on_again_restores_it() {
    use nemo_soundcloud::ClientError;

    let (parts, host) = open_parts();
    let slot = ClientSlot::default();
    slot.apply(false); // Nothing is built yet: nothing happens.
    slot.set(Arc::clone(&parts.client));
    tokio::time::sleep(Duration::from_millis(100)).await; // The client id look-up of `build`.
    slot.apply(false);
    let before = host.urls.lock().expect("lock").len();
    assert_eq!(
        parts.client.get("/search", &[]).await,
        Err(ClientError::Off)
    );
    assert_eq!(
        host.urls.lock().expect("lock").len(),
        before,
        "nothing was sent"
    );
    slot.apply(true);
    assert!(parts.client.is_enabled());
}

#[test]
fn a_closed_gate_has_no_account_and_leaves_the_slot_empty() {
    let paths = AppPaths::under(&std::env::temp_dir().join("nemo-app-sc-closed"));
    let slot = ClientSlot::default();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    assert!(account_for(None, None, &paths, rt.handle(), &slot).is_none());
    slot.apply(false); // Nothing to switch.
}
