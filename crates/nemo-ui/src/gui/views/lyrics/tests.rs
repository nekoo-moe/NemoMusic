use super::*;

#[test]
fn credit_names_the_submitter_and_the_link_host() {
    let c = LyricsCredit {
        label: "Lyrics via Better Lyrics".to_owned(),
        url: Some("https://better-lyrics.boidu.dev/".to_owned()),
        by: None,
        by_url: None,
        note: None,
    };
    assert_eq!(
        credit_text(&c),
        "Lyrics via Better Lyrics \u{b7} better-lyrics.boidu.dev"
    );
    let u = LyricsCredit {
        label: "Lyrics from Unison".to_owned(),
        url: None,
        by: Some("mira_k".to_owned()),
        by_url: None,
        note: None,
    };
    assert_eq!(credit_text(&u), "Lyrics from Unison \u{b7} by mira_k");
}

#[test]
fn long_lines_wrap_instead_of_being_cut() {
    let long = "và tâm hồn tôi là một bản nhạc chưa bao giờ được hát hết, dù đêm đã rất dài";
    assert!(rows_at(long, 292.0, 20.0) >= 2);
    assert_eq!(rows_at("short", 292.0, 20.0), 1);
}

#[test]
fn the_note_svg_fills_from_the_bottom_in_bounded_steps() {
    let ink = crate::theme::color::ON_SURFACE;
    let half = note::note_svg(0.5, ink, 1.0, 0.4);
    assert!(half.starts_with("<svg") && half.contains("fill=\"url(#f)\""));
    assert_eq!(half.matches("offset=\"0.5000\"").count(), 2, "{half}");
    assert!(half.contains("stop-opacity=\"1.000\"") && half.contains("stop-opacity=\"0.400\""));
    // Levels are rounded to 1/64, so nearby positions share one raster.
    assert_eq!(note::note_svg(0.5001, ink, 1.0, 0.4), half);
    assert!(note::note_svg(2.0, ink, 1.0, 0.4).contains("offset=\"1.0000\""));
    assert!(note::note_svg(-1.0, ink, 1.0, 0.4).contains("offset=\"0.0000\""));
}
