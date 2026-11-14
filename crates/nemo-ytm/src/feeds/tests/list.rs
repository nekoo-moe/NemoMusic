//! Tests for the library, playlist, history and podcast readers.

use std::time::Duration;

use super::fixtures::{ID, ID2, browse, card, link, row, runs, section_list, thumbs};
use crate::feeds::GREY_OUT;
use crate::feeds::list::{
    parse_library_albums, parse_library_artists, parse_library_playlists, parse_song_rows,
};
use crate::feeds::playlist::{
    PlaylistHeader, parse_history, parse_playlist_header, parse_podcast, parse_watch_playlist,
};
use nemo_core::catalog::BrowseId;
use serde_json::{Value, json};

#[test]
fn library_playlists_skip_the_create_tile_and_follow_the_token() {
    let create = json!({ "musicTwoRowItemRenderer": {
        "title": runs(&["New playlist"]),
        "navigationEndpoint": { "createPlaylistEndpoint": { "params": "x" } }
    } });
    let liked = card(
        "Liked music",
        json!({ "runs": [ { "text": "Auto playlist" } ] }),
        browse("VLLM", "MUSIC_PAGE_TYPE_PLAYLIST"),
    );
    let own = card(
        "Mine",
        json!({ "runs": [ { "text": "Playlist" }, { "text": " • " }, { "text": "139 songs" } ] }),
        browse("VLPLsynthetic4", "MUSIC_PAGE_TYPE_PLAYLIST"),
    );
    let first = json!({ "contents": { "singleColumnBrowseResultsRenderer": { "tabs": [ {
        "tabRenderer": { "content": { "sectionListRenderer": { "contents": [
            { "gridRenderer": {
                "items": [create, liked, own],
                "continuations": [ { "nextContinuationData": { "continuation": "next1" } } ]
            } }
        ] } } }
    } ] } } });
    let page = parse_library_playlists(&first.to_string());
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].id.0, "VLLM");
    assert_eq!(page.items[1].title, "Mine");
    assert_eq!(page.items[1].author, None);
    assert_eq!(page.next.as_deref(), Some("next1"));

    let more = card(
        "More",
        json!({}),
        browse("VLPLsynthetic5", "MUSIC_PAGE_TYPE_PLAYLIST"),
    );
    let second = json!({ "continuationContents": { "gridContinuation": { "items": [more] } } });
    let page = parse_library_playlists(&second.to_string());
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.next, None);
    assert_eq!(parse_library_playlists("nope").items.len(), 0);
}

#[test]
fn library_albums_and_artists() {
    let album = card(
        "Saved",
        json!({ "runs": [ { "text": "Album" }, { "text": " • " }, link("Artist One", "UCsynthetic3"), { "text": " • " }, { "text": "2020" } ] }),
        browse("MPREbsynthetic3", "MUSIC_PAGE_TYPE_ALBUM"),
    );
    let grid = json!({ "gridRenderer": { "items": [album] } });
    let albums = parse_library_albums(&json!({ "contents": [grid] }).to_string());
    assert_eq!(albums.items.len(), 1);
    assert_eq!(albums.items[0].year, Some(2020));

    let artist = json!({ "musicResponsiveListItemRenderer": {
        "thumbnail": thumbs("https://img.invalid/a"),
        "flexColumns": [ { "musicResponsiveListItemFlexColumnRenderer": { "text": runs(&["Band"]) } } ],
        "navigationEndpoint": browse("UCsynthetic7", "MUSIC_PAGE_TYPE_ARTIST")
    } });
    let shelf = json!({ "musicShelfRenderer": {
        "contents": [artist],
        "continuations": [ { "nextContinuationData": { "continuation": "more" } } ]
    } });
    let artists = parse_library_artists(&json!({ "contents": [shelf] }).to_string());
    assert_eq!(artists.items.len(), 1);
    assert_eq!(artists.items[0].name, "Band");
    assert_eq!(artists.items[0].thumbnails.len(), 1);
    assert_eq!(artists.next.as_deref(), Some("more"));
}

