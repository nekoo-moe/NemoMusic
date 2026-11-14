//! Unit tests for the catalog mapping and the catalog handle.

use std::time::Duration;

use nemo_core::audio::Origin;
use nemo_core::track::{Artwork, TrackId, VideoId};

use super::*;

const ID: &str = "abcdefghijk";

fn thumb(url: &str, width: u32) -> Thumbnail {
    Thumbnail {
        url: url.to_owned(),
        width,
        height: width,
    }
}

fn song(id: &str, title: &str) -> RawSong {
    RawSong {
        video_id: id.to_owned(),
        title: title.to_owned(),
        artists: vec!["Artist One".to_owned(), "  ".to_owned()],
        album: Some(" Album X ".to_owned()),
        duration: Some("3:45".to_owned()),
        thumbnails: vec![thumb("https://img.invalid/a", 120)],
    }
}

#[test]
fn durations() {
    assert_eq!(parse_duration("3:45"), Some(Duration::from_secs(225)));
    assert_eq!(parse_duration("0:05"), Some(Duration::from_secs(5)));
    assert_eq!(parse_duration("1:02:03"), Some(Duration::from_secs(3723)));
    assert_eq!(parse_duration(" 10:00 "), Some(Duration::from_secs(600)));
    for bad in [
        "", "0:00", "45", "1:2:3:4", "a:bc", "3:", ":30", "1:234", "-1:20", "3:45 min",
    ] {
        assert_eq!(parse_duration(bad), None, "{bad:?}");
    }
}

#[test]
fn years() {
    assert_eq!(parse_year("2019"), Some(2019));
    assert_eq!(parse_year("Album • 2019"), Some(2019));
    assert_eq!(parse_year("1999-2003"), Some(1999));
    assert_eq!(parse_year("12345"), None);
    assert_eq!(parse_year("abc"), None);
    assert_eq!(parse_year(""), None);
}

#[test]
fn thumbnails_drop_empty_urls_and_oversized_numbers() {
    let raw = vec![
        ("https://img.invalid/a".to_owned(), 60, 60),
        ("   ".to_owned(), 10, 10),
        (" https://img.invalid/b ".to_owned(), u64::MAX, 5),
    ];
    let mapped = map_thumbnails(&raw);
    assert_eq!(mapped.len(), 2);
    assert_eq!(mapped[0], thumb("https://img.invalid/a", 60));
    assert_eq!(mapped[1].url, "https://img.invalid/b");
    assert_eq!((mapped[1].width, mapped[1].height), (0, 5));
}

#[test]
fn a_song_maps_to_a_youtube_music_track() {
    let track = map_song(&song(ID, "  A Title ")).expect("valid song");
    assert_eq!(track.id, TrackId(ID.to_owned()));
    assert_eq!(track.title, "A Title");
    assert_eq!(track.artists, vec!["Artist One".to_owned()]);
    assert_eq!(track.album.as_deref(), Some("Album X"));
    assert_eq!(track.duration, Some(Duration::from_secs(225)));
    assert_eq!(track.origin, Origin::YoutubeMusic);
    assert_eq!(track.video_id.as_ref().map(VideoId::as_str), Some(ID));
    assert!(track.local_path.is_none());
    assert_eq!(
        track.artwork,
        Artwork::Remote(vec![thumb("https://img.invalid/a", 120)])
    );
}

#[test]
fn songs_without_a_usable_id_or_title_are_dropped() {
    for bad_id in [
        "",
        "short",
        "abcdefghijkl",
        "abc def ghi",
        "abcdefghij;",
        "abc\ndefghij",
    ] {
        assert_eq!(map_song(&song(bad_id, "T")), None, "{bad_id:?}");
    }
    assert_eq!(map_song(&song(ID, "   ")), None);
}

#[test]
fn missing_optional_parts_stay_missing() {
    let raw = RawSong {
        video_id: ID.to_owned(),
        title: "T".to_owned(),
        ..RawSong::default()
    };
    let track = map_song(&raw).expect("valid");
    assert_eq!(track.artists.len(), 0);
    assert_eq!(track.album, None);
    assert_eq!(track.duration, None);
    assert_eq!(track.artwork, Artwork::None);
}

