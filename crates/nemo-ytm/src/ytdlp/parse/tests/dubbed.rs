//! The audio-track choice on a video with dubbed tracks (account-actions criteria 24 to 27).
//!
//! `dubbed.json` is hand-made in the shape of `yt-dlp` output for such a video: per-track
//! `language`, `language_preference` 10 on the original track, the display name first in
//! `format_note`, a described-audio track and a muxed video format. No real URL or id.

use nemo_core::audio::Codec;

use super::super::{parse, parse_for, parse_with};

const DUBBED: &str = include_str!("dubbed.json");
const AAC: &[Codec] = &[Codec::Aac];

#[test]
fn a_matching_language_plays_that_track() {
    let r = parse_for(DUBBED, Some(AAC), Some("ja")).expect("parses");
    assert_eq!(r.audio_language.as_deref(), Some("ja"));
    assert!(
        r.url.expose().contains("ja-medium"),
        "the best Japanese format"
    );
    assert_eq!(r.info.bitrate_kbps, Some(130));
    // A region tag in the request still finds the track by its language.
    let r = parse_for(DUBBED, Some(AAC), Some("pt")).expect("parses");
    assert_eq!(r.audio_language.as_deref(), Some("pt-BR"));
}

#[test]
fn no_match_plays_the_original_track_and_says_which() {
    let r = parse_for(DUBBED, Some(AAC), Some("de")).expect("parses");
    assert_eq!(r.audio_language.as_deref(), Some("en"));
    assert!(r.url.expose().contains("en-medium"));
}

#[test]
fn automatic_plays_the_original_track_not_the_highest_bitrate() {
    // The described-audio and Portuguese formats have a higher bitrate than the original.
    let r = parse_with(DUBBED, Some(AAC)).expect("parses");
    assert_eq!(r.audio_language.as_deref(), Some("en"));
    assert!(r.url.expose().contains("en-medium"));
    assert_eq!(parse_for(DUBBED, Some(AAC), None), Ok(r));
    // Every codec allowed: the original track's best format, here Opus.
    let all = parse(DUBBED).expect("parses");
    assert_eq!(all.info.codec, Codec::Opus);
    assert_eq!(all.audio_language.as_deref(), Some("en"));
}

#[test]
fn described_audio_is_listed_but_never_matched_by_language() {
    let r = parse_for(DUBBED, Some(AAC), Some("en")).expect("parses");
    assert!(
        r.url.expose().contains("en-medium"),
        "not the en-desc track"
    );
}

#[test]
fn the_answer_lists_every_track_original_first() {
    let r = parse_with(DUBBED, Some(AAC)).expect("parses");
    let tracks: Vec<(Option<&str>, &str, bool)> = r
        .audio_tracks
        .iter()
        .map(|t| (t.language.as_deref(), t.label.as_str(), t.is_default))
        .collect();
    assert_eq!(
        tracks,
        [
            (Some("en"), "English original", true),
            (Some("ja"), "Japanese", false),
            (Some("pt-BR"), "Portuguese (Brazil)", false),
            (Some("en-desc"), "English descriptive", false),
        ]
    );
}

#[test]
fn a_single_track_video_lists_no_tracks() {
    let json = r#"{"formats":[{"vcodec":"none","acodec":"mp4a.40.2","abr":128,"language":"en","url":"https://x.example.invalid/a"}]}"#;
    let r = parse_for(json, None, Some("ja")).expect("parses");
    assert!(r.audio_tracks.is_empty());
    assert_eq!(r.audio_language.as_deref(), Some("en"));
    let bare = r#"{"formats":[{"vcodec":"none","acodec":"opus","abr":160,"url":"https://x.example.invalid/b"}]}"#;
    let r = parse_for(bare, None, Some("ja")).expect("parses");
    assert!(r.audio_tracks.is_empty() && r.audio_language.is_none());
}
