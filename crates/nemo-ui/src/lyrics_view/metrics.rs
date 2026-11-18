//! Type and layout numbers of the lyrics (`lyrics-rebuild` spec, owner request 2026-11-18).
//! Every lyric line uses `font.display` at weight 600 and one size; lit and unlit lines differ by
//! alpha and scale only. Pure: no iced types.

/// Alpha of an unlit line (the owner's number of 2026-11-16, one token).
pub const INACTIVE_ALPHA: f32 = 0.38;
/// Scale of an unlit line, around its left edge.
pub const INACTIVE_SCALE: f32 = 0.95;
/// Line height over size. Better Lyrics uses 1.333; a little more keeps stacked Vietnamese marks
/// clear of the next row.
pub const LINE_HEIGHT_RATIO: f32 = 1.35;
/// Space above and below a line, in em (so two lines are 1.2 em apart).
pub const GAP_EM: f32 = 0.6;
/// Smallest and largest full-screen size, px.
pub const FULL_SIZE: (f32, f32) = (40.0, 48.0);
/// Smallest and largest size of the right pane, px.
pub const PANE_SIZE: (f32, f32) = (22.0, 26.0);
/// Size of the three-line lyrics (compact layout, Classic style), px.
pub const COMPACT_SIZE: f32 = 16.0;
/// Window width per px of full-screen size: 1280 wide gives 40 px, 1536 and more give 48 px.
const WINDOW_PER_PX: f32 = 32.0;
/// Pane width per px of pane size: 316 wide gives 22 px, 364 and more give 26 px.
const PANE_PER_PX: f32 = 14.0;
/// The full-screen lyric column takes at least this many percent of the window width.
pub const COLUMN_PCT_MIN: u32 = 58;
/// The full-screen lyric column takes at most this many percent of the window width.
pub const COLUMN_PCT_MAX: u32 = 66;

/// Size and spacing of one lyric line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    /// Font size, px.
    pub size: f32,
    /// Height of one text row, px.
    pub line_h: f32,
    /// Space above and below a line, px.
    pub gap: f32,
}

impl Metrics {
    fn from_size(size: f32) -> Self {
        Self {
            size,
            line_h: (size * LINE_HEIGHT_RATIO).round(),
            gap: (size * GAP_EM).round(),
        }
    }

    /// The full-screen lyrics in a window `window_w` px wide: 40 to 48 px.
    #[must_use]
    pub fn full(window_w: f32) -> Self {
        Self::from_size(
            (window_w / WINDOW_PER_PX)
                .clamp(FULL_SIZE.0, FULL_SIZE.1)
                .round(),
        )
    }

    /// The right pane lyrics in a pane `pane_w` px wide: 22 to 26 px.
    #[must_use]
    pub fn pane(pane_w: f32) -> Self {
        Self::from_size(
            (pane_w / PANE_PER_PX)
                .clamp(PANE_SIZE.0, PANE_SIZE.1)
                .round(),
        )
    }

    /// The three-line lyrics of the compact layout and the Classic style: 16 px, under the pane
    /// size so three lines and the footer fit under the controls.
    #[must_use]
    pub fn compact() -> Self {
        Self::from_size(COMPACT_SIZE)
    }

    /// Height of a line that wraps onto `rows` rows, with the gap above and below, px.
    #[must_use]
    pub fn slot_height(&self, rows: u8) -> f32 {
        f32::from(rows.max(1)) * self.line_h + 2.0 * self.gap
    }

    /// Height of the text rows alone, px.
    #[must_use]
    pub fn text_height(&self, rows: u8) -> f32 {
        f32::from(rows.max(1)) * self.line_h
    }
}

/// Width of the full-screen lyric column: all of `avail` px (what the window leaves next to the
/// art), held between [`COLUMN_PCT_MIN`] and [`COLUMN_PCT_MAX`] percent of `window_w`. A result
/// above `avail` tells the caller to shrink the art.
#[must_use]
pub fn column_width(window_w: u32, avail: u32) -> u32 {
    avail
        .min(window_w * COLUMN_PCT_MAX / 100)
        .max(window_w * COLUMN_PCT_MIN / 100)
}

