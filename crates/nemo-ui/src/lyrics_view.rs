//! Lyrics view logic (`design.md` section 10). Pure: which line is active, how tall each line is,
//! where the scroll offset must be, and the footer text.

use nemo_core::lyrics::{Granularity, LyricsDoc};

use crate::theme::TextStyle;
use crate::theme::type_scale::{HEADLINE_MD, TITLE_MD, TITLE_SM};

/// A line changes when `position >= start - 150 ms`.
pub const LEAD_MS: i64 = 150;
/// The active line sits at this fraction of the viewport height.
pub const ANCHOR: f32 = 0.35;
/// Manual scrolling pauses auto-follow for this long.
pub const FOLLOW_PAUSE_MS: f32 = 4000.0;
/// Space between lines in px.
pub const LINE_GAP: f32 = 8.0;

/// How a line is drawn relative to the active one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineRole {
    /// The line being sung.
    Active,
    /// The line right before or after it.
    Neighbor,
    /// Any other line.
    Other,
}

impl LineRole {
    /// Role of line `index` when `active` is the active line (`None`: nothing is active).
    #[must_use]
    pub fn of(index: usize, active: Option<usize>) -> Self {
        match active {
            Some(a) if a == index => Self::Active,
            Some(a) if a.abs_diff(index) == 1 => Self::Neighbor,
            _ => Self::Other,
        }
    }

    /// Type style: `headline-md` 28/36, `title-md` 20/28, `title-sm` 16/24.
    #[must_use]
    pub fn style(self) -> TextStyle {
        match self {
            Self::Active => HEADLINE_MD,
            Self::Neighbor => TITLE_MD,
            Self::Other => TITLE_SM,
        }
    }

    /// Weight: 600 for the active line, 400 otherwise (two weights per view).
    #[must_use]
    pub fn semibold(self) -> bool {
        self == Self::Active
    }
}

/// Index of the active line for a playback position, or `None` before the first line, for
/// unsynced lyrics and for empty documents. `offset_ms` is added to the position (a positive
/// offset shows lines earlier).
#[must_use]
pub fn active_line(doc: &LyricsDoc, position_ms: i64, offset_ms: i64) -> Option<usize> {
    if doc.granularity == Granularity::Unsynced {
        return None;
    }
    let t = position_ms
        .saturating_add(offset_ms)
        .saturating_add(LEAD_MS);
    doc.lines
        .iter()
        .enumerate()
        .take_while(|(_, l)| {
            l.start_ms
                .is_some_and(|s| i64::try_from(s).unwrap_or(i64::MAX) <= t)
        })
        .map(|(i, _)| i)
        .last()
}

/// Height in px of a one-line row of the given role.
#[must_use]
pub fn row_height(role: LineRole) -> f32 {
    f32::from(role.style().line) + LINE_GAP
}

/// The scroll offset (px from the top of the list) that puts the top of line `active` at
/// [`ANCHOR`] of the viewport. `line_count` lines, one text row each.
#[must_use]
pub fn scroll_target(active: Option<usize>, viewport_h: f32) -> f32 {
    let Some(a) = active else {
        return 0.0;
    };
    let above: f32 = (0..a).map(|i| row_height(LineRole::of(i, Some(a)))).sum();
    (above - ANCHOR * viewport_h).max(0.0)
}

/// Footer text: `Lyrics: <provider> . Synced` (design.md section 10). Never hidden.
#[must_use]
pub fn footer(doc: Option<&LyricsDoc>) -> String {
    match doc {
        None => "Lyrics: Unknown".to_owned(),
        Some(d) => {
            let name = d.provider.as_str();
            let name = if name.is_empty() { "Unknown" } else { name };
            let sync = if d.granularity == Granularity::Unsynced {
                "Not synced"
            } else {
                "Synced"
            };
            format!("Lyrics: {name} \u{b7} {sync}")
        }
    }
}

#[cfg(test)]
mod tests {
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
    fn line_changes_150_ms_early() {
        let d = doc(&[Some(1000), Some(5000), Some(9000)], Granularity::Line);
        assert_eq!(active_line(&d, 0, 0), None);
        assert_eq!(active_line(&d, 849, 0), None);
        assert_eq!(active_line(&d, 850, 0), Some(0));
        assert_eq!(active_line(&d, 4849, 0), Some(0));
        assert_eq!(active_line(&d, 4850, 0), Some(1));
        assert_eq!(active_line(&d, 99_000, 0), Some(2));
    }

    #[test]
    fn offset_shifts_the_active_line() {
        let d = doc(&[Some(1000), Some(5000)], Granularity::Line);
        assert_eq!(active_line(&d, 3000, 2000), Some(1));
        assert_eq!(active_line(&d, 3000, -2500), None);
        assert_eq!(active_line(&d, i64::MAX, i64::MAX), Some(1));
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
    fn roles_and_styles() {
        assert_eq!(LineRole::of(3, Some(3)), LineRole::Active);
        assert_eq!(LineRole::of(2, Some(3)), LineRole::Neighbor);
        assert_eq!(LineRole::of(4, Some(3)), LineRole::Neighbor);
        assert_eq!(LineRole::of(0, Some(3)), LineRole::Other);
        assert_eq!(LineRole::of(0, None), LineRole::Other);
        assert_eq!(LineRole::Active.style().size, 28);
        assert_eq!(LineRole::Neighbor.style().size, 20);
        assert_eq!(LineRole::Other.style().size, 16);
        assert!(LineRole::Active.semibold() && !LineRole::Other.semibold());
    }

    #[test]
    fn scroll_target_anchors_at_35_percent() {
        assert!(scroll_target(None, 600.0).abs() < f32::EPSILON);
        // The first lines are above the anchor already: no negative scroll.
        assert!(scroll_target(Some(2), 600.0).abs() < f32::EPSILON);
        let off = scroll_target(Some(20), 600.0);
        let above: f32 = (0..20).map(|i| row_height(LineRole::of(i, Some(20)))).sum();
        assert!((above - off - 0.35 * 600.0).abs() < 1e-3);
        assert!(scroll_target(Some(21), 600.0) > off);
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
    }
}
