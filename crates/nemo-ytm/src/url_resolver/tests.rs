//! Tests for the `yt-dlp` page resolver: the answer parser over invented answers shaped like the
//! SoundCloud extractor's, and the argument list.

use std::ffi::OsString;

use nemo_core::audio::Codec;

use super::*;

fn policy() -> FormatPolicy {
    FormatPolicy {
        playable: vec![Codec::Aac, Codec::Mp3],
        allow_hls: true,
    }
}

const HOST: &str = "https://playback.media-streaming.soundcloud.cloud/x";

fn format(id: &str, ext: &str, acodec: &str, abr: Option<f64>, protocol: &str) -> String {
    let abr = abr.map_or("null".to_owned(), |a| a.to_string());
    format!(
        r#"{{"format_id":"{id}","ext":"{ext}","acodec":"{acodec}","abr":{abr},"protocol":"{protocol}","url":"{HOST}/{id}.m3u8"}}"#
    )
}

fn answer(formats: &[String]) -> String {
    format!(
        r#"{{"id":"7000001","title":"Fixture","formats":[{}]}}"#,
        formats.join(",")
    )
}

#[test]
fn the_best_format_is_chosen_like_the_native_resolver_does() {
    let text = answer(&[
        format("hls_aac_96k", "m4a", "mp4a.40.2", Some(96.0), "m3u8_native"),
        format(
            "hls_aac_160k",
            "m4a",
            "mp4a.40.2",
            Some(160.0),
            "m3u8_native",
        ),
        format("http_mp3_0_0", "mp3", "mp3", None, "http"),
    ]);
    let stream = parse_page(&text, &policy()).expect("stream");
    assert_eq!(stream.transport, Transport::Hls { live: false });
    assert_eq!(stream.info.origin, Origin::SoundCloud);
    assert_eq!(
        (stream.info.codec, stream.info.bitrate_kbps),
        (Codec::Aac, Some(160))
    );
    assert!(!stream.preview);
    assert!(stream.url.expose().ends_with("hls_aac_160k.m3u8"));
}

#[test]
fn a_file_beats_hls_of_the_same_codec_and_a_missing_bitrate_stays_unknown() {
    let text = answer(&[
        format("hls_mp3_0_0", "mp3", "mp3", None, "m3u8_native"),
        format("http_mp3_0_0", "mp3", "mp3", None, "http"),
    ]);
    let stream = parse_page(&text, &policy()).expect("stream");
    assert_eq!(stream.transport, Transport::File);
    assert_eq!(stream.info.bitrate_kbps, None);
}

#[test]
fn encrypted_only_and_unplayable_answers_are_not_available_and_nothing_is_picked_around_them() {
    let encrypted = answer(&[format(
        "hls-aes_aac_160k",
        "m4a",
        "mp4a.40.2",
        Some(160.0),
        "m3u8_native",
    )]);
    assert_eq!(
        parse_page(&encrypted, &policy()),
        Err(StreamError::NotAvailable)
    );
    let opus = answer(&[format("hls_opus_0_0", "opus", "opus", None, "m3u8_native")]);
    assert_eq!(
        parse_page(&opus, &policy()),
        Err(StreamError::NotAvailable),
        "Opus is not in this policy"
    );
    assert_eq!(
        parse_page(&answer(&[]), &policy()),
        Err(StreamError::NotAvailable)
    );
}

#[test]
fn a_preview_stays_a_preview_and_ranks_below_a_full_stream() {
    let only = answer(&[format(
        "http_mp3_0_0_preview",
        "mp3",
        "mp3",
        Some(128.0),
        "http",
    )]);
    assert!(parse_page(&only, &policy()).expect("preview").preview);
    let both = answer(&[
        format("http_mp3_0_0_preview", "mp3", "mp3", Some(256.0), "http"),
        format("hls_aac_96k", "m4a", "mp4a.40.2", Some(96.0), "m3u8_native"),
    ]);
    assert!(!parse_page(&both, &policy()).expect("full").preview);
}

#[test]
fn addresses_off_the_soundcloud_hosts_and_plain_http_are_ignored() {
    let bad = |url: &str| {
        format!(
            r#"{{"formats":[{{"format_id":"x","ext":"mp3","acodec":"mp3","abr":128,"protocol":"http","url":"{url}"}}]}}"#
        )
    };
    for url in [
        "https://evil.example/a.mp3",
        "http://cf-media.sndcdn.com/a.mp3",
        "file:///etc/passwd",
    ] {
        assert_eq!(
            parse_page(&bad(url), &policy()),
            Err(StreamError::NotAvailable),
            "{url}"
        );
    }
    assert!(parse_page(&bad("https://cf-media.sndcdn.com/a.mp3"), &policy()).is_ok());
}

#[test]
fn text_that_is_not_json_is_a_parse_error_without_the_text() {
    let err = parse_page("ERROR: secret-token-123", &policy()).expect_err("not json");
    assert!(matches!(err, StreamError::Parse(_)));
    assert!(!err.to_string().contains("secret"));
}

#[test]
fn the_arguments_hold_no_shell_text_and_the_address_comes_after_the_separator() {
    let page = MediaUrl::parse("https://soundcloud.com/fixture-artist/track", HOSTS).expect("page");
    let args = arguments(&page);
    let args: Vec<String> = args
        .iter()
        .map(|a: &OsString| a.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        args.last().map(String::as_str),
        Some("https://soundcloud.com/fixture-artist/track")
    );
    let at = args.iter().position(|a| a == "--").expect("separator");
    assert_eq!(at, args.len() - 2, "only the address follows `--`");
    assert!(args.contains(&"--ignore-config".to_owned()));
    assert!(
        !args
            .iter()
            .any(|a| a.contains("cookie") || a.contains("username") || a.contains("password"))
    );
}

#[test]
fn it_lists_nothing_and_reports_ready() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let resolver = YtDlpUrlResolver::new(YtDlpResolver::new());
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

#[test]
fn a_missing_program_is_resolver_missing() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let resolver = YtDlpUrlResolver::new(YtDlpResolver::with_program("nemo-no-such-yt-dlp-xyz"));
    let page = MediaUrl::parse("https://soundcloud.com/fixture-artist/track", HOSTS).expect("page");
    let p = policy();
    let answer = rt.block_on(resolver.resolve(&page, &p));
    assert_eq!(answer, Err(StreamError::ResolverMissing));
}
