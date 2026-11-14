//! Unit tests for the `yt-dlp` output parsing.

use super::*;

const URL_A: &str = "https://a.example.invalid/audio?expire=1900000000&sig=FAKE";
const URL_B: &str = "https://b.example.invalid/audio?sig=FAKE2";

fn fmt(id: &str, vcodec: &str, acodec: &str, abr: &str, url: &str) -> String {
    format!(
        r#"{{"format_id":"{id}","vcodec":"{vcodec}","acodec":"{acodec}",{abr}"asr":48000,"audio_channels":2,"url":"{url}"}}"#
    )
}

fn doc(formats: &[String]) -> String {
    format!(
        r#"{{"id":"fakeid00000","formats":[{}]}}"#,
        formats.join(",")
    )
}

#[test]
fn best_abr_wins_and_maps_opus() {
    let json = doc(&[
        fmt("lo", "none", "mp4a.40.2", r#""abr":128.5,"#, URL_B),
        fmt("hi", "none", "opus", r#""abr":160.0,"#, URL_A),
    ]);
    let r = parse(&json).expect("parses");
    assert_eq!(r.info.codec, Codec::Opus);
    assert_eq!(r.info.bitrate_kbps, Some(160));
    assert_eq!(r.info.origin, Origin::YoutubeMusic);
    assert_eq!(
        r.info.quality_class(),
        nemo_core::audio::QualityClass::Lossy
    );
    assert_eq!(r.url.expose(), URL_A);
    assert_eq!(
        r.info.format,
        Some(AudioFormat {
            sample_rate: 48_000,
            channels: 2,
            bit_depth: None
        })
    );
    assert_eq!(
        r.expires_at,
        Some(UNIX_EPOCH + Duration::from_secs(1_900_000_000))
    );
}

#[test]
fn aac_and_unknown_codecs() {
    let aac = parse(&doc(&[fmt(
        "a",
        "none",
        "mp4a.40.2",
        r#""abr":128.0,"#,
        URL_B,
    )]))
    .expect("parses");
    assert_eq!(aac.info.codec, Codec::Aac);
    assert_eq!(aac.expires_at, None);
    let other = parse(&doc(&[fmt(
        "a",
        "none",
        "vorbis",
        r#""abr":128.0,"#,
        URL_B,
    )]))
    .expect("parses");
    assert_eq!(other.info.codec, Codec::Unknown);
}

#[test]
fn missing_abr_ranks_last_but_is_usable() {
    let only = parse(&doc(&[fmt("a", "none", "opus", "", URL_B)])).expect("parses");
    assert_eq!(only.info.bitrate_kbps, None);
    let both = parse(&doc(&[
        fmt("a", "none", "opus", "", URL_A),
        fmt("b", "none", "mp4a.40.2", r#""abr":48.0,"#, URL_B),
    ]))
    .expect("parses");
    assert_eq!(both.info.codec, Codec::Aac);
}

#[test]
fn video_only_and_empty_lists_are_not_available() {
    let video = doc(&[
        fmt("v", "avc1.4d401f", "none", "", URL_A),
        fmt("m", "avc1.4d401f", "mp4a.40.2", r#""abr":128.0,"#, URL_B),
    ]);
    assert_eq!(parse(&video).unwrap_err(), StreamError::NotAvailable);
    assert_eq!(parse(&doc(&[])).unwrap_err(), StreamError::NotAvailable);
    let no_acodec = doc(&[fmt("a", "none", "none", r#""abr":1.0,"#, URL_A)]);
    assert_eq!(parse(&no_acodec).unwrap_err(), StreamError::NotAvailable);
}

#[test]
fn garbage_gives_typed_parse_errors() {
    for bad in [
        "",
        "not json",
        "[]",
        "null",
        "{}",
        r#"{"formats":"x"}"#,
        r#"{"formats":[1,null,"a",{"vcodec":5}]}"#,
    ] {
        let r = parse(bad);
        assert!(
            matches!(r, Err(StreamError::Parse(_) | StreamError::NotAvailable)),
            "{bad:?}"
        );
    }
    assert!(matches!(parse("not json"), Err(StreamError::Parse(_))));
    assert!(matches!(parse("{}"), Err(StreamError::Parse(_))));
}

#[test]
fn debug_output_hides_the_url() {
    let r = parse(&doc(&[fmt("a", "none", "opus", r#""abr":160.0,"#, URL_A)])).expect("ok");
    let shown = format!("{r:?}");
    assert!(!shown.contains("example.invalid"));
    assert!(!shown.contains("sig=FAKE"));
    assert!(shown.contains("[redacted]"));
}

#[test]
fn allowed_codecs_filter_the_choice() {
    let json = doc(&[
        fmt("opus", "none", "opus", r#""abr":160.0,"#, URL_A),
        fmt("aac", "none", "mp4a.40.2", r#""abr":128.0,"#, URL_B),
    ]);
    let aac_only = parse_with(&json, Some(&[Codec::Aac])).expect("parses");
    assert_eq!(aac_only.info.codec, Codec::Aac);
    assert_eq!(aac_only.url.expose(), URL_B);
    let both = parse_with(&json, Some(&[Codec::Aac, Codec::Opus])).expect("parses");
    assert_eq!(both.info.codec, Codec::Opus);
    assert_eq!(
        parse_with(&json, None).expect("parses").info.codec,
        Codec::Opus
    );
    assert_eq!(
        parse_with(&json, Some(&[Codec::Flac])).unwrap_err(),
        StreamError::NotAvailable
    );
    assert_eq!(
        parse_with(&json, Some(&[])).unwrap_err(),
        StreamError::NotAvailable
    );
    // An unknown codec is accepted only when the list names `Unknown`.
    let vorbis = doc(&[fmt("v", "none", "vorbis", r#""abr":96.0,"#, URL_A)]);
    assert!(parse_with(&vorbis, Some(&[Codec::Opus])).is_err());
    assert!(parse_with(&vorbis, Some(&[Codec::Unknown])).is_ok());
}

#[test]
fn stderr_is_mapped_to_typed_errors() {
    let not_available = [
        "ERROR: [youtube] abc: Video unavailable",
        "ERROR: [youtube] abc: Private video. Sign in if you've been granted access",
        "ERROR: [youtube] abc: Sign in to confirm your age. This video may be inappropriate",
        "ERROR: The uploader has not made this video available in your country",
        "ERROR: [youtube] abc: This video is available to Music Premium members only",
        "ERROR: This video is not available",
    ];
    for text in not_available {
        assert_eq!(classify_stderr(text), StreamError::NotAvailable, "{text}");
    }
    let network = [
        "ERROR: [youtube] abc: Unable to download webpage: <urlopen error [Errno 11001] getaddrinfo failed>",
        "ERROR: Unable to download JSON metadata: HTTP Error 503",
        "ERROR: <urlopen error timed out>",
        "ERROR: [Errno 10061] Connection refused",
        "ERROR: HTTP Error 429: Too Many Requests",
        "",
        "some text nobody has seen before",
    ];
    for text in network {
        assert!(
            matches!(classify_stderr(text), StreamError::Network(_)),
            "{text}"
        );
    }
    let parse = [
        "ERROR: [youtube] abc: Unable to extract initial player response",
        "ERROR: Unsupported URL: https://example.invalid/",
    ];
    for text in parse {
        assert!(
            matches!(classify_stderr(text), StreamError::Parse(_)),
            "{text}"
        );
    }
}

#[test]
fn mapped_errors_never_echo_stderr() {
    let secret = "ERROR: Unable to download https://host.invalid/v?sig=SECRETSIG timed out";
    let shown = format!("{:?}", classify_stderr(secret));
    assert!(!shown.contains("SECRETSIG") && !shown.contains("host.invalid"));
}

mod props {
    use super::super::parse;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn parse_never_panics(s in ".*") {
            let _ = parse(&s);
        }
    }
}
