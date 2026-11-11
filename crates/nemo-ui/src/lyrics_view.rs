//! Lyrics view logic (`design.md` section 10). Pure: which line is active, how tall each line is,
//! where the scroll offset must be, and the footer text.

use nemo_core::lyrics::{Granularity, LyricsDoc};

use crate::theme::TextStyle;
use crate::theme::type_scale::{BODY_MD, HEADLINE_MD, TITLE_MD, TITLE_SM};

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

    /// Type style for the dense lyrics of the right pane: 20/28, 16/24, 14/20.
    #[must_use]
    pub fn style_d(self, dense: bool) -> TextStyle {
        if !dense {
            return self.style();
        }
        match self {
            Self::Active => TITLE_MD,
            Self::Neighbor => TITLE_SM,
            Self::Other => BODY_MD,
        }
    }

    /// Weight: 600 for the active line, 400 otherwise (two weights per view).
    #[must_use]
    pub fn semibold(self) -> bool {
        self == Self::Active
    }

    /// Index of this role in a [`LineRows`] entry.
    #[must_use]
    pub const fn slot(self) -> usize {
        match self {
            Self::Active => 0,
            Self::Neighbor => 1,
            Self::Other => 2,
        }
    }
}

/// Wrapped row counts of one lyric line in each role's type size: `[active, neighbor, other]`
/// (index with [`LineRole::slot`]). Measured by the GUI with the real font advances.
pub type LineRows = [u8; 3];

/// Height in px of a line of `role` that wraps onto `rows` rows.
#[must_use]
pub fn wrapped_height(role: LineRole, dense: bool, rows: u8) -> f32 {
    f32::from(role.style_d(dense).line) * f32::from(rows.max(1)) + LINE_GAP
}

/// Text for the sync level in the footer (wave-lyrics criterion 9).
#[must_use]
pub const fn level_text(g: Granularity) -> &'static str {
    match g {
        Granularity::Syllable => "Syllable synced",
        Granularity::Word => "Word synced",
        Granularity::Line => "Synced",
        Granularity::Unsynced => "Not synced",
    }
}

/// Short level tag for source lists (Tools panel, Settings).
#[must_use]
pub const fn level_tag(g: Granularity) -> &'static str {
    match g {
        Granularity::Syllable => "Syllable",
        Granularity::Word => "Word",
        Granularity::Line => "Line",
        Granularity::Unsynced => "Unsynced",
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
    row_height_d(role, false)
}

/// [`row_height`] for the dense (right pane) or roomy (full window) lyrics.
#[must_use]
pub fn row_height_d(role: LineRole, dense: bool) -> f32 {
    f32::from(role.style_d(dense).line) + LINE_GAP
}

/// The scroll offset (px from the top of the list) that puts the top of line `active` at
/// [`ANCHOR`] of the viewport. `line_count` lines, one text row each.
#[must_use]
pub fn scroll_target(active: Option<usize>, viewport_h: f32) -> f32 {
    scroll_target_d(active, viewport_h, false)
}

/// [`scroll_target`] for the dense (right pane) or roomy (full window) lyrics.
#[must_use]
pub fn scroll_target_d(active: Option<usize>, viewport_h: f32, dense: bool) -> f32 {
    scroll_target_rows(active, viewport_h, dense, &[])
}

/// [`scroll_target_d`] with the wrapped row counts of every line (`rows[i]` for line `i`; a
/// missing entry counts as one row), so a long line that wraps keeps the anchor right.
#[must_use]
pub fn scroll_target_rows(
    active: Option<usize>,
    viewport_h: f32,
    dense: bool,
    rows: &[LineRows],
) -> f32 {
    let Some(a) = active else {
        return 0.0;
    };
    let height = |i: usize| {
        let role = LineRole::of(i, Some(a));
        wrapped_height(role, dense, rows.get(i).map_or(1, |r| r[role.slot()]))
    };
    let above: f32 = (0..a).map(height).sum();
    // The anchor puts the line's top at 35%; a line that wraps onto more rows than fit below
    // the anchor moves up so its last row stays in view (its top never leaves the view).
    let fit = (above + height(a) - viewport_h).max(0.0);
    let raw = (above - ANCHOR * viewport_h).max(fit).min(above).max(0.0);
    if !dense {
        return raw;
    }
    // The small pane rests on a row edge, so no half-cut line shows at its top; when the
    // active line needs more room, the next edge down is used instead.
    let mut edge = 0.0;
    let mut acc = 0.0;
    for i in 0..=a {
        if acc > raw {
            break;
        }
        edge = acc;
        acc += height(i);
    }
    if edge + 0.5 < fit {
        edge = acc.min(above);
    }
    edge
}

/// Footer text: `Lyrics: <provider> . <level>` (design.md section 10, wave-lyrics criterion 9).
/// Never hidden.
#[must_use]
pub fn footer(doc: Option<&LyricsDoc>) -> String {
    match doc {
        None => "Lyrics: Unknown".to_owned(),
        Some(d) => {
            let name = d.provider.as_str();
            let name = if name.is_empty() { "Unknown" } else { name };
            format!("Lyrics: {name} \u{b7} {}", level_text(d.granularity))
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
    fn wrapped_lines_move_the_anchor_by_their_real_height() {
        let one = scroll_target(Some(20), 600.0);
        let mut rows = vec![[1_u8, 1, 1]; 30];
        rows[5] = [3, 2, 2];
        let wrapped = scroll_target_rows(Some(20), 600.0, false, &rows);
        let extra = f32::from(LineRole::Other.style().line);
        assert!((wrapped - one - extra).abs() < 1e-3, "{wrapped} vs {one}");
        // Rows of the active line itself do not move its top.
        rows[20] = [4, 4, 4];
        assert!((scroll_target_rows(Some(20), 600.0, false, &rows) - wrapped).abs() < 1e-3);
        // A line too tall for the room under the anchor moves up until its last row shows.
        let mut tall = vec![[1_u8, 1, 1]; 4];
        tall[2] = [3, 3, 3];
        let vh = 140.0;
        let off = scroll_target_rows(Some(2), vh, true, &tall);
        let above: f32 = (0..2)
            .map(|i| wrapped_height(LineRole::of(i, Some(2)), true, 1))
            .sum();
        let bottom = above + wrapped_height(LineRole::Active, true, 3);
        assert!(bottom - off <= vh + 1e-3, "{off}: bottom {bottom}");
        assert!(off <= above + 1e-3);
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
}
