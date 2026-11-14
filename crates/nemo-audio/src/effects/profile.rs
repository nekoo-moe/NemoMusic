//! `AutoEq` profile files: read and write the plain-text equalizer format.
//!
//! One format covers the fixed-band (ten-band) and the parametric files of the `AutoEq` project,
//! and the user's own presets:
//!
//! ```text
//! Preamp: -6.2 dB
//! Filter 1: ON PK Fc 105 Hz Gain 3.0 dB Q 0.70
//! Filter 2: ON LSC Fc 31 Hz Gain -2.5 dB Q 0.71
//! ```
//!
//! Blank lines and `#` comments are skipped, keywords match in any letter case, and an `OFF`
//! filter is left out. Pure functions with no I/O, off the real-time path.

use std::fmt::Write as _;

use nemo_core::effects::{EQ_MAX_FILTERS, EqFilter, FilterKind};

/// Lowest filter frequency a file may hold, in Hz.
const FREQ_MIN_HZ: i64 = 10;
/// Highest filter frequency a file may hold, in Hz.
const FREQ_MAX_HZ: i64 = 20_000;
/// Lowest Q a file may hold, in hundredths (Q 0.10).
const Q_MIN_HUNDREDTHS: i64 = 10;
/// Highest Q a file may hold, in hundredths (Q 10.00).
const Q_MAX_HUNDREDTHS: i64 = 1_000;
/// Lowest preamp a file may hold, in tenths of a dB (-30 dB).
const PREAMP_MIN_TENTH_DB: i64 = -300;
/// Highest preamp a file may hold, in tenths of a dB (+12 dB).
const PREAMP_MAX_TENTH_DB: i64 = 120;
/// Longest filter type name kept in [`ProfileError::UnsupportedFilter`].
const KIND_TEXT_MAX: usize = 16;

/// An equalizer profile: a preamp and up to [`EQ_MAX_FILTERS`] filters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EqProfile {
    /// The name shown to the user, taken from the file name (the file text names nothing).
    pub name: String,
    /// Preamp in tenths of a dB.
    pub preamp_tenth_db: i16,
    /// The filters in file order.
    pub filters: Vec<EqFilter>,
}

/// Why a profile file was refused. Nothing is applied when parsing fails. Lines count from 1.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProfileError {
    /// The file holds no preamp and no filter.
    #[error("the file holds no equalizer settings")]
    Empty,
    /// A filter type the equalizer cannot represent (only `PK`, `LSC` and `HSC` are).
    #[error("line {line}: filter type {kind} is not supported, only PK, LSC and HSC")]
    UnsupportedFilter {
        /// The line of the filter.
        line: usize,
        /// The filter type as written, cut to 16 characters.
        kind: String,
    },
    /// More than [`EQ_MAX_FILTERS`] filters are on.
    #[error("the file has more than {EQ_MAX_FILTERS} filters")]
    TooManyFilters,
    /// A value outside its range: frequency 10 Hz to 20 kHz, gain -12 to +12 dB, Q 0.10 to
    /// 10.00, preamp -30 to +12 dB.
    #[error("line {line}: a value is out of range")]
    OutOfRange {
        /// The line of the value.
        line: usize,
    },
    /// A line that is not a preamp, a filter, a comment or blank.
    #[error("line {line}: the line could not be read")]
    Syntax {
        /// The line that could not be read.
        line: usize,
    },
}

/// Read an `AutoEq` profile (fixed-band or parametric text). `name` becomes the profile name.
///
/// # Errors
/// Returns a [`ProfileError`] naming the first line that is refused: an unsupported filter
/// type, a value out of range, an unreadable line, more than [`EQ_MAX_FILTERS`] filters, or
/// a file with no settings at all.
pub fn parse_autoeq(name: &str, text: &str) -> Result<EqProfile, ProfileError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut preamp = None;
    let mut filters = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let body = raw.trim();
        if body.is_empty() || body.starts_with('#') {
            continue;
        }
        if let Some(rest) = strip_keyword(body, "Preamp:") {
            if preamp.is_some() {
                return Err(ProfileError::Syntax { line });
            }
            preamp = Some(parse_preamp(rest, line)?);
        } else if let Some(rest) = strip_keyword(body, "Filter") {
            if let Some(filter) = parse_filter(rest, line)? {
                if filters.len() == EQ_MAX_FILTERS {
                    return Err(ProfileError::TooManyFilters);
                }
                filters.push(filter);
            }
        } else {
            return Err(ProfileError::Syntax { line });
        }
    }
    if preamp.is_none() && filters.is_empty() {
        return Err(ProfileError::Empty);
    }
    Ok(EqProfile {
        name: name.to_owned(),
        preamp_tenth_db: preamp.unwrap_or(0),
        filters,
    })
}

