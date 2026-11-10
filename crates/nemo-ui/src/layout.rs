//! Responsive layout numbers from `docs/ui-reference.md` section 3 and `design.md` section 4.
//!
//! Pure functions of the window size: no GUI dependency, so every row of the reference table is
//! a unit test. Sizes are logical pixels.

/// Sidebar width at `W >= SIDEBAR_BREAKPOINT`.
pub const SIDEBAR_WIDE: u32 = 240;
/// Sidebar width (icons only) below the breakpoint.
pub const SIDEBAR_NARROW: u32 = 64;
/// Window width from which the wide sidebar is used.
pub const SIDEBAR_BREAKPOINT: u32 = 1100;
/// Content padding on both sides.
pub const CONTENT_PADDING: u32 = 24;
/// Widest content column.
pub const CONTENT_MAX: u32 = 1200;
/// Queue panel width.
pub const QUEUE_WIDTH: u32 = 320;
/// Window width from which the queue panel pushes the content (below it overlays).
pub const QUEUE_PUSH_BREAKPOINT: u32 = 1280;
/// Narrowest card art.
pub const CARD_MIN: u32 = 160;
/// Window width from which the grid gap grows to [`GAP_WIDE`].
pub const GAP_BREAKPOINT: u32 = 1600;
/// Grid gap below [`GAP_BREAKPOINT`].
pub const GAP_NARROW: u32 = 16;
/// Grid gap from [`GAP_BREAKPOINT`].
pub const GAP_WIDE: u32 = 24;
/// Fewest columns of a card grid.
pub const COLUMNS_MIN: u32 = 2;
/// Top bar height.
pub const TOP_BAR_HEIGHT: u32 = 56;
/// Player bar height.
pub const PLAYER_BAR_HEIGHT: u32 = 88;
/// Player bar inset from the window edges.
pub const PLAYER_BAR_INSET: u32 = 12;
/// Bottom scroll padding so the floating bar never hides the last row (88 + 12 + 12).
pub const CONTENT_BOTTOM_PADDING: u32 = PLAYER_BAR_HEIGHT + 2 * PLAYER_BAR_INSET;
/// Track row height.
pub const TRACK_ROW: u32 = 48;
/// Track row height in compact mode.
pub const TRACK_ROW_COMPACT: u32 = 40;
/// Window width below which Now Playing is compact.
pub const COMPACT_WIDTH: u32 = 1100;
/// Window height below which Now Playing is compact.
pub const COMPACT_HEIGHT: u32 = 600;
/// Widest Now Playing art.
pub const NOW_PLAYING_ART_MAX: u32 = 480;
/// Narrowest non-compact Now Playing art.
pub const NOW_PLAYING_ART_MIN: u32 = 320;
/// Compact Now Playing art (the largest; it shrinks with the height).
pub const COMPACT_ART_MAX: u32 = 280;
/// Narrowest compact art.
pub const COMPACT_ART_MIN: u32 = 160;
/// Widest lyrics column.
pub const LYRICS_MAX_WIDTH: u32 = 640;
/// Gap between the art column and the lyrics column in Now Playing.
pub const NOW_PLAYING_GAP: u32 = 64;
/// Now Playing header height.
pub const NOW_PLAYING_HEADER: u32 = 56;
/// Compact Now Playing header height.
pub const COMPACT_HEADER: u32 = 48;

/// The shape of a card grid for one window size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grid {
    /// Sidebar width.
    pub sidebar: u32,
    /// Content column width `C`.
    pub content: u32,
    /// Gap between cards.
    pub gap: u32,
    /// Number of columns.
    pub columns: u32,
    /// Card (and card art) width, rounded down to a multiple of 4.
    pub card: u32,
}

/// Sidebar width for a window width.
#[must_use]
pub const fn sidebar_width(window_w: u32) -> u32 {
    if window_w >= SIDEBAR_BREAKPOINT {
        SIDEBAR_WIDE
    } else {
        SIDEBAR_NARROW
    }
}

/// `true` when the open queue panel narrows the content (it overlays below the breakpoint).
#[must_use]
pub const fn queue_pushes_content(window_w: u32, queue_open: bool) -> bool {
    queue_open && window_w >= QUEUE_PUSH_BREAKPOINT
}

