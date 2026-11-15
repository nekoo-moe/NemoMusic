//! Tests for channel pages: the parsers on the spike fixtures and the request order, the
//! id check and the backoff against a fake host.

use super::*;

const CHANNEL: &str = include_str!("../../../tests/fixtures/web/browse-channel.json");
const UPLOADS: &str = include_str!("../../../tests/fixtures/web/browse-videos.json");
const LOFI: &str = "UCSJ4gkVC6NrvII8umztf0Ow";
/// `params` of the Videos tab in the channel fixture.
const VIDEOS_PARAMS: &str = "EgZ2aWRlb3PyBgQKAjoA";

/// A playlists tab answer in the shape the search fixture shows for playlist lockups: one
/// playlist, the same playlist again, and a video lockup.
fn playlists_tab() -> String {
    let playlist = |title: &str| {
        json!({ "lockupViewModel": {
            "contentType": "LOCKUP_CONTENT_TYPE_PLAYLIST",
            "contentId": "PLabc_123-x",
            "metadata": { "lockupMetadataViewModel": {
                "title": { "content": title },
                "metadata": { "contentMetadataViewModel": { "metadataRows": [
                    { "metadataParts": [{ "text": { "content": "Updated today" } }] }
                ] } },
            } },
        } })
    };
    let video = json!({ "lockupViewModel": {
        "contentType": "LOCKUP_CONTENT_TYPE_VIDEO",
        "contentId": "abcdefghijk",
    } });
    let grid =
        json!({ "gridRenderer": { "items": [playlist("Study beats"), playlist("Again"), video] } });
    json!({ "contents": { "twoColumnBrowseResultsRenderer": { "tabs": [{ "tabRenderer": {
        "selected": true,
        "content": { "sectionListRenderer": { "contents": [
            { "itemSectionRenderer": { "contents": [grid] } }
        ] } },
    } }] } } })
    .to_string()
}

fn lofi() -> ChannelRef {
    ChannelRef {
        id: BrowseId(LOFI.to_owned()),
        name: "Lofi Girl".to_owned(),
    }
}

#[test]
fn the_channel_fixture_gives_the_header_and_the_tab_params_from_its_endpoints() {
    let page = parse_channel(CHANNEL).expect("parses");
    assert_eq!(page.summary.id.0, LOFI);
    assert_eq!(page.summary.name, "Lofi Girl");
    assert_eq!(
        page.summary.subscribers.as_deref(),
        Some("15.8M subscribers")
    );
    assert_eq!(page.videos_params.as_deref(), Some(VIDEOS_PARAMS));
    let playlists = page.playlists_params.expect("playlists tab");
    assert_ne!(playlists, VIDEOS_PARAMS);
}

#[test]
fn a_tab_params_value_that_is_not_plain_text_is_ignored() {
    let mut root: Value = serde_json::from_str(CHANNEL).expect("json");
    let tabs = root
        .pointer_mut("/contents/twoColumnBrowseResultsRenderer/tabs")
        .and_then(Value::as_array_mut)
        .expect("tabs");
    for tab in tabs.iter_mut() {
        if let Some(p) = tab.pointer_mut("/tabRenderer/endpoint/browseEndpoint/params") {
            *p = Value::String("has space".to_owned());
        }
    }
    let page = parse_channel(&root.to_string()).expect("parses");
    assert_eq!((page.videos_params, page.playlists_params), (None, None));
}

#[test]
fn a_channel_answer_without_an_id_or_json_is_a_parse_error() {
    assert!(matches!(
        parse_channel("<html>"),
        Err(CatalogError::Parse(_))
    ));
    assert!(matches!(parse_channel("{}"), Err(CatalogError::Parse(_))));
}

#[test]
fn the_uploads_fixture_gives_videos_with_length_views_and_the_channel() {
    let uploads = parse_uploads(UPLOADS, &lofi()).expect("parses");
    assert_eq!(uploads.len(), 2);
    let first = &uploads[0];
    assert_eq!(
        first.track.video_id.as_ref().map(VideoId::as_str),
        Some("d3QQHqRmvH4")
    );
    assert_eq!(
        first.track.title,
        "The Christmas gift that started it all (animated clip)"
    );
    assert_eq!(first.track.duration, Some(Duration::from_secs(73)));
    assert_eq!(first.views.as_deref(), Some("136K views"));
    assert_eq!(first.channel, Some(lofi()));
    assert_eq!(first.track.artists, vec!["Lofi Girl".to_owned()]);
    assert!(!first.live);
    assert!(
        matches!(&first.track.artwork, Artwork::Remote(t) if t[0].url.starts_with("https://i.ytimg.com/"))
    );
    assert_eq!(uploads[1].track.duration, Some(Duration::from_secs(39)));
}

