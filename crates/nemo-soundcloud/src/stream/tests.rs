//! Tests for the stream lookup over a fake HTTP layer and the recorded, rewritten fixtures.

use std::sync::Arc;

use nemo_core::catalog::{Catalog, SearchKind};
use nemo_core::track::MediaUrl;

use super::*;
use crate::catalog::SoundcloudCatalog;
use crate::client::{Endpoints, Pacing};
use crate::fake::{FakeHttp, ok, status};
use crate::fixtures::{SEARCH_TRACKS, TRACK_HLS};
use crate::hls::Rules;
use crate::http::ScHttp;

const ID: &str = "AbCdEfGh0123456789AbCdEfGh012345";
const HLS_ADDRESS: &str =
    "https://playback.media-streaming.soundcloud.cloud/x/aac_160k/u/playlist.m3u8";

fn pacing() -> Pacing {
    Pacing {
        min_gap: Duration::ZERO,
        retry_delays: [Duration::ZERO, Duration::ZERO],
        default_backoff: Duration::from_millis(20),
        max_backoff: Duration::from_millis(20),
    }
}

fn policy() -> FormatPolicy {
    FormatPolicy {
        playable: vec![Codec::Aac, Codec::Mp3],
        allow_hls: true,
    }
}

/// A resolver and catalog over a fake service. `media` answers the stream lookups.
fn setup<F>(media: F) -> (SoundcloudCatalog, ScStreamResolver, Arc<FakeHttp>)
where
    F: Fn(&str) -> String + Send + Sync + 'static,
{
    let fake = Arc::new(FakeHttp::new(move |url, _| {
        if url == "https://soundcloud.com/" {
            ok(r#"<script src="https://a-v2.sndcdn.com/assets/1-a.js"></script>"#)
        } else if url.starts_with("https://a-v2.sndcdn.com/") {
            ok(format!("client_id:\"{ID}\""))
        } else if url.contains("/media/") {
            let answer = media(url);
            if answer.is_empty() {
                status(404, "{}")
            } else {
                ok(answer)
            }
        } else if url.contains("/resolve?") {
            ok(TRACK_HLS)
        } else {
            ok(SEARCH_TRACKS)
        }
    }));
    let client = Arc::new(ScClient::with(
        Arc::clone(&fake) as Arc<dyn ScHttp>,
        Endpoints::production(),
        pacing(),
    ));
    let shared = Arc::new(Shared::with(Rules::loopback()));
    let resolver = ScStreamResolver::new(Arc::clone(&client), shared, Handle::current());
    (SoundcloudCatalog::new(client), resolver, fake)
}

fn answer(url: &str) -> String {
    format!("{{\"url\":\"{url}\"}}")
}

async fn songs(catalog: &SoundcloudCatalog) -> Vec<nemo_core::track::Track> {
    catalog
        .search_kind("a", SearchKind::Songs)
        .await
        .expect("songs")
        .songs
}

fn lookups(fake: &FakeHttp) -> Vec<String> {
    fake.urls()
        .into_iter()
        .filter(|u| u.contains("/media/"))
        .collect()
}

#[test]
fn presets_and_addresses_state_codec_and_bitrate() {
    assert_eq!(preset("aac_160k"), Some((Codec::Aac, Some(160))));
    assert_eq!(preset("aac_96k"), Some((Codec::Aac, Some(96))));
    assert_eq!(preset("mp3_0_0"), Some((Codec::Mp3, None)));
    assert_eq!(preset("opus_0_0"), Some((Codec::Opus, None)));
    assert_eq!(
        preset("abr_sq"),
        None,
        "the adaptive list is not a candidate"
    );
    assert_eq!(preset("flac"), None);
    assert_eq!(
        kbps_in_address("https://cf-preview-media.sndcdn.com/preview/0/30/ab.128.mp3?x=1"),
        Some(128)
    );
    assert_eq!(kbps_in_address("https://h.example/a.64.opus"), Some(64));
    assert_eq!(kbps_in_address("https://h.example/a.mp3"), None);
    assert_eq!(kbps_in_address("https://h.example/a.99999.mp3"), None);
    assert_eq!(kbps_in_address("https://h.example/a.128.m4a"), None);
}

#[tokio::test]
async fn the_best_aac_stream_is_looked_up_with_the_authorization_and_cached() {
    let (catalog, resolver, fake) = setup(|_| answer(HLS_ADDRESS));
    let track = songs(&catalog).await.remove(0);
    let page = track.source_url.expect("page");
    let stream = resolver.resolve(&page, &policy()).await.expect("stream");
    assert_eq!(stream.url.expose(), HLS_ADDRESS);
    assert_eq!(stream.transport, Transport::Hls { live: false });
    assert_eq!(stream.info.origin, Origin::SoundCloud);
    assert_eq!(
        (stream.info.codec, stream.info.bitrate_kbps),
        (Codec::Aac, Some(160))
    );
    assert!(!stream.preview);
    assert!(stream.expires_at.is_some());
    let lookup = lookups(&fake);
    assert_eq!(lookup.len(), 1);
    assert!(
        lookup[0].contains("000001000000/stream/hls"),
        "aac_160k, not the 96k one"
    );
    assert!(lookup[0].contains("track_authorization=fixture-authorization"));
    assert!(lookup[0].ends_with(ID), "client id last");
    // Cached: no new request.
    let again = resolver.resolve(&page, &policy()).await.expect("cached");
    assert_eq!(again, stream);
    assert_eq!(lookups(&fake).len(), 1);
    // A failed stream drops the cache, so the next resolve looks it up again.
    resolver.stream_failed(&page);
    resolver.resolve(&page, &policy()).await.expect("fresh");
    assert_eq!(lookups(&fake).len(), 2);
}

#[tokio::test]
async fn a_page_the_catalog_has_not_seen_is_resolved_with_one_more_call() {
    let (_, resolver, fake) = setup(|_| answer(HLS_ADDRESS));
    let page = MediaUrl::parse(
        "https://soundcloud.com/fixture-artist-1/fixture-track-1",
        HOSTS,
    )
    .expect("url");
    let stream = resolver.resolve(&page, &policy()).await.expect("stream");
    assert_eq!(stream.info.bitrate_kbps, Some(160));
    let urls = fake.urls();
    assert!(urls.iter().any(|u| u.contains("/resolve?url=")));
    assert_eq!(lookups(&fake).len(), 1);
}

#[tokio::test]
async fn an_encrypted_only_track_is_not_available_and_no_lookup_is_made() {
    let (catalog, resolver, fake) = setup(|_| answer(HLS_ADDRESS));
    let tracks = songs(&catalog).await;
    let encrypted = tracks.last().expect("the encrypted-only fixture is last");
    let page = encrypted.source_url.clone().expect("page");
    let err = resolver
        .resolve(&page, &policy())
        .await
        .expect_err("cannot play");
    assert_eq!(err, StreamError::NotAvailable);
    assert!(lookups(&fake).is_empty(), "no bypass is tried");
}

#[tokio::test]
async fn a_preview_is_marked_and_a_progressive_file_is_a_file() {
    let (catalog, resolver, _) =
        setup(|_| answer("https://cf-preview-media.sndcdn.com/preview/0/30/ab.128.mp3?x=1"));
    let tracks = songs(&catalog).await;
    // Fixture order: two AAC tracks, an old full MP3, a preview, an encrypted one.
    let full = resolver
        .resolve(tracks[2].source_url.as_ref().expect("page"), &policy())
        .await
        .expect("mp3");
    assert_eq!(
        full.transport,
        Transport::File,
        "a plain file beats HLS of the same codec"
    );
    assert_eq!(
        (full.info.codec, full.info.bitrate_kbps),
        (Codec::Mp3, Some(128)),
        "the address states 128"
    );
    let preview = resolver
        .resolve(tracks[3].source_url.as_ref().expect("page"), &policy())
        .await
        .expect("preview");
    assert!(preview.preview, "a preview stays a preview");
}

#[tokio::test]
async fn a_codec_the_decoder_cannot_play_is_not_picked() {
    let (catalog, resolver, _) = setup(|_| answer(HLS_ADDRESS));
    let page = songs(&catalog).await.remove(0).source_url.expect("page");
    let mp3_only = FormatPolicy {
        playable: vec![Codec::Mp3],
        allow_hls: true,
    };
    assert_eq!(
        resolver.resolve(&page, &mp3_only).await,
        Err(StreamError::NotAvailable)
    );
    let no_hls = FormatPolicy {
        playable: vec![Codec::Aac],
        allow_hls: false,
    };
    assert_eq!(
        resolver.resolve(&page, &no_hls).await,
        Err(StreamError::NotAvailable)
    );
}

#[tokio::test]
async fn an_address_off_the_soundcloud_hosts_is_refused() {
    for bad in [
        "https://evil.example/a.m3u8",
        "http://playback.media-streaming.soundcloud.cloud/a.m3u8",
        "not a url",
    ] {
        let (catalog, resolver, _) = setup(move |_| answer(bad));
        let page = songs(&catalog).await.remove(0).source_url.expect("page");
        let err = resolver
            .resolve(&page, &policy())
            .await
            .expect_err("refused");
        assert!(matches!(err, StreamError::Parse(_)), "{bad}");
        assert!(!err.to_string().contains("evil"), "no address in the error");
    }
}

#[tokio::test]
async fn failures_become_plain_stream_errors() {
    let (catalog, resolver, _) = setup(|_| String::new());
    let page = songs(&catalog).await.remove(0).source_url.expect("page");
    assert_eq!(
        resolver.resolve(&page, &policy()).await,
        Err(StreamError::NotAvailable),
        "404"
    );
    let (catalog, resolver, _) = setup(|_| "not json".to_owned());
    let page = songs(&catalog).await.remove(0).source_url.expect("page");
    assert!(matches!(
        resolver.resolve(&page, &policy()).await,
        Err(StreamError::Parse(_))
    ));
    assert_eq!(
        stream_error(ClientError::RateLimited).to_string(),
        "network error: too many requests; try again later"
    );
}

#[tokio::test]
async fn a_media_lookup_never_leaves_the_api_host() {
    let client = ScClient::with(
        Arc::new(FakeHttp::new(|_, _| ok("{}"))) as Arc<dyn ScHttp>,
        Endpoints::production(),
        pacing(),
    );
    let err = client
        .get_api_url("https://evil.example/media/x/stream/hls", &[])
        .await;
    assert_eq!(err, Err(ClientError::Refused));
    let err = client
        .get_api_url("https://api-v2.soundcloud.com.evil.example/x", &[])
        .await;
    assert_eq!(err, Err(ClientError::Refused));
}

#[test]
fn the_resolver_lists_nothing_and_reports_the_native_client() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let client = Arc::new(ScClient::new(Arc::new(FakeHttp::new(|_, _| ok("{}")))));
    let resolver = ScStreamResolver::new(client, Arc::new(Shared::new()), rt.handle().clone());
    assert!(matches!(resolver.state(), ResolverState::Ready { .. }));
    let query = MediaQuery::Search {
        site: nemo_core::url_resolver::Site::SoundCloud,
        text: "a".into(),
        limit: 3,
    };
    assert_eq!(
        rt.block_on(resolver.list(&query)),
        Err(StreamError::NotAvailable)
    );
}

