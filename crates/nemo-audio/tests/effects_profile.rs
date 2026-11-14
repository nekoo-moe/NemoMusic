//! The hand-written `AutoEq` fixtures in `tests/fixtures/autoeq/` parse as the equalizer
//! expects, and a profile the equalizer cannot represent is refused with its line.

use std::path::Path;

use nemo_audio::effects::{ProfileError, parse_autoeq, write_autoeq};
use nemo_core::effects::FilterKind;

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/autoeq")
        .join(name);
    std::fs::read_to_string(path).expect("fixture")
}

#[test]
fn the_ten_band_file_is_ten_peaks_at_the_iso_centres() {
    let profile = parse_autoeq("Ten band", &fixture("ten-band.txt")).expect("parse");
    assert_eq!(profile.name, "Ten band");
    assert_eq!(profile.preamp_tenth_db, -45);
    let centres: Vec<u16> = profile.filters.iter().map(|f| f.freq_hz).collect();
    assert_eq!(
        centres,
        [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]
    );
    assert!(
        profile
            .filters
            .iter()
            .all(|f| f.kind == FilterKind::Peak && f.q_hundredths == 141)
    );
    assert_eq!(profile.filters[0].gain_tenth_db, 40);
    assert_eq!(profile.filters[9].gain_tenth_db, -30);
}

#[test]
fn the_parametric_file_reads_shelves_and_peaks_and_writes_back_the_same_text() {
    let text = fixture("parametric.txt");
    let profile = parse_autoeq("Parametric", &text).expect("parse");
    let kinds: Vec<FilterKind> = profile.filters.iter().map(|f| f.kind).collect();
    assert_eq!(kinds[0], FilterKind::LowShelf);
    assert_eq!(kinds[5], FilterKind::HighShelf);
    assert!(kinds[1..5].iter().all(|k| *k == FilterKind::Peak));
    assert_eq!(profile.filters[4].q_hundredths, 475);
    let normalized = text.replace("\r\n", "\n");
    assert_eq!(write_autoeq(&profile), normalized);
}

#[test]
fn a_low_pass_filter_is_refused_and_named_with_its_line() {
    let err = parse_autoeq("Refused", &fixture("refused-low-pass.txt")).expect_err("refused");
    assert_eq!(
        err,
        ProfileError::UnsupportedFilter {
            line: 3,
            kind: "LP".into()
        }
    );
    assert_eq!(
        err.to_string(),
        "line 3: filter type LP is not supported, only PK, LSC and HSC"
    );
}