#[test]
fn a_live_badge_on_an_upload_marks_it_live_with_no_length() {
    let live = UPLOADS.replace("\"text\": \"1:13\"", "\"text\": \"LIVE\"");
    let uploads = parse_uploads(&live, &lofi()).expect("parses");
    assert!(uploads[0].live);
    assert_eq!(uploads[0].track.duration, None);
}

#[test]
fn the_playlists_tab_lists_each_playlist_once_without_an_author() {
    let playlists = parse_playlists(&playlists_tab()).expect("parses");
    assert_eq!(playlists.len(), 1);
    assert_eq!(playlists[0].id.0, "PLabc_123-x");
    assert_eq!(playlists[0].title, "Study beats");
    assert_eq!(playlists[0].author, None);
    assert_eq!(parse_playlists("{}").expect("empty"), Vec::new());
    assert!(matches!(parse_playlists("x"), Err(CatalogError::Parse(_))));
}

/// The web host for channel pages: logs `(browseId, params)` of each request.
struct ChannelWeb {
    requests: Mutex<Vec<(String, Option<String>)>>,
    status: u16,
}

impl WebPost for ChannelWeb {
    fn post<'a>(&'a self, url: &'a str, body: String) -> BoxFuture<'a, Result<WebAnswer, String>> {
        assert_eq!(url, BROWSE_URL);
        let json: Value = serde_json::from_str(&body).expect("json body");
        assert_eq!(json["context"]["client"]["clientName"], "WEB");
        let id = json["browseId"].as_str().expect("browseId").to_owned();
        let params = json
            .get("params")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let text = match params.as_deref() {
            None => CHANNEL.to_owned(),
            Some(VIDEOS_PARAMS) => UPLOADS.to_owned(),
            Some(_) => playlists_tab(),
        };
        self.requests.lock().expect("log").push((id, params));
        let answer = WebAnswer {
            status: self.status,
            body: text,
            retry_after: None,
        };
        Box::pin(async move { Ok(answer) })
    }
}

fn channel_ext(status: u16) -> WebCatalogExt<ChannelWeb> {
    let web = ChannelWeb {
        requests: Mutex::default(),
        status,
    };
    WebCatalogExt::new(Arc::new(Music(Log::default())), web)
}

#[test]
fn a_channel_opens_with_the_header_then_uploads_then_playlists_in_order() {
    let catalog = channel_ext(200);
    let detail = block_on(catalog.channel(&BrowseId(LOFI.to_owned()))).expect("channel");
    assert_eq!(detail.summary.name, "Lofi Girl");
    assert_eq!(detail.uploads.len(), 2);
    assert_eq!(detail.playlists.len(), 1);
    let requests = catalog.web.requests.lock().expect("log");
    assert_eq!(requests.len(), 3);
    assert!(requests.iter().all(|(id, _)| id == LOFI));
    assert_eq!(requests[0].1, None);
    assert_eq!(requests[1].1.as_deref(), Some(VIDEOS_PARAMS));
    assert!(requests[2].1.is_some());
}

#[test]
fn an_id_that_is_not_a_channel_id_is_not_found_and_sends_nothing() {
    let catalog = channel_ext(200);
    for id in ["VLPLxyz", "UC bad/id", ""] {
        assert_eq!(
            block_on(catalog.channel(&BrowseId(id.to_owned()))),
            Err(CatalogError::NotFound)
        );
    }
    assert!(catalog.web.requests.lock().expect("log").is_empty());
}

#[test]
fn a_429_on_a_channel_stops_further_requests() {
    let catalog = channel_ext(429);
    let id = BrowseId(LOFI.to_owned());
    assert_eq!(
        block_on(catalog.channel(&id)),
        Err(CatalogError::RateLimited)
    );
    assert_eq!(
        block_on(catalog.channel(&id)),
        Err(CatalogError::RateLimited)
    );
    assert_eq!(catalog.web.requests.lock().expect("log").len(), 1);
}

#[test]
fn other_statuses_on_a_channel_are_network_errors_that_name_the_channel_page() {
    let catalog = channel_ext(500);
    assert_eq!(
        block_on(catalog.channel(&BrowseId(LOFI.to_owned()))),
        Err(CatalogError::Network("channel page: HTTP 500".to_owned()))
    );
    assert!(!catalog.blocked());
}

#[test]
fn the_browse_body_carries_params_only_when_given() {
    let id = BrowseId(LOFI.to_owned());
    let plain: Value = serde_json::from_str(&browse_body(&id, None)).expect("json");
    assert_eq!(plain["browseId"], LOFI);
    assert!(plain.get("params").is_none());
    let tab: Value = serde_json::from_str(&browse_body(&id, Some("abc%3D"))).expect("json");
    assert_eq!(tab["params"], "abc%3D");
}