/// Write `profile` in the `AutoEq` text format, one `Filter N: ON ...` line per filter. The name
/// is not written: it travels as the file name. [`parse_autoeq`] reads the text back to the
/// same profile when every value is inside its range.
#[must_use]
pub fn write_autoeq(profile: &EqProfile) -> String {
    let mut out = format!("Preamp: {} dB\n", tenths(profile.preamp_tenth_db));
    for (index, f) in profile.filters.iter().enumerate() {
        // Writing into a String cannot fail.
        let _ = writeln!(
            out,
            "Filter {}: ON {} Fc {} Hz Gain {} dB Q {}",
            index + 1,
            kind_code(f.kind),
            f.freq_hz,
            tenths(f.gain_tenth_db),
            hundredths(f.q_hundredths),
        );
    }
    out
}

/// The rest of `body` after `keyword`, matched in any letter case.
fn strip_keyword<'a>(body: &'a str, keyword: &str) -> Option<&'a str> {
    let head = body.get(..keyword.len())?;
    head.eq_ignore_ascii_case(keyword)
        .then(|| body.get(keyword.len()..))
        .flatten()
}

/// `-6.2 dB` after `Preamp:`.
fn parse_preamp(rest: &str, line: usize) -> Result<i16, ProfileError> {
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    let [value, unit] = tokens.as_slice() else {
        return Err(ProfileError::Syntax { line });
    };
    if !unit.eq_ignore_ascii_case("dB") {
        return Err(ProfileError::Syntax { line });
    }
    let tenth_db = parse_scaled(value, 1).ok_or(ProfileError::Syntax { line })?;
    in_range(tenth_db, PREAMP_MIN_TENTH_DB, PREAMP_MAX_TENTH_DB, line)
}

/// ` 1: ON PK Fc 105 Hz Gain 3.0 dB Q 0.70` after `Filter`. `None` for an `OFF` filter.
fn parse_filter(rest: &str, line: usize) -> Result<Option<EqFilter>, ProfileError> {
    let syntax = ProfileError::Syntax { line };
    let (index, params) = rest.split_once(':').ok_or(syntax.clone())?;
    if !index.trim().bytes().all(|b| b.is_ascii_digit()) {
        return Err(syntax);
    }
    let tokens: Vec<&str> = params.split_whitespace().collect();
    let [state, kind, values @ ..] = tokens.as_slice() else {
        return Err(syntax);
    };
    if state.eq_ignore_ascii_case("OFF") {
        return Ok(None);
    }
    if !state.eq_ignore_ascii_case("ON") {
        return Err(syntax);
    }
    let kind = filter_kind(kind).ok_or_else(|| ProfileError::UnsupportedFilter {
        line,
        kind: kind.chars().take(KIND_TEXT_MAX).collect(),
    })?;
    let [fc, freq, hz, gain_kw, gain, db, q_kw, q] = values else {
        return Err(syntax);
    };
    let keywords = [
        (fc, "Fc"),
        (hz, "Hz"),
        (gain_kw, "Gain"),
        (db, "dB"),
        (q_kw, "Q"),
    ];
    if !keywords
        .iter()
        .all(|(got, want)| got.eq_ignore_ascii_case(want))
    {
        return Err(syntax);
    }
    let number = |token: &str, decimals| parse_scaled(token, decimals).ok_or(syntax.clone());
    let (freq, gain, q) = (number(freq, 0)?, number(gain, 1)?, number(q, 2)?);
    Ok(Some(EqFilter {
        kind,
        freq_hz: in_range(freq, FREQ_MIN_HZ, FREQ_MAX_HZ, line)?,
        gain_tenth_db: in_range(
            gain,
            i64::from(EqFilter::GAIN_MIN_TENTH_DB),
            i64::from(EqFilter::GAIN_MAX_TENTH_DB),
            line,
        )?,
        q_hundredths: in_range(q, Q_MIN_HUNDREDTHS, Q_MAX_HUNDREDTHS, line)?,
    }))
}