#[test]
fn liked_songs_rows_skip_greyed_items_and_only_use_their_own_token() {
    let mut grey = row("Gone", ID2, "Artist", None, "1:00");
    grey["musicResponsiveListItemRenderer"]["musicItemRendererDisplayPolicy"] = json!(GREY_OUT);
    let page = json!({
        "contents": { "twoColumnBrowseResultsRenderer": { "secondaryContents": {
            "sectionListRenderer": {
                "contents": [ { "musicPlaylistShelfRenderer": {
                    "contents": [
                        row("First", ID, "Artist One", Some("Album X"), "3:45"),
                        grey,
                        json!({ "musicResponsiveListItemRenderer": { "flexColumns": [] } })
                    ],
                    "continuations": [ { "nextContinuationData": { "continuation": "rows2" } } ]
                } } ],
                "continuations": [ { "nextContinuationData": { "continuation": "elsewhere" } } ]
            }
        } } }
    });
    let parsed = parse_song_rows(&page.to_string());
    assert_eq!(parsed.items.len(), 1);
    assert_eq!(parsed.items[0].title, "First");
    assert_eq!(parsed.next.as_deref(), Some("rows2"));

    let later = json!({ "continuationContents": { "musicPlaylistShelfContinuation": {
        "contents": [ row("Second", ID2, "Artist Two", None, "2:00") ]
    } } });
    let parsed = parse_song_rows(&later.to_string());
    assert_eq!(parsed.items.len(), 1);
    assert_eq!(parsed.next, None);
}

#[test]
fn a_playlist_header_is_read_also_inside_the_editable_header() {
    let page = json!({ "header": { "musicEditablePlaylistDetailHeaderRenderer": {
        "editHeader": {},
        "header": { "musicResponsiveHeaderRenderer": {
            "title": runs(&["My Playlist"]),
            "straplineTextOne": runs(&["Owner"]),
            "thumbnail": thumbs("https://img.invalid/h")
        } }
    } } });
    let header = parse_playlist_header(&page.to_string());
    assert_eq!(header.title.as_deref(), Some("My Playlist"));
    assert_eq!(header.author.as_deref(), Some("Owner"));
    assert_eq!(header.thumbnails.len(), 1);
    assert_eq!(parse_playlist_header("{}"), PlaylistHeader::default());
}

/// A browse page as playlists of every kind come back on 2026-11-11 (`PL`, `OLAK5uy`,
/// `RDCLAK`, `RDTMAK`): two-column layout, responsive header, playlist shelf.
fn playlist_page(rows: Vec<Value>) -> Value {
    json!({
        "contents": { "twoColumnBrowseResultsRenderer": {
            "tabs": [ { "tabRenderer": { "content": { "sectionListRenderer": { "contents": [
                { "musicResponsiveHeaderRenderer": {
                    "title": runs(&["Curated Radio"]),
                    "straplineTextOne": runs(&["YouTube Music"]),
                    "subtitle": runs(&["Playlist", " • ", "2026"]),
                    "thumbnail": thumbs("https://img.invalid/p")
                } }
            ] } } } } ],
            "secondaryContents": { "sectionListRenderer": { "contents": [
                { "musicPlaylistShelfRenderer": { "contents": rows } }
            ] } }
        } }
    })
}

#[test]
fn every_playlist_kind_reads_header_and_rows() {
    let page = playlist_page(vec![
        row("One", ID, "Artist One", Some("Album X"), "3:45"),
        row("Two", ID2, "Artist Two", None, "2:00"),
    ])
    .to_string();
    let header = parse_playlist_header(&page);
    assert_eq!(header.title.as_deref(), Some("Curated Radio"));
    assert_eq!(header.author.as_deref(), Some("YouTube Music"));
    assert_eq!(header.year, Some(2026));
    let rows = parse_song_rows(&page);
    assert_eq!(rows.items.len(), 2);
    assert_eq!(rows.items[1].artists, vec!["Artist Two".to_owned()]);
}

#[test]
fn an_album_page_keeps_rows_without_artists() {
    let bare = json!({ "musicResponsiveListItemRenderer": {
        "flexColumns": [ { "musicResponsiveListItemFlexColumnRenderer": {
            "text": runs(&["Track"]) } } ],
        "fixedColumns": [ { "musicResponsiveListItemFixedColumnRenderer": {
            "text": runs(&["4:01"]) } } ],
        "playlistItemData": { "videoId": ID }
    } });
    let page = json!({ "contents": { "twoColumnBrowseResultsRenderer": {
        "tabs": [ { "tabRenderer": { "content": { "sectionListRenderer": { "contents": [
            { "musicResponsiveHeaderRenderer": {
                "title": runs(&["Album"]),
                "straplineTextOne": { "runs": [ link("The Band", "UCsynthetic3") ] },
                "subtitle": runs(&["Album", " • ", "2019"])
            } } ] } } } } ],
        "secondaryContents": { "sectionListRenderer": { "contents": [
            { "musicShelfRenderer": { "contents": [ bare ] } } ] } }
    } } })
    .to_string();
    let rows = parse_song_rows(&page);
    assert_eq!(rows.items.len(), 1);
    assert!(rows.items[0].artists.is_empty());
    let header = parse_playlist_header(&page);
    assert_eq!(header.author.as_deref(), Some("The Band"));
    assert_eq!(header.year, Some(2019));
}