/// Alpha and scale of one line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    /// Opacity of the ink, 0 to 1.
    pub alpha: f32,
    /// Scale around the left edge of the line.
    pub scale: f32,
}

/// The look of a line that is `lit` (0 unlit to 1 fully lit; see `clock::LineWindow::lit_progress`).
/// With `reduced` motion the scale stays 1 and only the colour changes.
#[must_use]
pub fn look(lit: f32, reduced: bool) -> Look {
    let p = lit.clamp(0.0, 1.0);
    Look {
        alpha: INACTIVE_ALPHA + (1.0 - INACTIVE_ALPHA) * p,
        scale: if reduced {
            1.0
        } else {
            INACTIVE_SCALE + (1.0 - INACTIVE_SCALE) * p
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_screen_size_follows_the_window_width_between_40_and_48() {
        assert!((Metrics::full(960.0).size - 40.0).abs() < f32::EPSILON);
        assert!((Metrics::full(1280.0).size - 40.0).abs() < f32::EPSILON);
        let mid = Metrics::full(1366.0).size;
        assert!(mid > 40.0 && mid < 48.0, "{mid}");
        assert!((Metrics::full(1920.0).size - 48.0).abs() < f32::EPSILON);
        assert!((Metrics::full(5000.0).size - 48.0).abs() < f32::EPSILON);
    }

    #[test]
    fn the_pane_size_is_22_to_26() {
        assert!((Metrics::pane(300.0).size - 22.0).abs() < f32::EPSILON);
        assert!((Metrics::pane(340.0).size - 24.0).abs() < 1.0);
        assert!((Metrics::pane(380.0).size - 26.0).abs() < f32::EPSILON);
    }

    #[test]
    fn line_height_keeps_vietnamese_marks_clear_and_the_gap_is_0_6_em() {
        for w in [960.0, 1366.0, 1920.0] {
            let m = Metrics::full(w);
            assert!(m.line_h >= 1.333 * m.size, "{w}: {m:?}");
            assert!((m.gap - 0.6 * m.size).abs() <= 0.5, "{w}: {m:?}");
        }
        let m = Metrics::full(1920.0);
        assert!((m.slot_height(2) - (2.0 * m.line_h + 2.0 * m.gap)).abs() < 1e-3);
        assert!(
            (m.slot_height(0) - m.slot_height(1)).abs() < 1e-3,
            "at least one row"
        );
        assert!((m.text_height(3) - 3.0 * m.line_h).abs() < 1e-3);
    }

    #[test]
    fn the_column_fills_what_is_left_between_58_and_66_percent() {
        assert_eq!(column_width(1000, 700), 660, "capped at 66%");
        assert_eq!(column_width(1000, 620), 620, "all that is left");
        assert_eq!(column_width(1000, 500), 580, "at least 58%");
        assert_eq!(column_width(1366, 837), 837);
    }

    #[test]
    fn an_unlit_line_is_dim_and_small_a_lit_line_is_full() {
        let off = look(0.0, false);
        assert!((off.alpha - 0.38).abs() < 1e-6 && (off.scale - 0.95).abs() < 1e-6);
        let on = look(1.0, false);
        assert!((on.alpha - 1.0).abs() < 1e-6 && (on.scale - 1.0).abs() < 1e-6);
        let half = look(0.5, false);
        assert!(half.alpha > off.alpha && half.alpha < on.alpha);
        assert!(half.scale > off.scale && half.scale < on.scale);
        assert!((look(2.0, false).alpha - 1.0).abs() < 1e-6, "clamped");
    }

    #[test]
    fn reduced_motion_changes_colour_only() {
        for p in [0.0, 0.3, 1.0] {
            assert!((look(p, true).scale - 1.0).abs() < f32::EPSILON);
        }
        assert!((look(0.0, true).alpha - INACTIVE_ALPHA).abs() < 1e-6);
    }
}