#[tokio::test]
async fn a_click_and_a_warm_hint_for_the_same_track_make_one_lookup() {
    let (catalog, resolver, fake) =
        setup(|_| answer("https://cf-preview-media.sndcdn.com/a.128.mp3"));
    let tracks = songs(&catalog).await;
    // The third fixture track is an old upload with a progressive file: nothing to prefetch.
    let page = tracks[2].source_url.clone().expect("page");
    resolver.warm(&page);
    let wanted = policy();
    let (one, two) = tokio::join!(
        resolver.resolve(&page, &wanted),
        resolver.resolve(&page, &wanted)
    );
    assert_eq!(one.expect("first"), two.expect("second"));
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(lookups(&fake).len(), 1, "one look-up for three callers");
}

#[tokio::test]
async fn the_streams_hook_opens_the_media_connection_once_while_a_listener_works() {
    let server =
        crate::test_server::TestServer::start(|_, _| crate::test_server::Response::new(403, ""));
    let (fake, client) = {
        let fake = Arc::new(FakeHttp::new(|url, _| {
            if url == "https://soundcloud.com/" {
                ok(r#"<script src="https://a-v2.sndcdn.com/assets/1-a.js"></script>"#)
            } else if url.starts_with("https://a-v2.sndcdn.com/") {
                ok(format!("client_id:\"{ID}\""))
            } else {
                ok("{}")
            }
        }));
        let client = Arc::new(ScClient::with(
            Arc::clone(&fake) as Arc<dyn ScHttp>,
            Endpoints::production(),
            pacing(),
        ));
        (fake, client)
    };
    let shared = Arc::new(Shared::with_preconnect(
        Rules::loopback(),
        server.url("/media-root"),
    ));
    let (_resolver, _opener) =
        crate::stream::streams_with(Arc::clone(&client), Handle::current(), shared);
    client.get("/search", &[]).await.expect("a search");
    client.get("/tracks/1", &[]).await.expect("a page");
    client
        .get_background("/tracks/2", &[])
        .await
        .expect("a hint");
    for _ in 0..100 {
        if !server.paths().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        server.paths(),
        ["/media-root"],
        "one connection opened ahead, not one per call"
    );
    assert!(
        fake.urls().iter().all(|u| !u.contains("media-root")),
        "never through the API layer"
    );
}

#[test]
fn a_web_app_the_client_cannot_read_is_the_one_answer_the_fallback_takes() {
    assert!(matches!(
        stream_error(ClientError::NoClientId),
        StreamError::Parse(_)
    ));
    assert!(matches!(
        stream_error(ClientError::Parse),
        StreamError::Parse(_)
    ));
    // A refusal, a missing track, a limit and a network failure are not.
    for refused in [ClientError::Refused, ClientError::NotFound] {
        assert_eq!(stream_error(refused), StreamError::NotAvailable);
    }
    for failed in [
        ClientError::RateLimited,
        ClientError::Network,
        ClientError::Timeout,
    ] {
        assert!(matches!(stream_error(failed), StreamError::Network(_)));
    }
}