#[test]
fn albums_artists_and_playlists() {
    let album = RawAlbum {
        id: "MPREb_x".to_owned(),
        title: "Album".to_owned(),
        artists: vec!["A".to_owned()],
        year: Some("Album • 2020".to_owned()),
        thumbnails: vec![thumb("https://img.invalid/c", 226)],
    };
    let mapped = map_album(&album).expect("valid");
    assert_eq!(mapped.id, BrowseId("MPREb_x".to_owned()));
    assert_eq!(mapped.year, Some(2020));
    assert_eq!(
        map_album(&RawAlbum {
            id: " ".to_owned(),
            ..album.clone()
        }),
        None
    );
    assert_eq!(
        map_album(&RawAlbum {
            title: String::new(),
            ..album
        }),
        None
    );

    let artist = RawArtist {
        id: "UCx".to_owned(),
        name: "Name".to_owned(),
        thumbnails: Vec::new(),
    };
    assert_eq!(map_artist(&artist).expect("valid").name, "Name");
    assert_eq!(
        map_artist(&RawArtist {
            name: String::new(),
            ..artist
        }),
        None
    );

    let playlist = RawPlaylist {
        id: "PLx".to_owned(),
        title: "List".to_owned(),
        author: Some("  ".to_owned()),
        thumbnails: Vec::new(),
    };
    let mapped = map_playlist(&playlist).expect("valid");
    assert_eq!(mapped.author, None);
    assert_eq!(
        map_playlist(&RawPlaylist {
            id: String::new(),
            ..playlist
        }),
        None
    );
}

#[test]
fn album_page_tracks_inherit_artists_title_and_cover() {
    let page = RawAlbumPage {
        album: RawAlbum {
            id: "page".to_owned(),
            title: "The Album".to_owned(),
            artists: vec!["Band".to_owned()],
            year: Some("2001".to_owned()),
            thumbnails: vec![thumb("https://img.invalid/d", 544)],
        },
        tracks: vec![
            RawAlbumTrack {
                video_id: ID.to_owned(),
                title: "One".to_owned(),
                duration: Some("2:00".to_owned()),
            },
            RawAlbumTrack {
                video_id: "bad id".to_owned(),
                title: "Two".to_owned(),
                duration: None,
            },
            RawAlbumTrack {
                video_id: "zzzzzzzzzzz".to_owned(),
                title: "Three".to_owned(),
                duration: Some("x".to_owned()),
            },
        ],
    };
    let requested = BrowseId("MPREb_real".to_owned());
    let detail = map_album_page(&page, &requested).expect("valid");
    assert_eq!(detail.summary.id, requested);
    assert_eq!(detail.summary.year, Some(2001));
    assert_eq!(detail.tracks.len(), 2);
    assert_eq!(detail.tracks[0].artists, vec!["Band".to_owned()]);
    assert_eq!(detail.tracks[0].album.as_deref(), Some("The Album"));
    assert_eq!(detail.tracks[0].duration, Some(Duration::from_secs(120)));
    assert_eq!(detail.tracks[1].duration, None);
    assert!(matches!(detail.tracks[1].artwork, Artwork::Remote(_)));
}

#[test]
fn artist_and_playlist_pages_keep_the_requested_id() {
    let requested = BrowseId("UCreal".to_owned());
    let page = RawArtistPage {
        artist: RawArtist {
            id: "page".to_owned(),
            name: "Name".to_owned(),
            thumbnails: Vec::new(),
        },
        description: Some("  ".to_owned()),
        songs: vec![song(ID, "S"), song("bad", "S2")],
        albums: vec![RawAlbum {
            id: "MPREb_a".to_owned(),
            title: "A".to_owned(),
            ..RawAlbum::default()
        }],
    };
    let detail = map_artist_page(&page, &requested).expect("valid");
    assert_eq!(detail.summary.id, requested);
    assert_eq!(detail.description, None);
    assert_eq!(detail.top_songs.len(), 1);
    assert_eq!(detail.albums.len(), 1);

    let requested = BrowseId("VLPLreal".to_owned());
    let page = RawPlaylistPage {
        playlist: RawPlaylist {
            id: "page".to_owned(),
            title: "P".to_owned(),
            ..RawPlaylist::default()
        },
        songs: vec![song(ID, "S")],
    };
    let detail = map_playlist_page(&page, &requested).expect("valid");
    assert_eq!(detail.summary.id, requested);
    assert_eq!(detail.tracks.len(), 1);
    // A page without a title is "not found".
    assert!(map_playlist_page(&RawPlaylistPage::default(), &requested).is_none());
}

