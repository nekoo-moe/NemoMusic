use super::*;

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

#[test]
fn line_rows_has_one_entry_per_line_and_wraps_long_ones() {
    use nemo_core::lyrics::{LyricLine, ProviderId};
    let line = |t: &str| LyricLine {
        start_ms: Some(0),
        end_ms: None,
        text: t.to_owned(),
        words: Vec::new(),
    };
    let doc = LyricsDoc {
        lines: vec![line("short"), line(&"a long line of words ".repeat(8))],
        granularity: Granularity::Line,
        provider: ProviderId::new("Mock"),
        matched: None,
        instrumental: false,
    };
    let rows = line_rows(&doc, 400.0, 44.0);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], 1);
    assert!(rows[1] >= 3, "{rows:?}");
}
