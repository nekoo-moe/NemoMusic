//! Tests for the playlist reader.

use std::fmt::Write as _;

use super::*;

#[allow(clippy::unnecessary_wraps)]
fn same(url: &str) -> Option<String> {
    Some(format!("https://cdn.test/{url}"))
}

const VOD: &str = "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-TARGETDURATION:10\n#EXT-X-MEDIA-SEQUENCE:0\n\
#EXT-X-PLAYLIST-TYPE:VOD\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:10.007800,\nseg0.m4s\n\
#EXTINF:9.984580,\nseg1.m4s\n#EXTINF:3.5,\nseg2.m4s\n#EXT-X-ENDLIST\n";

#[test]
fn a_vod_playlist_gives_the_init_and_the_segments_in_order() {
    let list = parse(VOD, &same).expect("playlist");
    assert_eq!(list.init.as_deref(), Some("https://cdn.test/init.mp4"));
    let urls: Vec<&str> = list.segments.iter().map(|s| s.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "https://cdn.test/seg0.m4s",
            "https://cdn.test/seg1.m4s",
            "https://cdn.test/seg2.m4s"
        ]
    );
    assert_eq!(list.segments[1].duration, Duration::from_micros(9_984_580));
    assert_eq!(list.duration(), Duration::from_micros(23_492_380));
}

#[test]
fn windows_line_endings_and_a_missing_map_are_fine() {
    let text = "#EXTM3U\r\n#EXTINF:2.0,\r\na.mp3\r\n#EXT-X-ENDLIST\r\n";
    let list = parse(text, &same).expect("playlist");
    assert_eq!(list.init, None);
    assert_eq!(list.segments.len(), 1);
}

#[test]
fn a_key_other_than_none_is_refused_before_anything_else() {
    let aes = VOD.replace(
        "#EXT-X-MAP",
        "#EXT-X-KEY:METHOD=AES-128,URI=\"k\"\n#EXT-X-MAP",
    );
    assert_eq!(parse(&aes, &same), Err(PlaylistError::Encrypted));
    let sample = VOD.replace(
        "#EXT-X-MAP",
        "#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"skd://x\"\n#EXT-X-MAP",
    );
    assert_eq!(parse(&sample, &same), Err(PlaylistError::Encrypted));
    let none = VOD.replace("#EXT-X-MAP", "#EXT-X-KEY:METHOD=NONE\n#EXT-X-MAP");
    assert!(parse(&none, &same).is_ok());
}

#[test]
fn master_live_and_foreign_text_are_not_media_playlists() {
    let master = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\nlow.m3u8\n";
    assert_eq!(parse(master, &same), Err(PlaylistError::Unsupported));
    let live = VOD.replace("#EXT-X-ENDLIST\n", "");
    assert_eq!(parse(&live, &same), Err(PlaylistError::Unsupported));
    assert_eq!(parse("<html>", &same), Err(PlaylistError::Unsupported));
    assert_eq!(parse("", &same), Err(PlaylistError::Unsupported));
}

#[test]
fn bad_lines_and_refused_addresses_are_malformed() {
    let no_extinf = "#EXTM3U\nseg.m4s\n#EXT-X-ENDLIST\n";
    assert_eq!(parse(no_extinf, &same), Err(PlaylistError::Malformed));
    for bad in ["x", "-1", "nan", "inf", "99999"] {
        let text = format!("#EXTM3U\n#EXTINF:{bad},\ns.m4s\n#EXT-X-ENDLIST\n");
        assert_eq!(parse(&text, &same), Err(PlaylistError::Malformed), "{bad}");
    }
    let refuse = |_: &str| None;
    assert_eq!(parse(VOD, &refuse), Err(PlaylistError::Malformed));
    let no_uri = VOD.replace("URI=\"init.mp4\"", "BYTERANGE=\"1@0\"");
    assert_eq!(parse(&no_uri, &same), Err(PlaylistError::Malformed));
}

#[test]
fn an_empty_list_and_two_init_segments_are_refused() {
    assert_eq!(
        parse("#EXTM3U\n#EXT-X-ENDLIST\n", &same),
        Err(PlaylistError::Empty)
    );
    let two = VOD.replace("#EXTINF:3.5", "#EXT-X-MAP:URI=\"other.mp4\"\n#EXTINF:3.5");
    assert_eq!(parse(&two, &same), Err(PlaylistError::Unsupported));
}

#[test]
fn too_many_segments_are_refused() {
    let mut text = String::from("#EXTM3U\n");
    for n in 0..=MAX_SEGMENTS {
        let _ = writeln!(text, "#EXTINF:1.0,\ns{n}.m4s");
    }
    text.push_str("#EXT-X-ENDLIST\n");
    assert_eq!(parse(&text, &same), Err(PlaylistError::Unsupported));
}

#[test]
fn errors_name_no_address() {
    for e in [
        PlaylistError::Encrypted,
        PlaylistError::Unsupported,
        PlaylistError::Malformed,
        PlaylistError::Empty,
    ] {
        assert!(!e.to_string().contains("http"));
    }
}