fn filter_kind(code: &str) -> Option<FilterKind> {
    [
        ("PK", FilterKind::Peak),
        ("LSC", FilterKind::LowShelf),
        ("HSC", FilterKind::HighShelf),
    ]
    .into_iter()
    .find_map(|(name, kind)| code.eq_ignore_ascii_case(name).then_some(kind))
}

fn kind_code(kind: FilterKind) -> &'static str {
    match kind {
        FilterKind::Peak => "PK",
        FilterKind::LowShelf => "LSC",
        FilterKind::HighShelf => "HSC",
    }
}

/// `value` as the target integer type when it lies in `min..=max`.
fn in_range<T: TryFrom<i64>>(
    value: i64,
    min: i64,
    max: i64,
    line: usize,
) -> Result<T, ProfileError> {
    (min..=max)
        .contains(&value)
        .then(|| T::try_from(value).ok())
        .flatten()
        .ok_or(ProfileError::OutOfRange { line })
}

/// A plain decimal number (`-6.2`, `105`, `.5`) as a count of `10^-decimals` units, rounded
/// half away from zero. Whole parts too long to matter saturate, so the range check refuses
/// them. `None` when `token` is not a plain decimal number.
fn parse_scaled(token: &str, decimals: u32) -> Option<i64> {
    let (negative, digits) = match token.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, token.strip_prefix('+').unwrap_or(token)),
    };
    let (whole, frac) = digits.split_once('.').unwrap_or((digits, ""));
    let all_digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if (whole.is_empty() && frac.is_empty()) || !all_digits(whole) || !all_digits(frac) {
        return None;
    }
    let whole = whole.trim_start_matches('0');
    if whole.len() > 9 {
        return Some(if negative { i64::MIN } else { i64::MAX });
    }
    let digit = |b: u8| i64::from(b - b'0');
    let mut value = whole.bytes().fold(0_i64, |v, b| v * 10 + digit(b));
    let mut rest = frac.bytes();
    for _ in 0..decimals {
        value = value * 10 + rest.next().map_or(0, digit);
    }
    if rest.next().is_some_and(|b| b >= b'5') {
        value += 1;
    }
    Some(if negative { -value } else { value })
}

/// Tenths as a decimal with one place: `-62` is `-6.2`.
fn tenths(value: i16) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let abs = value.unsigned_abs();
    format!("{sign}{}.{}", abs / 10, abs % 10)
}

/// Hundredths as a decimal with two places: `70` is `0.70`.
fn hundredths(value: u16) -> String {
    format!("{}.{:02}", value / 100, value % 100)
}

#[cfg(test)]
mod tests {
    use nemo_core::effects::{EqFilter, FilterKind};
    use proptest::collection::vec;
    use proptest::prelude::*;

    use super::{EqProfile, ProfileError, parse_autoeq, parse_scaled, write_autoeq};

    fn refused(text: &str) -> ProfileError {
        parse_autoeq("t", text).expect_err("refused")
    }

    #[test]
    fn reads_a_parametric_line_and_the_preamp() {
        let p = parse_autoeq(
            "Mine",
            "Preamp: -6.2 dB\nFilter 1: ON PK Fc 105 Hz Gain 3.0 dB Q 0.70\n",
        )
        .expect("parse");
        let want = EqFilter {
            kind: FilterKind::Peak,
            freq_hz: 105,
            gain_tenth_db: 30,
            q_hundredths: 70,
        };
        assert_eq!(p.name, "Mine");
        assert_eq!(p.preamp_tenth_db, -62);
        assert_eq!(p.filters, vec![want]);
    }

    #[test]
    fn skips_comments_blank_lines_off_filters_and_a_bom() {
        let text = "\u{feff}# made by hand\n\npreamp: 0 db\nfilter: off pk fc 100 hz gain 1 db q 1\n\
                    filter 2: on hsc fc 8000 hz gain -1.5 db q 0.71\n";
        let p = parse_autoeq("t", text).expect("parse");
        assert_eq!(p.filters.len(), 1);
        assert_eq!(p.filters[0].kind, FilterKind::HighShelf);
        assert_eq!(p.filters[0].gain_tenth_db, -15);
    }