#[test]
fn browse_ids_are_checked() {
    for ok in ["MPREb_AbC-123", "UCabc", "VLPLxyz", "RDCLAK5uy_k"] {
        assert!(valid_browse_id(ok), "{ok}");
    }
    for bad in [
        "",
        "a b",
        "a/b",
        "a\"b",
        "x\ny",
        "é",
        &"a".repeat(MAX_ID_BYTES + 1),
    ] {
        assert!(!valid_browse_id(bad), "{bad:?}");
    }
}

/// Search answers are built from JSON, because `ytmapi-rs` result types cannot be built by
/// hand. The shapes below are synthetic.
#[test]
fn search_answer_is_mapped_and_podcasts_are_dropped() {
    let songs = r#"[
        {"title":"Song A","artist":"Band","album":{"name":"Rec","id":"MPREb_1"},
         "duration":"4:01","plays":"1M","explicit":"NotExplicit","video_id":"abcdefghijk",
         "thumbnails":[{"height":60,"width":60,"url":"https://img.invalid/s"}]},
        {"title":"Bad id","artist":"Band","album":null,"duration":"1:00","plays":"1",
         "explicit":"NotExplicit","video_id":"no way","thumbnails":[]}
    ]"#;
    let albums = r#"[
        {"title":"Rec","artist":"Band","year":"2018","explicit":"NotExplicit",
         "album_id":"MPREb_1","album_type":"Album","thumbnails":[]}
    ]"#;
    let artists = r#"[
        {"artist":"Band","subscribers":null,"browse_id":"UCband","thumbnails":[]}
    ]"#;
    let playlists = r#"[
        {"Featured":{"title":"Mix","author":"YouTube Music","songs":"50 songs",
         "playlist_id":"RDCLAK1","thumbnails":[]}},
        {"Community":{"title":"Fan list","author":"Someone","views":"10 views",
         "playlist_id":"PLfan","thumbnails":[]}},
        {"Podcast":{"title":"A show","publisher":"Host","podcast_id":"MPSPPLshow",
         "thumbnails":[]}}
    ]"#;
    let parts = SearchParts {
        songs: serde_json::from_str(songs).expect("synthetic songs parse"),
        albums: serde_json::from_str(albums).expect("synthetic albums parse"),
        artists: serde_json::from_str(artists).expect("synthetic artists parse"),
        playlists: serde_json::from_str(playlists).expect("synthetic playlists parse"),
    };
    let mapped = map_search(&parts);
    assert_eq!(mapped.songs.len(), 1);
    assert_eq!(mapped.songs[0].title, "Song A");
    assert_eq!(mapped.songs[0].album.as_deref(), Some("Rec"));
    assert_eq!(mapped.songs[0].duration, Some(Duration::from_secs(241)));
    assert_eq!(mapped.albums.len(), 1);
    assert_eq!(mapped.albums[0].year, Some(2018));
    assert_eq!(mapped.artists.len(), 1);
    assert_eq!(mapped.artists[0].id, BrowseId("UCband".to_owned()));
    let titles: Vec<&str> = mapped.playlists.iter().map(|p| p.title.as_str()).collect();
    assert_eq!(titles, vec!["Mix", "Fan list"]);
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("runtime builds")
}

#[test]
fn empty_queries_and_bad_ids_make_no_request() {
    // Nothing here can reach the network: all four calls answer before any request.
    let rt = runtime();
    let catalog = YtmCatalog::new(rt.handle().clone());
    let empty = rt.block_on(catalog.search("   ")).expect("empty is fine");
    assert_eq!(empty, SearchResults::default());
    let bad = BrowseId("a b/c".to_owned());
    assert_eq!(
        rt.block_on(catalog.album(&bad)).unwrap_err(),
        CatalogError::NotFound
    );
    assert_eq!(
        rt.block_on(catalog.artist(&bad)).unwrap_err(),
        CatalogError::NotFound
    );
    assert_eq!(
        rt.block_on(catalog.playlist(&bad)).unwrap_err(),
        CatalogError::NotFound
    );
    assert!(matches!(
        rt.block_on(catalog.home()),
        Err(CatalogError::Unavailable(_))
    ));
}

/// Anonymous `ytmapi-rs` search against the real service. Run by hand, at most twice.
#[test]
#[ignore = "needs network access to music.youtube.com"]
fn real_anonymous_search_returns_songs() {
    let rt = runtime();
    let catalog = YtmCatalog::new(rt.handle().clone());
    let results = rt
        .block_on(catalog.search("piano"))
        .expect("anonymous search works");
    assert_ne!(results.songs.len(), 0);
    assert!(results.songs.iter().all(|t| t.video_id.is_some()));
}
