//! Tests of the in-process solver tier. Fakes only, except the `#[ignore]` vector test, which
//! downloads public player scripts (human-paced) and checks the solver against the test vectors
//! of `yt-dlp/ejs` (`src/yt/solver/test/tests.ts`, Unlicense).

mod resolver;
mod solver;
mod store;
mod vectors;

use super::player::{player_id_in, sts_in};

#[test]
fn the_player_id_is_read_from_iframe_api_like_yt_dlp_does() {
    let text = r#"var scriptUrl = 'https:\/\/www.youtube.com\/s\/player\/983e9c18\/www-widgetapi.vflset\/www-widgetapi.js';"#;
    assert_eq!(player_id_in(text).as_deref(), Some("983e9c18"));
    assert_eq!(
        player_id_in("player/ABCDEF12/x").as_deref(),
        Some("abcdef12")
    );
    assert_eq!(player_id_in("player/zzzzzzzz/x"), None);
    assert_eq!(player_id_in("player/983e9c1/x"), None);
    assert_eq!(player_id_in("no id here"), None);
}

#[test]
fn the_signature_timestamp_is_five_digits_after_a_known_key() {
    assert_eq!(sts_in("a,signatureTimestamp:20522,b"), Some(20522));
    assert_eq!(sts_in("a,sts : 20523 ,b"), Some(20523));
    assert_eq!(sts_in("sts:1234,signatureTimestamp:20524"), Some(20524));
    assert_eq!(sts_in("signatureTimestamp:123456"), None);
    assert_eq!(sts_in("nothing"), None);
}