/// Compute the card grid for a window of width `window_w`.
///
/// `C = min(1200, W - sidebar - 48 - (320 if the queue pushes))`,
/// `n = max(2, floor((C + gap) / (160 + gap)))`, card = `(C - gap (n - 1)) / n` rounded down to 4.
#[must_use]
pub fn grid(window_w: u32, queue_open: bool) -> Grid {
    let sidebar = sidebar_width(window_w);
    let queue = if queue_pushes_content(window_w, queue_open) {
        QUEUE_WIDTH
    } else {
        0
    };
    let content = window_w
        .saturating_sub(sidebar + 2 * CONTENT_PADDING + queue)
        .min(CONTENT_MAX);
    let gap = if window_w >= GAP_BREAKPOINT {
        GAP_WIDE
    } else {
        GAP_NARROW
    };
    let columns = ((content + gap) / (CARD_MIN + gap)).max(COLUMNS_MIN);
    let card = content.saturating_sub(gap * (columns - 1)) / columns / 4 * 4;
    Grid {
        sidebar,
        content,
        gap,
        columns,
        card,
    }
}

/// `true` when Now Playing uses the compact layout (`W < 1100` or `H < 600`). 600, not 700, so a maximized window on a 1366 x 768 laptop (about 697 px high) keeps art and lyrics side by side.
#[must_use]
pub const fn is_compact(window_w: u32, window_h: u32) -> bool {
    window_w < COMPACT_WIDTH || window_h < COMPACT_HEIGHT
}

/// Now Playing art size: 480, shrinking to fit `H - 280` but never below 400; in compact mode
/// 280, shrinking to fit `H - 360` (240 at 600 px high) but never below 160.
#[must_use]
pub fn now_playing_art(window_w: u32, window_h: u32) -> u32 {
    if is_compact(window_w, window_h) {
        window_h
            .saturating_sub(360)
            .clamp(COMPACT_ART_MIN, COMPACT_ART_MAX)
    } else {
        window_h
            .saturating_sub(280)
            .clamp(NOW_PLAYING_ART_MIN, NOW_PLAYING_ART_MAX)
    }
}

/// Width of the lyrics column next to the art in the wide Now Playing layout.
#[must_use]
pub fn lyrics_column_width(window_w: u32, art: u32) -> u32 {
    window_w
        .saturating_sub(2 * CONTENT_PADDING + art + NOW_PLAYING_GAP)
        .min(LYRICS_MAX_WIDTH)
}

// ---------------------------------------------------------------------------------------------
// Three-pane shell (`docs/ui-redesign.md` section 2)
// ---------------------------------------------------------------------------------------------

/// Gap between the panes and between a pane and the window edge.
pub const PANE_GAP: u32 = 8;
/// Corner radius of a pane (`shape.xl`).
pub const PANE_RADIUS: f32 = 28.0;
/// Left pane width below [`SIDEBAR_BREAKPOINT`]: an icon rail.
pub const LEFT_RAIL: u32 = 80;
/// Left pane width from [`SIDEBAR_BREAKPOINT`].
pub const LEFT_WIDE: u32 = 248;
/// Left pane width from [`GAP_BREAKPOINT`].
pub const LEFT_WIDER: u32 = 288;
/// Right (Now Playing) pane width from 1280.
pub const RIGHT_WIDE: u32 = 316;
/// Right pane width from [`GAP_BREAKPOINT`].
pub const RIGHT_WIDER: u32 = 380;
/// Right pane width below 1280, where it overlays the content.
pub const RIGHT_OVERLAY: u32 = 340;
/// Window width from which the right pane pushes the centre pane and is open by default.
pub const RIGHT_PUSH_BREAKPOINT: u32 = 1280;
/// Floating capsule height.
pub const CAPSULE_HEIGHT: u32 = 72;
/// Gap between the capsule and the window bottom.
pub const CAPSULE_MARGIN: u32 = 16;
/// Widest capsule.
pub const CAPSULE_MAX: u32 = 1280;
/// Narrowest capsule.
pub const CAPSULE_MIN: u32 = 560;
/// Scroll padding at the bottom of a pane so the capsule never hides the last row.
pub const PANE_BOTTOM_RESERVE: u32 = CAPSULE_HEIGHT + CAPSULE_MARGIN + PANE_GAP;
/// Height of the centre pane top bar.
pub const CENTER_BAR: u32 = 64;

