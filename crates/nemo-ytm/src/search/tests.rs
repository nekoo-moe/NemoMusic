//! Tests of the unfiltered search reader, on a recorded answer and on hand-built shapes.

use nemo_core::catalog::{SearchKind, TopResult};
use serde_json::{Value, json};
use ytmapi_rs::query::PostQuery;

use super::{SearchQuery, parse_search};

/// Anonymous answer for "HVL", 2026-11-15 (see `tests/fixtures/music/README.md`).
const HVL: &str = include_str!("../../tests/fixtures/music/search-hvl.json");

fn answer(sections: &Value) -> String {
    json!({ "contents": { "tabbedSearchResultsRenderer": { "tabs": [ { "tabRenderer": {
        "content": { "sectionListRenderer": { "contents": sections } }
    } } ] } } })
    .to_string()
}

fn run(text: &str) -> Value {
    json!({ "text": text })
}

fn link(text: &str, id: &str) -> Value {
    json!({ "text": text, "navigationEndpoint": { "browseEndpoint": { "browseId": id } } })
}

fn song_row(id: &str, title: &str, sub: &[Value]) -> Value {
    json!({ "musicResponsiveListItemRenderer": {
        "flexColumns": [
            { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [ run(title) ] } } },
            { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": sub } } }
        ],
        "playlistItemData": { "videoId": id },
        "overlay": { "musicItemThumbnailOverlayRenderer": { "content": { "musicPlayButtonRenderer": {
            "playNavigationEndpoint": { "watchEndpoint": { "videoId": id,
                "watchEndpointMusicSupportedConfigs": { "watchEndpointMusicConfig": {
                    "musicVideoType": "MUSIC_VIDEO_TYPE_ATV" } } } }
        } } } }
    } })
}

#[test]
fn the_recorded_answer_keeps_the_top_result_and_the_order() {
    let r = parse_search(HVL).expect("known layout");
    let Some(TopResult::Album(top)) = &r.top else {
        panic!("top result should be the album, got {:?}", r.top);
    };
    assert_eq!(top.title, "HVL");
    assert_eq!(top.id.0, "MPREb_gsgslx4jAfe");
    assert_eq!(top.artists, ["RPT MCK"]);
    assert_eq!(top.year, Some(2026));
    assert!(!top.thumbnails.is_empty());
    assert_eq!(
        r.order,
        [
            SearchKind::Songs,
            SearchKind::Artists,
            SearchKind::Videos,
            SearchKind::Playlists,
            SearchKind::Albums,
        ]
    );
    let titles: Vec<&str> = r.songs.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        ["Nếu Như Ta Chẳng Còn (feat. A$AP Ướt Mi)", "Moliti", "IDK"]
    );
    assert_eq!(r.songs[0].artists, ["RPT MCK"]);
    assert_eq!(r.songs[1].artists, ["HVL"]);
    let artists: Vec<&str> = r.artists.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(artists, ["RPT MCK", "HVL"]);
    assert_eq!(r.videos.len(), 2);
    assert_eq!(r.videos[0].track.title, "H & H");
    assert_eq!(r.videos[0].track.artists, ["RPT MCK"]);
    assert_eq!(r.videos[0].views.as_deref(), Some("861K views"));
    assert_eq!(r.playlists.len(), 1);
    assert_eq!(r.playlists[0].author.as_deref(), Some("Jerran"));
    assert_eq!(r.albums.len(), 1);
    assert_eq!(r.albums[0].artists, ["DJ lungtung"]);
    assert_eq!(r.albums[0].year, Some(2026));
    assert!(
        r.channels.is_empty(),
        "profiles, episodes and podcasts are skipped"
    );
}

#[test]
fn a_type_label_in_any_language_is_not_an_artist() {
    let rows = json!([
        { "itemSectionRenderer": { "contents": [
            song_row("abcdefghijk", "Elegie", &[run("Bài hát"), run(" • "), run("RPT MCK"), run(" • "), run("3:21")])
        ] } }
    ]);
    let r = parse_search(&answer(&rows)).expect("known layout");
    assert_eq!(r.songs.len(), 1);
    assert_eq!(r.songs[0].artists, ["RPT MCK"]);
    assert_eq!(r.songs[0].duration.map(|d| d.as_secs()), Some(201));
}

#[test]
fn the_older_shelf_layout_and_card_rows_are_read_in_order() {
    let card = json!({ "musicCardShelfRenderer": {
        "title": { "runs": [ { "text": "Song A", "navigationEndpoint": { "watchEndpoint": {
            "videoId": "abcdefghijk",
            "watchEndpointMusicSupportedConfigs": { "watchEndpointMusicConfig": {
                "musicVideoType": "MUSIC_VIDEO_TYPE_ATV" } } } } } ] },
        "subtitle": { "runs": [ run("Song"), run(" • "), link("Ann", "UCann") ] },
        "contents": [
            song_row("abcdefghijk", "Song A", &[run("Song"), run(" • "), link("Ann", "UCann")]),
            song_row("lmnopqrstuv", "Song B", &[run("Song"), run(" • "), link("Ann", "UCann")])
        ]
    } });
    let shelf = json!({ "musicShelfRenderer": {
        "title": { "runs": [ run("Songs") ] },
        "contents": [
            song_row("wxyzabcdefg", "Song C", &[run("Bob"), run(" • "), run("2:00")])
        ]
    } });
    let r = parse_search(&answer(&json!([card, shelf]))).expect("known layout");
    assert!(matches!(&r.top, Some(TopResult::Song(t)) if t.title == "Song A"));
    let titles: Vec<&str> = r.songs.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, ["Song B", "Song C"], "the top song is not repeated");
    assert_eq!(r.songs[1].artists, ["Bob"], "a shelf row has no type label");
    assert_eq!(r.order, [SearchKind::Songs]);
}

#[test]
fn an_unknown_layout_is_none_and_no_results_is_empty() {
    assert_eq!(parse_search("{}"), None);
    assert_eq!(parse_search("not json"), None);
    let empty = parse_search(&answer(&json!([]))).expect("known layout");
    assert!(empty.is_empty());
}

#[test]
fn greyed_out_rows_and_bad_ids_are_dropped() {
    let mut grey = song_row("abcdefghijk", "Gone", &[run("Song")]);
    grey["musicResponsiveListItemRenderer"]["musicItemRendererDisplayPolicy"] =
        json!("MUSIC_ITEM_RENDERER_DISPLAY_POLICY_GREY_OUT");
    let bad = song_row("bad id", "Bad", &[run("Song")]);
    let rows = json!([{ "itemSectionRenderer": { "contents": [grey, bad] } }]);
    let r = parse_search(&answer(&rows)).expect("known layout");
    assert!(r.is_empty());
}

#[test]
fn the_request_sends_only_the_query() {
    let q = SearchQuery::new("HVL");
    let body = q.header();
    assert_eq!(body.get("query"), Some(&json!("HVL")));
    assert_eq!(body.len(), 1, "no params: the unfiltered search");
    assert!(PostQuery::params(&q).is_empty());
    assert_eq!(q.path(), "search");
}
