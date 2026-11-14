//! Unit tests for the SponsorBlock source.

use std::sync::Mutex;

use super::*;

fn vid(s: &str) -> VideoId {
    VideoId::parse(s).expect("test id is valid")
}

const ID: &str = "abcdefghijk";
const OTHER: &str = "zzzzzzzzzzz";

#[test]
fn sha256_matches_the_nist_test_vector() {
    // FIPS 180-2 / NIST example: SHA-256("abc").
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    // NIST: SHA-256("") as well.
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn prefix_is_four_hex_chars_of_the_digest() {
    // Digest computed independently with .NET SHA256 over the ASCII id.
    assert_eq!(hash_prefix(&vid(ID)), "ca2f");
}

#[test]
fn url_has_prefix_action_and_categories_only() {
    let url = request_url(&vid(ID), &DEFAULT_CATEGORIES);
    assert_eq!(
        url,
        "https://sponsor.ajay.app/api/skipSegments/ca2f?actionType=skip\
         &category=music_offtopic&category=sponsor&category=selfpromo&category=interaction"
    );
    assert!(!url.contains(ID));
}

#[test]
fn percent_encoding_escapes_reserved_bytes() {
    assert_eq!(percent_encode("a b&c=d/é"), "a%20b%26c%3Dd%2F%C3%A9");
    assert_eq!(percent_encode("az-._~09"), "az-._~09");
}

fn seg(range: &str, cat: &str, action: &str, dur: &str, uuid: &str) -> String {
    format!(
        r#"{{"segment":{range},"UUID":"{uuid}","category":"{cat}","actionType":"{action}","videoDuration":{dur}}}"#
    )
}

fn body(entries: &[(&str, Vec<String>)]) -> String {
    let items: Vec<String> = entries
        .iter()
        .map(|(id, segs)| format!(r#"{{"videoID":"{id}","segments":[{}]}}"#, segs.join(",")))
        .collect();
    format!("[{}]", items.join(","))
}

const REAL: Duration = Duration::from_secs(200);

#[test]
fn filters_sorts_clamps_and_drops() {
    let json = body(&[
        (
            ID,
            vec![
                seg("[100.0, 120.5]", "sponsor", "skip", "200", "u2"),
                seg("[10, 20]", "music_offtopic", "skip", "0", "u1"),
                seg("[190, 999]", "outro", "skip", "201.5", "u3"),
                seg("[30, 40]", "sponsor", "mute", "200", "mute"),
                seg("[30, 40]", "poi_highlight", "skip", "200", "unk"),
                seg("[50, 50]", "sponsor", "skip", "200", "empty"),
                seg("[60, 55]", "sponsor", "skip", "200", "rev"),
                seg("[210, 220]", "sponsor", "skip", "200", "beyond"),
                seg("[70, 80]", "sponsor", "skip", "230", "stale"),
                seg("[-5, 3]", "sponsor", "skip", "200", "neg"),
                seg("[1]", "sponsor", "skip", "200", "short"),
            ],
        ),
        (
            OTHER,
            vec![seg("[1, 2]", "sponsor", "skip", "200", "other")],
        ),
    ]);
    let got = parse_response(&json, &vid(ID), REAL).expect("parses");
    let ids: Vec<&str> = got.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["u1", "u2", "u3"]);
    assert_eq!(got[0].category, SegmentCategory::MusicOffTopic);
    assert_eq!(got[1].end, Duration::from_secs_f64(120.5));
    assert_eq!(got[2].end, REAL);
}

#[test]
fn other_videos_sharing_the_prefix_are_ignored() {
    let json = body(&[(OTHER, vec![seg("[1, 2]", "sponsor", "skip", "200", "x")])]);
    assert_eq!(parse_response(&json, &vid(ID), REAL), Ok(Vec::new()));
}

#[test]
fn garbage_is_a_typed_error() {
    for bad in ["", "nope", "{}", r#"[{"segments":[]}]"#, "[1,2]"] {
        assert!(
            matches!(
                parse_response(bad, &vid(ID), REAL),
                Err(SegmentError::Parse(_))
            ),
            "{bad:?}"
        );
    }
    assert_eq!(parse_response("[]", &vid(ID), REAL), Ok(Vec::new()));
}

struct Fake {
    reply: Result<HttpResponse, String>,
    seen: Mutex<Vec<String>>,
}

impl Fake {
    fn new(status: u16, body: &str) -> Self {
        Self {
            reply: Ok(HttpResponse {
                status,
                body: body.to_owned(),
                retry_after: Some(Duration::from_secs(7)),
            }),
            seen: Mutex::new(Vec::new()),
        }
    }
}

impl HttpGet for Fake {
    fn get<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<HttpResponse, String>> {
        self.seen.lock().expect("lock").push(url.to_owned());
        let reply = self.reply.clone();
        Box::pin(async move { reply })
    }
}

fn run(source: &SponsorBlockSource<Fake>) -> Result<Vec<Segment>, SegmentError> {
    let id = vid(ID);
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime builds")
        .block_on(source.segments(&id, REAL))
}

#[test]
fn source_returns_filtered_segments_and_sends_only_the_prefix() {
    let json = body(&[(ID, vec![seg("[10, 20]", "sponsor", "skip", "200", "u1")])]);
    let source = SponsorBlockSource::new(Fake::new(200, &json));
    let got = run(&source).expect("ok");
    assert_eq!(got.len(), 1);
    let seen = source.http.seen.lock().expect("lock");
    assert_eq!(seen.len(), 1);
    assert!(seen[0].contains("/ca2f?"));
    assert!(!seen[0].contains(ID));
}

#[test]
fn status_404_is_no_segments() {
    let source = SponsorBlockSource::new(Fake::new(404, "Not Found"));
    assert_eq!(run(&source), Ok(Vec::new()));
}

#[test]
fn status_429_is_rate_limited_with_retry_after() {
    let source = SponsorBlockSource::new(Fake::new(429, ""));
    assert_eq!(
        run(&source),
        Err(SegmentError::RateLimited {
            retry_after: Some(Duration::from_secs(7))
        })
    );
}

#[test]
fn other_statuses_and_transport_errors_are_network_errors() {
    let source = SponsorBlockSource::new(Fake::new(500, ""));
    assert!(matches!(run(&source), Err(SegmentError::Network(_))));
    let mut failing = Fake::new(200, "");
    failing.reply = Err("connection refused".to_owned());
    let source = SponsorBlockSource::new(failing);
    assert!(matches!(run(&source), Err(SegmentError::Network(_))));
}

mod props {
    use super::{ID, REAL, vid};
    use crate::sponsorblock::parse_response;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn parse_response_never_panics(s in ".*") {
            let _ = parse_response(&s, &vid(ID), REAL);
        }
    }
}