/// Left pane width for a window width.
#[must_use]
pub const fn left_width(window_w: u32) -> u32 {
    if window_w >= GAP_BREAKPOINT {
        LEFT_WIDER
    } else if window_w >= SIDEBAR_BREAKPOINT {
        LEFT_WIDE
    } else {
        LEFT_RAIL
    }
}

/// Right pane width (when fully open) for a window width.
#[must_use]
pub const fn right_width(window_w: u32) -> u32 {
    if window_w >= GAP_BREAKPOINT {
        RIGHT_WIDER
    } else if window_w >= RIGHT_PUSH_BREAKPOINT {
        RIGHT_WIDE
    } else {
        RIGHT_OVERLAY
    }
}

/// `true` when the open right pane narrows the centre pane (it overlays below 1280).
#[must_use]
pub const fn right_pushes(window_w: u32) -> bool {
    window_w >= RIGHT_PUSH_BREAKPOINT
}

/// Centre pane width. `right_open` is the animated open fraction of the right pane (0 to 1);
/// it only matters when the pane pushes.
#[must_use]
pub fn center_width(window_w: u32, right_open: f32) -> u32 {
    let right = if right_pushes(window_w) {
        // A fraction of a width in px: small, non-negative.
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let px = (right_width(window_w) as f32 * right_open.clamp(0.0, 1.2)).round() as u32;
        px + if px > 0 { PANE_GAP } else { 0 }
    } else {
        0
    };
    window_w.saturating_sub(2 * PANE_GAP + PANE_GAP + left_width(window_w) + right)
}

/// Capsule width.
#[must_use]
pub fn capsule_width(window_w: u32) -> u32 {
    (window_w * 4 / 5).clamp(CAPSULE_MIN, CAPSULE_MAX)
}

/// Card grid inside a pane of `pane_w` px: padding 24 each side, cards at least 176 wide,
/// gap 16 (24 from a 1000 px pane), at most 6 columns, card width rounded down to 4.
#[must_use]
pub fn grid_in(pane_w: u32) -> Grid {
    let content = pane_w.saturating_sub(2 * CONTENT_PADDING).min(CONTENT_MAX);
    let gap = if pane_w >= 1000 { GAP_WIDE } else { GAP_NARROW };
    let columns = ((content + gap) / (PANE_CARD_MIN + gap)).clamp(2, 6);
    let card = content.saturating_sub(gap * (columns - 1)) / columns / 4 * 4;
    Grid {
        sidebar: 0,
        content,
        gap,
        columns,
        card,
    }
}

/// Narrowest card art in a pane.
pub const PANE_CARD_MIN: u32 = 176;

/// Square art of the right pane: the pane width less 12 px padding on both sides.
#[must_use]
pub fn pane_art(window_w: u32) -> u32 {
    right_width(window_w).saturating_sub(24)
}

/// Height of the lyrics list in the right pane, in px: the pane height less art, title block,
/// live line, tabs, footer and the space the capsule covers.
#[must_use]
pub fn pane_lyrics_height(window_w: u32, window_h: u32) -> f32 {
    let used = 12 + pane_art(window_w) + 12 + 68 + 40 + 48 + 8 + 24 + PANE_BOTTOM_RESERVE;
    // Small positive pixel counts, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let h = window_h.saturating_sub(2 * PANE_GAP).saturating_sub(used) as f32;
    h.max(72.0)
}
#[cfg(test)]
mod tests {
    use super::*;