    #[test]
    fn refuses_each_kind_of_bad_file() {
        assert_eq!(refused(""), ProfileError::Empty);
        assert_eq!(refused("# only a comment\n"), ProfileError::Empty);
        assert_eq!(
            refused("Preamp: 0 dB\nFilter 1: ON LP Fc 100 Hz Q 0.71\n"),
            ProfileError::UnsupportedFilter {
                line: 2,
                kind: "LP".into()
            }
        );
        let many = "Filter 1: ON PK Fc 100 Hz Gain 1.0 dB Q 1.00\n".repeat(13);
        assert_eq!(refused(&many), ProfileError::TooManyFilters);
        let out = [
            "Filter 1: ON PK Fc 5 Hz Gain 1.0 dB Q 1.00",
            "Filter 1: ON PK Fc 100 Hz Gain 12.1 dB Q 1.00",
            "Filter 1: ON PK Fc 100 Hz Gain 1.0 dB Q 0.05",
            "Preamp: -30.1 dB",
        ];
        for text in out {
            assert_eq!(
                refused(text),
                ProfileError::OutOfRange { line: 1 },
                "{text}"
            );
        }
        let bad = [
            "GraphicEQ: 20 0",
            "Filter 1: ON PK Fc 1e3 Hz Gain 1.0 dB Q 1.00",
            "Filter 1: ON PK Fc 100 Hz Gain 1.0 dB",
            "Filter x: ON PK Fc 100 Hz Gain 1.0 dB Q 1.00",
            "Preamp: -6.2",
        ];
        for text in bad {
            assert_eq!(refused(text), ProfileError::Syntax { line: 1 }, "{text}");
        }
        assert_eq!(
            refused("Preamp: 0 dB\nPreamp: 0 dB"),
            ProfileError::Syntax { line: 2 }
        );
    }

    #[test]
    fn scaled_numbers_round_half_away_from_zero() {
        assert_eq!(parse_scaled("0.705", 2), Some(71));
        assert_eq!(parse_scaled("-0.05", 1), Some(-1));
        assert_eq!(parse_scaled(".5", 1), Some(5));
        assert_eq!(parse_scaled("+3", 1), Some(30));
        assert_eq!(parse_scaled("12345678901", 0), Some(i64::MAX));
        assert_eq!(parse_scaled("-", 0), None);
        assert_eq!(parse_scaled("1.2.3", 1), None);
    }

    fn filter() -> impl Strategy<Value = EqFilter> {
        let kind = prop_oneof![
            Just(FilterKind::Peak),
            Just(FilterKind::LowShelf),
            Just(FilterKind::HighShelf)
        ];
        (kind, 10_u16..=20_000, -120_i16..=120, 10_u16..=1000).prop_map(
            |(kind, freq_hz, gain_tenth_db, q_hundredths)| EqFilter {
                kind,
                freq_hz,
                gain_tenth_db,
                q_hundredths,
            },
        )
    }

    const TOKENS: &[&str] = &[
        "Filter",
        "Filter 1:",
        "Preamp:",
        ":",
        "ON",
        "OFF",
        "PK",
        "LSC",
        "HSC",
        "LP",
        "Fc",
        "Hz",
        "Gain",
        "dB",
        "Q",
        "-",
        ".",
        "0",
        "1.5",
        "-12.0",
        "20000",
        "99999999999",
        "\n",
        "#",
    ];

    proptest! {
        #[test]
        fn write_then_parse_gives_the_same_profile(
            preamp_tenth_db in -300_i16..=120,
            filters in vec(filter(), 0..=12),
        ) {
            let profile = EqProfile { name: "Round trip".into(), preamp_tenth_db, filters };
            let back = parse_autoeq(&profile.name, &write_autoeq(&profile));
            prop_assert_eq!(back, Ok(profile));
        }

        #[test]
        fn parse_never_panics_on_any_string(text in any::<String>()) {
            let _ = parse_autoeq("any", &text);
        }

        #[test]
        fn parse_never_panics_on_token_soup(tokens in vec(proptest::sample::select(TOKENS), 0..40)) {
            let _ = parse_autoeq("soup", &tokens.join(" "));
        }
    }
}
