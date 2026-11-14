//! Table tests for the import score and confidence.

use nemo_core::audio::Origin;
use nemo_core::track::{Artwork, TrackId};

use super::*;

fn song(title: &str, artist: &str, secs: Option<u64>) -> ImportedSong {
    ImportedSong {
        title: title.to_owned(),
        artists: vec![artist.to_owned()],
        album: None,
        duration: secs.map(Duration::from_secs),
        isrc: None,
        line: 2,
    }
}

fn track(title: &str, artist: &str, secs: Option<u64>) -> Track {
    Track {
        id: TrackId("ytm:synthetic".to_owned()),
        title: title.to_owned(),
        artists: vec![artist.to_owned()],
        album: None,
        duration: secs.map(Duration::from_secs),
        origin: Origin::YoutubeMusic,
        video_id: None,
        local_path: None,
        artwork: Artwork::None,
    }
}

#[test]
fn scores_and_confidence_for_the_table_cases() {
    let cases = [
        // (imported, candidate, score, confidence)
        (
            song("Song (feat. X)", "Artist", Some(200)),
            track("Song", "Artist", Some(201)),
            100,
            Confidence::High,
        ),
        (
            song("Song - Remastered 2011", "Artist", Some(200)),
            track("Song", "Artist", Some(200)),
            100,
            Confidence::High,
        ),
        (
            song("Song", "Artist", Some(200)),
            track("Song", "Other Band", Some(200)),
            70,
            Confidence::Medium,
        ),
        (
            song("Song", "Artist", Some(200)),
            track("Song", "Artist", Some(220)),
            80,
            Confidence::Medium,
        ),
        (
            song("Song", "Artist", None),
            track("Song", "Artist", Some(200)),
            90,
            Confidence::High,
        ),
        (
            song("Night Drive", "Artist", Some(200)),
            track("Night Swim", "Someone Else", Some(200)),
            45,
            Confidence::Low,
        ),
        (
            song("Song", "Artist", Some(200)),
            track("Other Tune", "Someone Else", Some(400)),
            0,
            Confidence::None,
        ),
    ];
    for (imported, found, want_score, want_confidence) in cases {
        let got = candidate(&imported, found);
        assert_eq!(
            (got.score, got.confidence),
            (want_score, want_confidence),
            "{} vs {}",
            imported.title,
            got.track.title
        );
    }
}

#[test]
fn high_needs_the_durations_within_five_seconds() {
    let imported = song("Song", "Artist", Some(200));
    let near = candidate(&imported, track("Song", "Artist", Some(204)));
    assert_eq!((near.score, near.confidence), (98, Confidence::High));
    let far = candidate(&imported, track("Song", "Artist", Some(206)));
    assert_eq!((far.score, far.confidence), (95, Confidence::Medium));
}

#[test]
fn duration_score_falls_from_three_to_fifteen_seconds() {
    let at = |secs| {
        duration_score(
            Some(Duration::from_secs(100)),
            Some(Duration::from_secs(secs)),
        )
    };
    assert_eq!(at(100), 100);
    assert_eq!(at(97), 100);
    assert_eq!(at(109), 50);
    assert_eq!(at(115), 0);
    assert_eq!(at(150), 0);
    assert_eq!(duration_score(None, Some(Duration::from_secs(1))), 50);
}

#[test]
fn similarity_is_the_share_of_words_in_common() {
    assert_eq!(similarity("Night Drive", "night drive"), 100);
    assert_eq!(similarity("Night Drive", "Night"), 50);
    assert_eq!(similarity("", ""), 0);
    assert_eq!(similarity("Song (Live)", "Song"), 100);
}
