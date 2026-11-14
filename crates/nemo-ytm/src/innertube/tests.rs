//! Unit tests for the innertube resolver and player parsing.

use super::*;
use nemo_core::audio::AudioFormat;

fn vid() -> VideoId {
    VideoId::parse("abcdefghijk").expect("valid")
}

const URL_AAC: &str = "https://r1.example.invalid/videoplayback?expire=1900000000&itag=140";
const URL_OPUS: &str = "https://r1.example.invalid/videoplayback?itag=251";

fn answer(formats: &Value) -> String {
    json!({
        "playabilityStatus": { "status": "OK" },
        "streamingData": { "adaptiveFormats": formats }
    })
    .to_string()
}

#[test]
fn visitor_data_is_taken_only_when_it_looks_like_one() {
    let ok = json!({ "responseContext": { "visitorData": "Cgt4eXo%3D_-" } });
    assert_eq!(visitor_of(&ok).as_deref(), Some("Cgt4eXo%3D_-"));
    for bad in [
        json!({}),
        json!({ "responseContext": { "visitorData": "a b" } }),
        json!({ "responseContext": { "visitorData": "" } }),
    ] {
        assert_eq!(visitor_of(&bad), None);
    }
    let line = describe_player(
        &json!({ "playabilityStatus": { "status": "LOGIN_REQUIRED", "reason": "bot" } })
            .to_string(),
    );
    assert!(
        line.contains("LOGIN_REQUIRED") && line.contains("0 plain"),
        "{line}"
    );
}

#[test]
fn body_matches_the_yt_dlp_shape() {
    let body = player_body(&ANDROID_VR, &vid());
    assert_eq!(body["videoId"], "abcdefghijk");
    assert_eq!(body["context"]["client"]["clientName"], "ANDROID_VR");
    assert_eq!(body["context"]["client"]["androidSdkVersion"], 32);
    assert_eq!(body["contentCheckOk"], true);
    assert_eq!(body["racyCheckOk"], true);
    assert_eq!(
        body["playbackContext"]["contentPlaybackContext"]["html5Preference"],
        "HTML5_PREF_WANTS"
    );
    let vision = player_body(&VISIONOS, &vid());
    assert!(
        vision["context"]["client"]
            .get("androidSdkVersion")
            .is_none()
    );
    assert!(!body.to_string().to_ascii_lowercase().contains("cookie"));
}

#[test]
fn best_aac_with_plain_url_is_picked() {
    let json = answer(&json!([
        { "itag": 137, "mimeType": "video/mp4; codecs=\"avc1\"", "bitrate": 4_000_000, "url": URL_AAC },
        { "itag": 251, "mimeType": "audio/webm; codecs=\"opus\"", "bitrate": 160_000, "url": URL_OPUS },
        { "itag": 140, "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "bitrate": 130_000,
          "averageBitrate": 129_500, "audioSampleRate": "44100", "audioChannels": 2, "url": URL_AAC },
        { "itag": 141, "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "bitrate": 260_000,
          "signatureCipher": "s=x&url=y" },
        { "itag": 140, "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "bitrate": 140_000,
          "isDrc": true, "url": URL_OPUS }
    ]));
    let s = parse_player(&json, Some(&[Codec::Aac])).expect("parses");
    assert_eq!(s.url.expose(), URL_AAC);
    assert_eq!(s.info.codec, Codec::Aac);
    assert_eq!(s.info.bitrate_kbps, Some(130));
    assert_eq!(
        s.info.format,
        Some(AudioFormat {
            sample_rate: 44_100,
            channels: 2,
            bit_depth: None
        })
    );
    assert!(s.expires_at.is_some());
    assert!(!format!("{s:?}").contains("example.invalid"));
    let any = parse_player(&json, None).expect("parses");
    assert_eq!(any.info.codec, Codec::Opus);
}

#[test]
fn unplayable_and_garbage() {
    let login = json!({ "playabilityStatus": { "status": "LOGIN_REQUIRED" } }).to_string();
    assert_eq!(
        parse_player(&login, None).unwrap_err(),
        StreamError::NotAvailable
    );
    let no_audio = answer(&json!([{ "mimeType": "video/mp4", "url": URL_AAC }]));
    assert_eq!(
        parse_player(&no_audio, None).unwrap_err(),
        StreamError::NotAvailable
    );
    let http = answer(
        &json!([{ "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "url": "http://x.invalid/a" }]),
    );
    assert_eq!(
        parse_player(&http, None).unwrap_err(),
        StreamError::NotAvailable
    );
    for bad in ["", "null", "{}", "[1]", "{\"playabilityStatus\":5}"] {
        assert!(parse_player(bad, None).is_err(), "{bad:?}");
    }
    let ok_no_data = json!({ "playabilityStatus": { "status": "OK" } }).to_string();
    assert_eq!(
        parse_player(&ok_no_data, None).unwrap_err(),
        StreamError::NotAvailable
    );
}

mod props {
    use super::super::parse_player;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn never_panics(s in ".*") {
            let _ = parse_player(&s, None);
        }
    }
}
