//! Tests of the lyrics editor model and its preview.

use super::*;

const TEXT: &str = "first line\n\n  second line  \nthird\nfourth";

fn at(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

fn starts(model: &EditorModel) -> Vec<Option<u64>> {
    (0..model.len())
        .map(|i| model.line(i).and_then(|(_, s)| s))
        .collect()
}

#[test]
fn pasted_text_becomes_untimed_lines() {
    let model = EditorModel::from_text(TEXT);
    assert_eq!(model.len(), 4);
    assert_eq!(model.line(1), Some(("second line", None)));
    assert_eq!(model.pass(), Pass::Lines);
    assert!(EditorModel::from_text(" \n ").is_empty());
}

#[test]
fn a_tap_starts_the_current_line_and_moves_on() {
    let mut model = EditorModel::from_text(TEXT);
    model.tap(at(1_200));
    model.tap(at(3_450));
    assert_eq!(starts(&model), vec![Some(1_200), Some(3_450), None, None]);
    assert_eq!(model.current_line(), 2);
    model.tap(at(5_000));
    model.tap(at(7_000));
    model.tap(at(9_000));
    assert_eq!(
        model.current_line(),
        4,
        "a tap after the last line does nothing"
    );
    assert_eq!(starts(&model)[3], Some(7_000));
}

#[test]
fn step_back_and_retap_time_a_line_again() {
    let mut model = EditorModel::from_text(TEXT);
    model.tap(at(1_000));
    model.tap(at(2_000));
    model.step_back();
    assert_eq!(model.current_line(), 1);
    assert_eq!(starts(&model)[1], Some(2_000), "kept until the next tap");
    model.tap(at(2_400));
    assert_eq!(starts(&model)[1], Some(2_400));
    model.retap(0, at(900));
    assert_eq!(starts(&model)[0], Some(900));
    assert_eq!(model.current_line(), 1);
    model.step_back();
    model.step_back();
    assert_eq!(model.current_line(), 0, "never before the first line");
}

#[test]
fn nudge_moves_a_start_by_50_ms_and_not_below_zero() {
    let mut model = EditorModel::from_text(TEXT);
    model.tap(at(100));
    model.nudge(0, 1);
    assert_eq!(starts(&model)[0], Some(150));
    model.nudge(0, -2);
    assert_eq!(starts(&model)[0], Some(50));
    model.nudge(0, -5);
    assert_eq!(starts(&model)[0], Some(0));
    model.nudge(1, 3);
    assert_eq!(starts(&model)[1], None, "an untimed line gets no start");
}

#[test]
fn saving_names_the_first_line_with_no_start_and_spreads_nothing() {
    let mut model = EditorModel::from_text(TEXT);
    model.tap(at(1_000));
    model.tap(at(2_000));
    assert_eq!(model.draft(), Err(OwnLyricsError::MissingStart { line: 3 }));
    assert_eq!(starts(&model), vec![Some(1_000), Some(2_000), None, None]);
    assert_eq!(
        EditorModel::from_text("").draft(),
        Err(OwnLyricsError::Empty)
    );
    model.tap(at(3_000));
    model.tap(at(4_000));
    let draft = model.draft().expect("complete");
    assert_eq!(draft.lines[2].start_ms, Some(3_000));
    assert!(draft.lines.iter().all(|l| l.words.is_empty()));
}

#[test]
fn the_word_pass_times_words_line_by_line() {
    let mut model = EditorModel::from_text("a b\nc");
    assert_eq!(
        model.start_word_pass(),
        Err(OwnLyricsError::MissingStart { line: 1 })
    );
    model.tap(at(0));
    model.tap(at(1_000));
    model.start_word_pass().expect("lines are timed");
    assert_eq!(model.pass(), Pass::Words);
    model.tap(at(10));
    model.step_back();
    model.tap(at(20));
    model.tap(at(400));
    assert_eq!(model.current_line(), 1);
    assert_eq!(model.words(0), vec![("a ", Some(20)), ("b", Some(400))]);
    let half = model.draft().expect("draft");
    assert_eq!(half.lines[0].words.len(), 2);
    assert!(
        half.lines[1].words.is_empty(),
        "an untapped word keeps the line line-timed"
    );
    model.tap(at(1_050));
    let full = model.draft().expect("draft");
    assert_eq!(full.lines[1].words[0].start_ms, 1_050);
    model.step_back();
    assert_eq!((model.current_line(), model.current_word()), (1, 0));
}

#[test]
fn existing_lyrics_keep_their_starts() {
    let doc = LyricsDoc {
        lines: vec![nemo_core::lyrics::LyricLine {
            start_ms: Some(500),
            end_ms: None,
            text: "kept".to_owned(),
            words: Vec::new(),
        }],
        granularity: nemo_core::lyrics::Granularity::Line,
        provider: nemo_core::lyrics::ProviderId::new("LRCLIB"),
        matched: None,
        instrumental: false,
    };
    let model = EditorModel::from_doc(&doc);
    assert_eq!(model.line(0), Some(("kept", Some(500))));
}

/// The lyrics view as the app runs it: the state's own active line for a saved doc.
fn view_line(doc: &LyricsDoc, position: Duration, offset_ms: i64) -> Option<usize> {
    let mut ui = crate::state::UiState::new(
        crate::route::Route::Home,
        nemo_core::player::PlayerSnapshot::default(),
        false,
    );
    ui.lyrics = crate::state::LyricsState::Ready(doc.clone());
    ui.position = position;
    ui.lyrics_offset_ms = offset_ms;
    ui.active_lyric()
}

fn timed() -> EditorModel {
    let mut model = EditorModel::from_text(TEXT);
    for start in [1_000, 3_500, 6_200, 9_000] {
        model.tap(at(start));
    }
    model
}

#[test]
fn the_preview_and_the_lyrics_view_find_the_same_line_for_any_position_and_offset() {
    let mut partial = EditorModel::from_text(TEXT);
    partial.tap(at(1_000));
    partial.tap(at(3_500));
    for model in [timed(), partial] {
        let saved = model.preview_doc();
        for offset in [-10_000, -2_500, -150, 0, 50, 300, 10_000] {
            for step in 0..=240 {
                let position = at(step * 50);
                assert_eq!(
                    model.preview_line(position, offset),
                    view_line(&saved, position, offset),
                    "position {position:?}, offset {offset} ms"
                );
            }
        }
    }
}

#[test]
fn the_offset_moves_the_preview_highlight() {
    let model = timed();
    assert_eq!(model.preview_line(at(3_000), 0), Some(0));
    assert_eq!(
        model.preview_line(at(3_000), 400),
        Some(1),
        "a positive offset shows lines earlier"
    );
    assert_eq!(model.preview_line(at(3_000), -2_500), None);
    assert_eq!(
        model.preview_line(at(3_349), 0),
        Some(0),
        "line 2 shows 150 ms before its start"
    );
    assert_eq!(model.preview_line(at(3_350), 0), Some(1));
}

#[test]
fn an_untapped_line_stops_the_preview_highlight_and_gets_no_made_up_start() {
    let mut model = EditorModel::from_text(TEXT);
    model.tap(at(1_000));
    model.tap(at(3_500));
    assert_eq!(model.preview_line(at(60_000), 0), Some(1));
    let doc = model.preview_doc();
    let starts: Vec<Option<u64>> = doc.lines.iter().map(|l| l.start_ms).collect();
    assert_eq!(starts, vec![Some(1_000), Some(3_500), None, None]);
    assert_eq!(EditorModel::from_text("").preview_line(at(1_000), 0), None);
    assert_eq!(
        EditorModel::from_text("").preview_doc().granularity,
        Granularity::Line
    );
}

#[test]
fn the_preview_holds_what_saving_stores() {
    let mut model = EditorModel::from_text("a b\nc");
    model.tap(at(0));
    model.tap(at(1_000));
    model.start_word_pass().expect("lines are timed");
    for start in [20, 400] {
        model.tap(at(start));
    }
    let half = model.preview_doc();
    assert_eq!(half.provider, OWN_LYRICS);
    assert_eq!(
        half.granularity,
        Granularity::Line,
        "line 2 has no tapped word yet"
    );
    model.tap(at(1_050));
    let full = model.preview_doc();
    assert_eq!(full.granularity, Granularity::Word);
    let draft = model.draft().expect("draft");
    assert_eq!(full.lines.len(), draft.lines.len());
    for (shown, saved) in full.lines.iter().zip(&draft.lines) {
        assert_eq!(
            (&shown.text, shown.start_ms, &shown.words),
            (&saved.text, saved.start_ms, &saved.words)
        );
    }
    assert_eq!(model.preview_line(at(1_000), 0), Some(1));
}
