//! Text formatting for times and offsets (`design.md` section 3). Pure.

use std::time::Duration;

/// Thin space between a number and its unit.
pub const THIN_SPACE: char = '\u{2009}';

/// `m:ss`, or `h:mm:ss` from one hour. Fractions of a second are dropped.
#[must_use]
pub fn clock(d: Duration) -> String {
    let total = d.as_secs();
    let (h, m, s) = (total / 3600, (total / 60) % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{}:{s:02}", total / 60)
    }
}

/// Lyrics offset as `+0.30 s` / `-1.25 s` / `0.00 s` (two decimals, thin space before the unit).
#[must_use]
pub fn offset_seconds(offset_ms: i64) -> String {
    let sign = match offset_ms {
        0 => "",
        v if v > 0 => "+",
        _ => "-",
    };
    let abs = offset_ms.unsigned_abs();
    // Rounded to the centisecond for display.
    let centi = (abs + 5) / 10;
    format!("{sign}{}.{:02}{THIN_SPACE}s", centi / 100, centi % 100)
}

/// Join artist names for a subtitle; `Unknown artist` when the list is empty.
#[must_use]
pub fn artists(names: &[String]) -> String {
    if names.is_empty() {
        "Unknown artist".to_owned()
    } else {
        names.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_formats() {
        assert_eq!(clock(Duration::ZERO), "0:00");
        assert_eq!(clock(Duration::from_secs(65)), "1:05");
        assert_eq!(clock(Duration::from_millis(59_999)), "0:59");
        assert_eq!(clock(Duration::from_secs(3599)), "59:59");
        assert_eq!(clock(Duration::from_secs(3600)), "1:00:00");
        assert_eq!(clock(Duration::from_secs(3 * 3600 + 7 * 60 + 9)), "3:07:09");
    }

    #[test]
    fn offsets() {
        assert_eq!(offset_seconds(0), "0.00\u{2009}s");
        assert_eq!(offset_seconds(300), "+0.30\u{2009}s");
        assert_eq!(offset_seconds(-1250), "-1.25\u{2009}s");
        assert_eq!(offset_seconds(10_000), "+10.00\u{2009}s");
        assert_eq!(offset_seconds(50), "+0.05\u{2009}s");
    }

    #[test]
    fn artists_join() {
        assert_eq!(artists(&[]), "Unknown artist");
        assert_eq!(artists(&["A".into(), "B".into()]), "A, B");
    }
}