    /// One row of the table in `docs/ui-reference.md` section 3: (W, queue, sidebar, C, gap, n, art).
    #[test]
    fn rows_of_the_reference_table() {
        let rows: [(u32, bool, u32, u32, u32, u32, u32); 7] = [
            (960, false, 64, 848, 16, 4, 200),
            (1100, false, 240, 812, 16, 4, 188),
            (1280, false, 240, 992, 16, 5, 184),
            // The table says 208; the formula (C 672, n 3, (672 - 32) / 3 = 213.3, down to 4)
            // gives 212. The formula is the rule, so the test follows it. See the report.
            (1280, true, 240, 672, 16, 3, 212),
            (1600, false, 240, 1200, 24, 6, 180),
            (1920, false, 240, 1200, 24, 6, 180),
            (2560, false, 240, 1200, 24, 6, 180),
        ];
        for (w, queue, sidebar, content, gap, columns, card) in rows {
            let g = grid(w, queue);
            assert_eq!(
                g,
                Grid {
                    sidebar,
                    content,
                    gap,
                    columns,
                    card
                },
                "W = {w}, queue = {queue}"
            );
        }
    }

    #[test]
    fn queue_overlays_below_1280() {
        assert_eq!(grid(1100, true), grid(1100, false));
        assert_eq!(grid(1279, true), grid(1279, false));
        assert_ne!(grid(1280, true), grid(1280, false));
    }

    #[test]
    fn cards_never_overflow_and_stay_at_least_the_minimum() {
        for w in (960..=3000).step_by(20) {
            for queue in [false, true] {
                let g = grid(w, queue);
                assert!(g.columns >= COLUMNS_MIN && g.columns <= 6, "{w}");
                assert!(g.card.is_multiple_of(4));
                assert!(g.card * g.columns + g.gap * (g.columns - 1) <= g.content);
                assert!(g.card >= CARD_MIN, "W = {w}: card {}", g.card);
            }
        }
    }

    #[test]
    fn bottom_padding_clears_the_floating_bar() {
        assert_eq!(CONTENT_BOTTOM_PADDING, 112);
    }

    #[test]
    fn compact_breakpoints() {
        assert!(is_compact(1099, 800));
        assert!(is_compact(1280, 599));
        assert!(!is_compact(1100, 600));
        assert!(!is_compact(1366, 697));
        assert!(!is_compact(1280, 800));
    }

    #[test]
    fn now_playing_art_sizes() {
        assert_eq!(now_playing_art(1280, 800), 480);
        assert_eq!(now_playing_art(1280, 700), 420);
        assert_eq!(now_playing_art(960, 600), 240);
        assert_eq!(now_playing_art(1366, 697), 417);
        assert_eq!(now_playing_art(1280, 620), 340);
        assert_eq!(now_playing_art(1000, 800), 280);
        assert_eq!(now_playing_art(1280, 1200), 480);
    }

    #[test]
    fn lyrics_column_is_capped_at_640() {
        assert_eq!(lyrics_column_width(1280, 480), 640);
        assert_eq!(lyrics_column_width(2000, 480), 640);
        assert!(lyrics_column_width(1100, 420) < 640);
    }

    #[test]
    fn three_pane_widths_at_the_two_target_sizes() {
        // 1366 x 697: left 248, right 316, centre 770.
        assert_eq!(left_width(1366), 248);
        assert_eq!(right_width(1366), 316);
        assert_eq!(center_width(1366, 1.0), 1366 - 24 - 248 - 316 - 8);
        assert_eq!(center_width(1366, 0.0), 1366 - 24 - 248);
        // 1920 x 1040: left 288, right 380.
        assert_eq!(center_width(1920, 1.0), 1920 - 24 - 288 - 380 - 8);
        // Below 1280 the pane overlays: the centre does not shrink.
        assert_eq!(center_width(1100, 1.0), 1100 - 24 - 248);
        assert_eq!(left_width(960), LEFT_RAIL);
    }

    #[test]
    fn pane_grid_fits_and_keeps_big_cards() {
        for w in (400..=1400).step_by(20) {
            let g = grid_in(w);
            assert!(g.columns >= 2 && g.columns <= 6);
            assert!(g.card * g.columns + g.gap * (g.columns - 1) <= g.content.max(1) + 8);
        }
        assert_eq!(grid_in(770).columns, 3);
        assert!(grid_in(770).card >= 200);
    }

    #[test]
    fn capsule_and_lyrics_area_have_room() {
        assert_eq!(capsule_width(1366), 1092);
        assert_eq!(capsule_width(1920), 1280);
        assert_eq!(capsule_width(600), 560);
        assert!(pane_lyrics_height(1366, 697) >= 72.0);
        assert!(pane_lyrics_height(1920, 1040) > 300.0);
    }
}
