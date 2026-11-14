//! Tests for the home, explore, mood and continuation readers.

use std::time::Duration;

use super::fixtures::{
    ID, ID2, browse, card, carousel, chips, link, row, runs, section_list, watch,
};
use crate::feeds::GREY_OUT;
use crate::feeds::item::{mood_id, split_mood_id, two_row_item};
use crate::feeds::shelf::{
    HOME_SCOPES, LIST_SCOPES, continuation_token, parse_explore, parse_home_page, parse_mood_page,
    parse_sections,
};
use nemo_core::catalog::{BrowseId, FeedItem, HomeFeed, ShelfKind};
use serde_json::json;

#[test]
fn home_page_has_chips_subtitles_podcasts_and_a_next_token() {
    let mut page = section_list(vec![
        carousel(
            "Listen again",
            Some("SOMEONE"),
            vec![card(
                "My Mix",
                json!({ "runs": [ link("Someone", "UCsynthetic9"), { "text": " • " }, { "text": "20 songs" } ] }),
                browse("VLPLsynthetic1", "MUSIC_PAGE_TYPE_PLAYLIST"),
            )],
        ),
        carousel(
            "Shows",
            None,
            vec![card(
                "A Show",
                json!({ "runs": [ link("The Host", "UCsynthetic2") ] }),
                browse("MPSPsynthetic1", "MUSIC_PAGE_TYPE_PODCAST_SHOW_DETAIL_PAGE"),
            )],
        ),
        carousel(
            "New albums",
            None,
            vec![card(
                "An Album",
                json!({ "runs": [ { "text": "Album" }, { "text": " • " }, link("Artist One", "UCsynthetic3"), { "text": " • " }, { "text": "2024" } ] }),
                browse("MPREbsynthetic2", "MUSIC_PAGE_TYPE_ALBUM"),
            )],
        ),
    ]);
    page["contents"]["singleColumnBrowseResultsRenderer"]["tabs"][0]["tabRenderer"]["content"]["sectionListRenderer"]
        ["header"] = chips();
    page["contents"]["singleColumnBrowseResultsRenderer"]["tabs"][0]["tabRenderer"]["content"]["sectionListRenderer"]
        ["continuations"] = json!([ { "nextContinuationData": { "continuation": "tok123" } } ]);
    let (feed, next) = parse_home_page(&page.to_string());
    assert_eq!(next.as_deref(), Some("tok123"));
    // The chip without params cannot filter anything and is dropped.
    assert_eq!(feed.chips.len(), 1);
    assert_eq!(feed.chips[0].title, "Relax");
    assert_eq!(feed.chips[0].id.0, "FEmusic_home|ggM8abc%3D");
    assert_eq!(feed.shelves.len(), 3);
    assert_eq!(feed.shelves[0].subtitle.as_deref(), Some("SOMEONE"));
    assert_eq!(feed.shelves[0].kind, ShelfKind::Cards);
    match &feed.shelves[0].items[0] {
        FeedItem::Playlist(p) => {
            assert_eq!(p.author.as_deref(), Some("Someone"));
            assert_eq!(p.thumbnails[0].url, "https://img.invalid/cover");
        }
        other => panic!("{other:?}"),
    }
    match &feed.shelves[1].items[0] {
        FeedItem::Podcast(p) => {
            assert_eq!(p.id.0, "MPSPsynthetic1");
            assert_eq!(p.author.as_deref(), Some("The Host"));
        }
        other => panic!("{other:?}"),
    }
    match &feed.shelves[2].items[0] {
        FeedItem::Album(a) => {
            assert_eq!(a.year, Some(2024));
            assert_eq!(a.artists, vec!["Artist One".to_owned()]);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn translated_labels_do_not_leak_into_names() {
    let item = card(
        "Mix",
        json!({ "runs": [ { "text": "Danh sách phát" }, { "text": " • " }, link("Chủ", "UCsynthetic4"), { "text": " • " }, { "text": "139 bài hát" } ] }),
        browse("VLPLsynthetic2", "MUSIC_PAGE_TYPE_PLAYLIST"),
    );
    match two_row_item(&item["musicTwoRowItemRenderer"]) {
        Some(FeedItem::Playlist(p)) => assert_eq!(p.author.as_deref(), Some("Chủ")),
        other => panic!("{other:?}"),
    }
}

#[test]
fn songs_and_videos_are_told_apart_by_the_music_video_type() {
    let song = card(
        "A Song",
        json!({ "runs": [ link("Artist One", "UCsynthetic1") ] }),
        watch(ID, "MUSIC_VIDEO_TYPE_ATV"),
    );
    let video = card(
        "A Video",
        json!({ "runs": [ link("Artist Two", "UCsynthetic2"), { "text": " • " }, { "text": "1.2M views" } ] }),
        watch(ID2, "MUSIC_VIDEO_TYPE_OMV"),
    );
    let upload = card(
        "Upload",
        json!({ "runs": [ { "text": "Some Channel" } ] }),
        watch(ID, "MUSIC_VIDEO_TYPE_UGC"),
    );
    let episode = card(
        "Episode",
        json!({ "runs": [] }),
        watch(ID, "MUSIC_VIDEO_TYPE_PODCAST_EPISODE"),
    );
    let shelves = parse_sections(&[carousel("Mixed", None, vec![song, video, upload, episode])]);
    let items = &shelves[0].items;
    assert!(matches!(&items[0], FeedItem::Song(t) if t.artists == ["Artist One"]));
    assert!(matches!(&items[1], FeedItem::Video(t) if t.artists == ["Artist Two"]));
    assert!(matches!(&items[2], FeedItem::Video(t) if t.artists == ["Some Channel"]));
    assert!(matches!(&items[3], FeedItem::Episode(_)));
    assert_eq!(shelves[0].kind, ShelfKind::Cards);
}

#[test]
fn a_shelf_of_rows_is_a_quick_picks_list() {
    let shelves = parse_sections(&[carousel(
        "Quick picks",
        None,
        vec![row("Row Song", ID, "Artist One", Some("An Album"), "3:45")],
    )]);
    assert_eq!(shelves[0].kind, ShelfKind::QuickPicks);
    match &shelves[0].items[0] {
        FeedItem::Song(t) => {
            assert_eq!(t.title, "Row Song");
            assert_eq!(t.artists, vec!["Artist One".to_owned()]);
            assert_eq!(t.album.as_deref(), Some("An Album"));
            assert_eq!(t.duration, Some(Duration::from_secs(225)));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn unusable_entries_are_dropped_and_the_rest_stays() {
    let grey = json!({ "musicTwoRowItemRenderer": {
        "title": runs(&["Greyed"]),
        "musicItemRendererDisplayPolicy": GREY_OUT,
        "navigationEndpoint": watch(ID, "MUSIC_VIDEO_TYPE_ATV")
    } });
    let bad_id = card(
        "Bad id",
        json!({ "runs": [] }),
        watch("short", "MUSIC_VIDEO_TYPE_ATV"),
    );
    let no_nav = json!({ "musicTwoRowItemRenderer": { "title": runs(&["No nav"]) } });
    let create = card(
        "New playlist",
        json!({}),
        json!({ "createPlaylistEndpoint": { "params": "x" } }),
    );
    let other_page = card(
        "Other",
        json!({}),
        browse("FEsomething", "MUSIC_PAGE_TYPE_UNKNOWN"),
    );
    let good = card(
        "Good",
        json!({}),
        browse("VLPLsynthetic3", "MUSIC_PAGE_TYPE_PLAYLIST"),
    );
    let shelves = parse_sections(&[
        carousel(
            "Mixed",
            None,
            vec![grey, bad_id, no_nav, create, other_page, good],
        ),
        carousel("Empty", None, vec![]),
        json!({ "musicDescriptionShelfRenderer": {} }),
        carousel("", None, vec![]),
    ]);
    assert_eq!(shelves.len(), 1);
    assert_eq!(shelves[0].items.len(), 1);
    assert!(parse_sections(&[]).is_empty());
    assert_eq!(parse_home_page("nope").0, HomeFeed::default());
}

#[test]
fn mood_tiles_become_chips_and_ids_round_trip() {
    let tile = json!({ "musicNavigationButtonRenderer": {
        "buttonText": runs(&["Chill"]),
        "clickCommand": { "browseEndpoint": {
            "browseId": "FEmusic_moods_and_genres_category", "params": "ggMPOg32abc" } }
    } });
    let bad = json!({ "musicNavigationButtonRenderer": {
        "buttonText": runs(&["Evil"]),
        "clickCommand": { "browseEndpoint": { "browseId": "FEx", "params": "a b;c" } }
    } });
    let shelves = parse_sections(&[carousel("Moods", None, vec![tile, bad])]);
    assert_eq!(shelves[0].kind, ShelfKind::Chips);
    assert_eq!(shelves[0].items.len(), 1);
    let FeedItem::Mood(mood) = &shelves[0].items[0] else {
        panic!("not a mood");
    };
    assert_eq!(mood.id.0, "FEmusic_moods_and_genres_category|ggMPOg32abc");
    assert_eq!(
        split_mood_id(&mood.id),
        Some(("FEmusic_moods_and_genres_category", Some("ggMPOg32abc")))
    );
    assert_eq!(
        split_mood_id(&BrowseId("FEx".to_owned())),
        Some(("FEx", None))
    );
    for bad in ["", "a b|c", "FEx|a;b", "|abc", "FEx|", "a/b"] {
        assert_eq!(split_mood_id(&BrowseId(bad.to_owned())), None, "{bad:?}");
    }
    assert_eq!(mood_id("FEx", Some("a b")), None);
}

#[test]
fn explore_splits_new_releases_moods_and_the_rest() {
    let album = |title: &str, id: &str| {
        card(
            title,
            json!({ "runs": [ link("Artist", "UCsynthetic3") ] }),
            browse(id, "MUSIC_PAGE_TYPE_ALBUM"),
        )
    };
    let tile = json!({ "musicNavigationButtonRenderer": {
        "buttonText": runs(&["Focus"]),
        "clickCommand": { "browseEndpoint": {
            "browseId": "FEmusic_moods_and_genres_category", "params": "ggMPabc" } }
    } });
    let other_tile = json!({ "musicNavigationButtonRenderer": {
        "buttonText": runs(&["Charts"]),
        "clickCommand": { "browseEndpoint": { "browseId": "FEmusic_charts" } }
    } });
    let video = card(
        "Clip",
        json!({ "runs": [ link("Artist", "UCsynthetic3") ] }),
        watch(ID, "MUSIC_VIDEO_TYPE_OMV"),
    );
    let page = section_list(vec![
        carousel(
            "New albums",
            None,
            vec![
                album("One", "MPREbsynthetic4"),
                album("Two", "MPREbsynthetic5"),
            ],
        ),
        carousel(
            "Top songs",
            None,
            vec![row("Hit", ID, "Artist", None, "3:00")],
        ),
        carousel("Moods", None, vec![tile, other_tile]),
        carousel("Videos", None, vec![video]),
        carousel("More albums", None, vec![album("Three", "MPREbsynthetic6")]),
    ]);
    let explore = parse_explore(&page.to_string());
    assert_eq!(explore.new_releases.len(), 2);
    assert_eq!(explore.moods.len(), 1);
    assert_eq!(explore.moods[0].title, "Focus");
    let titles: Vec<&str> = explore.shelves.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["Top songs", "Videos", "More albums"]);
    assert!(explore.charts.is_empty());
    assert!(matches!(&explore.shelves[1].items[0], FeedItem::Video(_)));
}

#[test]
fn a_mood_page_keeps_carousels_and_grids() {
    let grid = json!({ "gridRenderer": {
        "header": { "gridHeaderRenderer": { "title": runs(&["Featured"]) } },
        "items": [ card("Radio", json!({}), browse("VLRDCLAKsynthetic", "MUSIC_PAGE_TYPE_PLAYLIST")) ]
    } });
    let page = section_list(vec![
        carousel(
            "Picks",
            None,
            vec![card(
                "List",
                json!({}),
                browse("VLPLsynthetic6", "MUSIC_PAGE_TYPE_PLAYLIST"),
            )],
        ),
        grid,
    ]);
    let shelves = parse_mood_page(&page.to_string());
    assert_eq!(shelves.len(), 2);
    assert_eq!(shelves[1].title, "Featured");
    assert!(parse_mood_page("nope").is_empty());
}

#[test]
fn continuation_tokens_are_taken_from_the_right_container() {
    let page = json!({ "a": { "nextContinuationData": { "continuation": "stray" } },
        "gridRenderer": { "continuations": [ { "nextContinuationData": { "continuation": "mine" } } ] } });
    assert_eq!(
        continuation_token(&page, &LIST_SCOPES).as_deref(),
        Some("mine")
    );
    assert_eq!(continuation_token(&page, &HOME_SCOPES), None);
    let new_style = json!({ "appendContinuationItemsAction": { "continuationItems": [
        { "continuationItemRenderer": { "continuationEndpoint": {
            "continuationCommand": { "token": "newtok" } } } } ] } });
    assert_eq!(
        continuation_token(&new_style, &LIST_SCOPES).as_deref(),
        Some("newtok")
    );
    assert_eq!(continuation_token(&json!({}), &LIST_SCOPES), None);
}