#[test]
fn a_watch_playlist_lists_radio_tracks() {
    let video = |id: &str, title: &str| {
        json!({ "playlistPanelVideoRenderer": {
            "videoId": id,
            "title": runs(&[title]),
            "longBylineText": { "runs": [
                link("Artist One", "UCsynthetic1"), { "text": " • " },
                link("Album X", "MPREbsynthetic1"), { "text": " • " }, { "text": "2020" }
            ] },
            "lengthText": runs(&["3:21"]),
            "thumbnail": thumbs("https://img.invalid/w")
        } })
    };
    let mut gone = video(ID2, "Gone");
    gone["playlistPanelVideoRenderer"]["unplayableText"] = runs(&["Not available"]);
    let wrapped = json!({ "playlistPanelVideoWrapperRenderer": { "primaryRenderer":
        video("ccccccccccc", "Wrapped") } });
    let answer = json!({ "contents": { "singleColumnMusicWatchNextResultsRenderer": {
        "tabbedRenderer": { "watchNextTabbedResultsRenderer": { "tabs": [ { "tabRenderer": {
            "content": { "musicQueueRenderer": { "content": { "playlistPanelRenderer": {
                "contents": [ video(ID, "First"), gone, wrapped,
                    json!({ "automixPreviewVideoRenderer": {} }) ]
            } } } }
        } } ] } }
    } } })
    .to_string();
    let tracks = parse_watch_playlist(&answer);
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0].title, "First");
    assert_eq!(tracks[0].artists, vec!["Artist One".to_owned()]);
    assert_eq!(tracks[0].album.as_deref(), Some("Album X"));
    assert_eq!(tracks[0].duration, Some(Duration::from_secs(201)));
    assert_eq!(tracks[1].title, "Wrapped");
    assert!(parse_watch_playlist("{}").is_empty());
    assert!(parse_watch_playlist("not json").is_empty());
}

#[test]
fn history_is_grouped_by_period() {
    let shelf = |title: &str, rows: Vec<Value>| json!({ "musicShelfRenderer": { "title": runs(&[title]), "contents": rows } });
    let page = section_list(vec![
        shelf(
            "Today",
            vec![
                row("A", ID, "Artist", None, "1:00"),
                row("B", ID2, "Artist", None, "2:00"),
            ],
        ),
        shelf("Empty day", vec![]),
        shelf("Yesterday", vec![row("C", ID, "Artist", None, "3:00")]),
    ]);
    let history = parse_history(&page.to_string());
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].title, "Today");
    assert_eq!(history[0].tracks.len(), 2);
    assert_eq!(history[1].tracks[0].title, "C");
}

#[test]
fn a_podcast_page_has_a_header_and_episodes() {
    let episode = |title: &str, id: &str| {
        json!({ "musicMultiRowListItemRenderer": {
            "title": runs(&[title]),
            "description": runs(&["About it"]),
            "subtitle": runs(&["Jan 5, 2026"]),
            "thumbnail": thumbs("https://img.invalid/e"),
            "onTap": { "watchEndpoint": { "videoId": id } }
        } })
    };
    let page = json!({ "contents": { "twoColumnBrowseResultsRenderer": {
        "tabs": [ { "tabRenderer": { "content": { "sectionListRenderer": { "contents": [
            { "musicResponsiveHeaderRenderer": {
                "title": runs(&["The Show"]),
                "straplineTextOne": runs(&["The Host"]),
                "thumbnail": thumbs("https://img.invalid/s"),
                "description": { "musicDescriptionShelfRenderer": { "description": runs(&["Show text"]) } }
            } }
        ] } } } } ],
        "secondaryContents": { "sectionListRenderer": { "contents": [
            { "musicShelfRenderer": { "contents": [ episode("Ep 1", ID), episode("Ep 2", ID2),
                json!({ "musicMultiRowListItemRenderer": { "title": runs(&["No id"]) } }) ] } }
        ] } }
    } } });
    let id = BrowseId("MPSPsynthetic1".to_owned());
    let detail = parse_podcast(&page.to_string(), &id).expect("podcast");
    assert_eq!(detail.summary.title, "The Show");
    assert_eq!(detail.summary.author.as_deref(), Some("The Host"));
    assert_eq!(detail.summary.id, id);
    assert_eq!(detail.episodes.len(), 2);
    assert_eq!(detail.episodes[0].track.title, "Ep 1");
    assert_eq!(
        detail.episodes[0].track.artists,
        vec!["The Host".to_owned()]
    );
    assert_eq!(detail.episodes[0].published.as_deref(), Some("Jan 5, 2026"));
    assert_eq!(detail.episodes[0].description.as_deref(), Some("About it"));
    assert_eq!(parse_podcast("{}", &id), None);
}
