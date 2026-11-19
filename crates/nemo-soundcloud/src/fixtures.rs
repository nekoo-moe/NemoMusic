//! The recorded and rewritten JSON answers under `tests/fixtures/`, and the lint that keeps them
//! free of anything real.

use nemo_core::track::MediaUrl;

use crate::hosts::HOSTS;

pub const TRACK_HLS: &str = include_str!("../tests/fixtures/track_hls.json");
pub const SEARCH_TRACKS: &str = include_str!("../tests/fixtures/search_tracks.json");
pub const SEARCH_MIXED: &str = include_str!("../tests/fixtures/search_mixed.json");
pub const SEARCH_USERS: &str = include_str!("../tests/fixtures/search_users.json");
pub const PLAYLIST_WITH_STUBS: &str = include_str!("../tests/fixtures/playlist_with_stubs.json");
pub const TRACKS_BATCH: &str = include_str!("../tests/fixtures/tracks_batch.json");
pub const USER: &str = include_str!("../tests/fixtures/user.json");
pub const USER_TRACKS: &str = include_str!("../tests/fixtures/user_tracks.json");
pub const USER_ALBUMS: &str = include_str!("../tests/fixtures/user_albums.json");
pub const RELATED: &str = include_str!("../tests/fixtures/related.json");
pub const ME: &str = include_str!("../tests/fixtures/me.json");

const ALL: [&str; 11] = [
    TRACK_HLS,
    SEARCH_TRACKS,
    SEARCH_MIXED,
    SEARCH_USERS,
    PLAYLIST_WITH_STUBS,
    TRACKS_BATCH,
    USER,
    USER_TRACKS,
    USER_ALBUMS,
    RELATED,
    ME,
];

/// Every `https://host` in `text`.
fn hosts_in(text: &str) -> Vec<String> {
    text.split("https://")
        .skip(1)
        .map(|rest| {
            rest.split(['/', '"', '?', ' '])
                .next()
                .unwrap_or_default()
                .to_owned()
        })
        .collect()
}

#[test]
fn every_fixture_is_json() {
    for text in ALL {
        serde_json::from_str::<serde_json::Value>(text).expect("valid json");
    }
}

#[test]
fn fixtures_hold_no_credential_and_no_foreign_host() {
    for text in ALL {
        let lower = text.to_ascii_lowercase();
        for word in [
            "client_id",
            "oauth",
            "signature",
            "key-pair-id",
            "policy=",
            "cookie",
            "password",
            "first_name",
            "date_of_birth",
            "email",
        ] {
            assert!(!lower.contains(word), "a fixture holds `{word}`");
        }
        for host in hosts_in(text) {
            assert!(
                MediaUrl::parse(&format!("https://{host}/"), HOSTS).is_ok(),
                "a fixture names a host that is not in HOSTS"
            );
        }
    }
}

#[test]
fn fixture_ids_are_invented() {
    // Invented ids start at 7,000,001; a real id is far larger.
    for text in ALL {
        for token in text
            .split(|c: char| !c.is_ascii_digit())
            .filter(|t| t.len() >= 8)
        {
            assert!(
                token.starts_with("7000") || token.starts_with("0000"),
                "a fixture holds a long number that looks real"
            );
        }
    }
}
