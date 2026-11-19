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

mod ciphered {
    //! Formats whose address carries a signature cipher and an `n` value.

    use std::collections::HashMap;

    use super::*;
    use crate::cipher::Solved;

    fn answer_with(entry: &Value) -> String {
        answer(&json!([entry]))
    }

    fn ciphered_opus() -> Value {
        json!({
            "mimeType": "audio/webm; codecs=\"opus\"",
            "bitrate": 130_000,
            "signatureCipher": "s=AB%3DC&sp=sig&url=https%3A%2F%2Fr1.example.invalid%2Fvideoplayback%3Fexpire%3D1900000000%26n%3Dabc%26itag%3D251",
        })
    }

    fn solved(n: &str, sig: &str) -> Solved {
        Solved {
            n: HashMap::from([("abc".to_owned(), n.to_owned())]),
            sig: HashMap::from([("AB=C".to_owned(), sig.to_owned())]),
            preprocessed: None,
        }
    }

    #[test]
    fn the_plain_reader_leaves_a_ciphered_format_alone_and_the_other_reads_it() {
        let text = answer_with(&ciphered_opus());
        assert!(parse_player(&text, None).is_err());
        let pending = parse_player_ciphered(&text, None).expect("ciphered format");
        let asked = pending.challenges();
        assert_eq!(asked.n, ["abc"]);
        assert_eq!(asked.sig, ["AB=C"], "the cipher is decoded");
    }

    #[test]
    fn the_solved_signature_is_added_under_its_own_name_and_n_is_replaced() {
        let pending = parse_player_ciphered(&answer_with(&ciphered_opus()), None).expect("pending");
        let stream = pending.finish(&solved("xyz", "SIG123")).expect("finished");
        assert_eq!(
            stream.url.expose(),
            "https://r1.example.invalid/videoplayback?expire=1900000000&n=xyz&itag=251&sig=SIG123"
        );
        assert!(stream.expires_at.is_some());
        assert_eq!(stream.info.codec, Codec::Opus);
    }

    #[test]
    fn an_answer_the_solver_did_not_give_or_a_signature_that_breaks_the_address_is_refused() {
        let pending = parse_player_ciphered(&answer_with(&ciphered_opus()), None).expect("pending");
        assert!(pending.finish(&Solved::default()).is_err());
        for bad in ["a&b", "a#b", "a b", ""] {
            assert!(pending.finish(&solved("xyz", bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_protected_format_is_never_taken() {
        let mut protected = ciphered_opus();
        protected["drmFamilies"] = json!(["WIDEVINE"]);
        assert!(parse_player_ciphered(&answer_with(&protected), None).is_err());
    }

    #[test]
    fn a_cipher_with_no_address_or_no_signature_is_skipped() {
        for cipher in [
            "s=AB&sp=sig",
            "url=https%3A%2F%2Fr1.example.invalid%2Fv%3Fn%3Da",
            "s=AB&url=http%3A%2F%2Fr1.example.invalid%2Fv",
        ] {
            let entry = json!({"mimeType": "audio/webm; codecs=\"opus\"", "bitrate": 1, "signatureCipher": cipher});
            assert!(
                parse_player_ciphered(&answer_with(&entry), None).is_err(),
                "{cipher}"
            );
        }
    }

    #[test]
    fn the_tv_body_names_the_player_by_its_timestamp_and_sends_no_empty_device_fields() {
        let body = player_body_with(&TV, &vid(), Some(20_773));
        assert_eq!(
            body["playbackContext"]["contentPlaybackContext"]["signatureTimestamp"],
            20_773
        );
        let client = &body["context"]["client"];
        assert_eq!(client["clientName"], "TVHTML5");
        for key in ["deviceMake", "deviceModel", "osName", "osVersion"] {
            assert!(client.get(key).is_none(), "{key}");
        }
        let samsung = player_body_with(&TV_SAMSUNG, &vid(), None);
        assert_eq!(samsung["context"]["client"]["deviceMake"], "Samsung");
        assert!(
            samsung["playbackContext"]["contentPlaybackContext"]
                .get("signatureTimestamp")
                .is_none()
        );
        assert!(!body.to_string().to_ascii_lowercase().contains("cookie"));
    }
}
