use super::*;
use nemo_core::lyrics::{LyricLine, ProviderId};

fn doc(starts: &[Option<u64>], granularity: Granularity) -> LyricsDoc {
    LyricsDoc {
        lines: starts
            .iter()
            .map(|s| LyricLine {
                start_ms: *s,
                end_ms: None,
                text: "la la test line".into(),
                words: Vec::new(),
            })
            .collect(),
        granularity,
        provider: ProviderId::new("Mock"),
        matched: None,
        instrumental: false,
    }
}

#[test]
fn line_changes_615_ms_early() {
    // Scroll clock: position + 115 ms line sync offset + 500 ms scroll lead.
    let d = doc(&[Some(1000), Some(5000), Some(9000)], Granularity::Line);
    assert_eq!(active_line(&d, 0, 0), None);
    assert_eq!(active_line(&d, 384, 0), None);
    assert_eq!(active_line(&d, 385, 0), Some(0));
    assert_eq!(active_line(&d, 4384, 0), Some(0));
    assert_eq!(active_line(&d, 4385, 0), Some(1));
    assert_eq!(active_line(&d, 99_000, 0), Some(2));
    let w = doc(&[Some(1000)], Granularity::Word);
    assert_eq!(active_line(&w, 349, 0), None, "word sync offset is 150 ms");
    assert_eq!(active_line(&w, 350, 0), Some(0));
}

#[test]
fn offset_shifts_the_active_line() {
    let d = doc(&[Some(1000), Some(5000)], Granularity::Line);
    assert_eq!(active_line(&d, 3000, 2000), Some(1));
    assert_eq!(active_line(&d, 3000, -3000), None);
    assert_eq!(active_line(&d, i64::MAX, i64::MAX), Some(1));
}

#[test]
fn active_line_at_reads_the_position_in_whole_milliseconds() {
    let d = doc(&[Some(1000), Some(5000)], Granularity::Line);
    assert_eq!(active_line_at(&d, Duration::from_micros(384_999), 0), None);
    assert_eq!(active_line_at(&d, Duration::from_millis(385), 0), Some(0));
    assert_eq!(
        active_line_at(&d, Duration::from_millis(3000), 2000),
        Some(1)
    );
    assert_eq!(active_line_at(&d, Duration::MAX, 0), Some(1));
}

#[test]
fn unsynced_and_empty_have_no_active_line() {
    assert_eq!(
        active_line(&doc(&[None, None], Granularity::Unsynced), 5000, 0),
        None
    );
    assert_eq!(active_line(&doc(&[], Granularity::Line), 5000, 0), None);
}

#[test]
fn the_edge_mask_fades_lines_toward_the_top_and_bottom() {
    let vh = 500.0;
    assert!(edge_fade(0.0, vh).abs() < 1e-6);
    assert!(edge_fade(vh, vh).abs() < 1e-6);
    assert!(
        (edge_fade(0.35 * vh, vh) - 1.0).abs() < 1e-6,
        "the anchor is clear"
    );
    assert!((edge_fade(0.5 * vh, vh) - 1.0).abs() < 1e-6);
    let a = edge_fade(20.0, vh);
    let b = edge_fade(60.0, vh);
    assert!(a > 0.0 && a < b && b < 1.0, "{a} {b}");
    assert!((edge_fade(vh - 60.0, vh) - b).abs() < 1e-5, "symmetric");
    assert!(edge_fade(-10.0, vh).abs() < 1e-6);
}

#[test]
fn footer_shows_provider_and_sync_state() {
    let synced = doc(&[Some(0)], Granularity::Line);
    assert_eq!(footer(Some(&synced)), "Lyrics: Mock \u{b7} Synced");
    let plain = doc(&[None], Granularity::Unsynced);
    assert_eq!(footer(Some(&plain)), "Lyrics: Mock \u{b7} Not synced");
    assert_eq!(footer(None), "Lyrics: Unknown");
    let mut unnamed = synced;
    unnamed.provider = ProviderId::new("");
    assert_eq!(footer(Some(&unnamed)), "Lyrics: Unknown \u{b7} Synced");
    let syl = doc(&[Some(0)], Granularity::Syllable);
    assert_eq!(footer(Some(&syl)), "Lyrics: Mock \u{b7} Syllable synced");
    assert_eq!(level_text(Granularity::Word), "Word synced");
}

fn credit(label: &str, url: Option<&str>, by: Option<&str>) -> LyricsCredit {
    LyricsCredit {
        label: label.to_owned(),
        url: url.map(str::to_owned),
        by: by.map(str::to_owned),
        by_url: None,
        note: None,
    }
}

#[test]
fn the_credit_is_merged_into_the_one_footer_line() {
    let mut d = doc(&[Some(0)], Granularity::Syllable);
    d.provider = ProviderId::new("Better Lyrics");
    let c = credit(
        "Lyrics via Better Lyrics",
        Some("https://better-lyrics.boidu.dev/"),
        None,
    );
    let line = footer_line(Some(&d), Some(&c));
    assert_eq!(
        line,
        "Lyrics: Better Lyrics \u{b7} Syllable synced \u{b7} better-lyrics.boidu.dev"
    );
    assert!(!line.contains('\n'));
    assert_eq!(line.matches("Better Lyrics").count(), 1, "named once");
    // A label that does not name the provider is kept, so credit is never lost.
    let mut u = doc(&[Some(0)], Granularity::Word);
    u.provider = ProviderId::new("Unison");
    let c = credit("Community lyric", None, Some("mira_k"));
    assert_eq!(
        footer_line(Some(&u), Some(&c)),
        "Lyrics: Unison \u{b7} Word synced \u{b7} Community lyric \u{b7} by mira_k"
    );
    assert_eq!(footer_line(Some(&u), None), footer(Some(&u)));
    assert_eq!(footer_line(None, None), "Lyrics: Unknown");
}
