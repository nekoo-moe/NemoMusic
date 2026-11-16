//! Per-word and per-syllable motion of word- and syllable-timed lyrics (`syllable-motion` spec,
//! an owner extension of `design.md` section 10). Pure: it splits the active line into its timed
//! pieces and says, for a playback position, which pieces are unsung (grey), being sung (a
//! left-to-right fill from grey to bright over the piece's own start and end time, with a small
//! lift, glow and, for long held syllables, a scale) and sung (bright). Drawing lives in
//! `gui::syllable_text`.

use std::ops::Range;

use nemo_core::lyrics::LyricLine;

/// The shortest fade of one piece, ms.
pub const MIN_FADE_MS: u64 = 100;
/// Fade length of the last piece when the provider gives no end time, ms.
const LAST_PIECE_MS: u64 = 600;

/// The texts of the timed pieces of `line`, covering the whole line text. Each piece is found in
/// `line.text` in order and runs to the start of the next one, so the spaces between words stay
/// exactly as the provider wrote them. When a piece cannot be found (the provider's word texts
/// differ from the line text) the word texts are used as they are, joined without a separator.
#[must_use]
pub fn texts(line: &LyricLine) -> Vec<String> {
    let text = line.text.as_str();
    let mut starts = Vec::with_capacity(line.words.len());
    let mut cursor = 0;
    for w in &line.words {
        let needle = w.text.trim();
        let found = if needle.is_empty() {
            Some(cursor)
        } else {
            text.get(cursor..)
                .and_then(|rest| rest.find(needle))
                .map(|p| cursor + p)
        };
        let Some(at) = found else {
            return line.words.iter().map(|w| w.text.clone()).collect();
        };
        starts.push(at);
        cursor = at + needle.len();
    }
    if let Some(first) = starts.first_mut() {
        // Leading spaces belong to the first piece.
        *first = 0;
    }
    let mut out = Vec::with_capacity(starts.len());
    for (i, s) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(text.len());
        out.push(text.get(*s..end).unwrap_or_default().to_owned());
    }
    out
}

/// Alpha of an unsung word: `on-surface-variant` at this opacity, clearly grey (owner override
/// of `design.md` section 10, 2026-11-16: it was 0.55). A sung word is full `on-surface`.
pub const UNSUNG_ALPHA: f32 = 0.38;
/// Highest lift of the word being sung, px (the motion rule allows 16).
pub const LIFT_PX: f32 = 2.0;
/// The lift and glow ease in over this long after the word starts, ms (`motion.fast`).
pub const RISE_MS: f32 = 100.0;
/// The lift and glow ease out over this long after the word ends, ms.
pub const FALL_MS: f32 = 200.0;
/// [`FALL_MS`] as whole ms.
const FALL_TAIL_MS: i64 = 200;
/// A syllable at least this long gets the held-syllable scale, ms.
pub const HELD_MS: u64 = 1000;
/// Largest scale of a held syllable.
pub const HELD_SCALE: f32 = 1.04;
/// A held syllable reaches [`HELD_SCALE`] at this length, ms.
const HELD_FULL_MS: u64 = 3000;

/// Start and end of one timed piece, ms of song time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// When the piece starts.
    pub start_ms: i64,
    /// When it ends (at least [`MIN_FADE_MS`] after the start).
    pub end_ms: i64,
}

impl Timing {
    /// Length in ms.
    #[must_use]
    pub fn len_ms(self) -> i64 {
        self.end_ms - self.start_ms
    }
}

/// Where a piece is for a playback position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordState {
    /// Not started: grey.
    Unsung,
    /// Between its start and its end: the fill runs.
    Singing,
    /// Past its end: bright.
    Sung,
}

/// Draw values of one piece at one position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WordFx {
    /// Share of the piece that is bright, 0 to 1, left to right (right to left for RTL text).
    pub fill: f32,
    /// Lift, px, upward.
    pub lift: f32,
    /// Scale around the piece's centre.
    pub scale: f32,
    /// Glow strength, 0 to 1.
    pub glow: f32,
}

/// Which pieces to draw how at one position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Pieces before this index are plain bright text.
    pub sung: usize,
    /// Pieces drawn on their own (singing, or still easing out); the rest are grey.
    pub anim: Range<usize>,
}

/// The timing of every piece of `line`. A missing end is the next start; the last piece ends
/// with the line, or [`LAST_PIECE_MS`] after its start.
#[must_use]
pub fn timings(line: &LyricLine) -> Vec<Timing> {
    let n = line.words.len();
    line.words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let next = line.words.get(i + 1).map(|n| n.start_ms);
            let fallback_end = if i + 1 == n {
                line.end_ms.unwrap_or(w.start_ms + LAST_PIECE_MS)
            } else {
                next.unwrap_or(w.start_ms + LAST_PIECE_MS)
            };
            let end = w.end_ms.unwrap_or(fallback_end);
            let start = i64::try_from(w.start_ms).unwrap_or(i64::MAX / 2);
            let len = i64::try_from(end.saturating_sub(w.start_ms).max(MIN_FADE_MS))
                .unwrap_or(i64::MAX / 2);
            Timing {
                start_ms: start,
                end_ms: start.saturating_add(len),
            }
        })
        .collect()
}

/// State of a piece at `t_ms`.
#[must_use]
pub fn state_at(t_ms: i64, w: Timing) -> WordState {
    if t_ms < w.start_ms {
        WordState::Unsung
    } else if t_ms < w.end_ms {
        WordState::Singing
    } else {
        WordState::Sung
    }
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Share of a piece that is bright at `t_ms`: 0 before it starts, 1 from its end, in between
/// the eased `(t - start) / (end - start)`.
#[must_use]
pub fn fill_at(t_ms: i64, w: Timing) -> f32 {
    match state_at(t_ms, w) {
        WordState::Unsung => 0.0,
        WordState::Sung => 1.0,
        // At most one line long: exact enough in f32.
        #[allow(clippy::cast_precision_loss)]
        WordState::Singing => smooth((t_ms - w.start_ms) as f32 / w.len_ms() as f32),
    }
}

/// Strength of the lift and glow: 0 before the piece, up to 1 over [`RISE_MS`] after its start,
/// and back to 0 over [`FALL_MS`] after its end.
#[must_use]
#[allow(clippy::cast_precision_loss)] // a few seconds in ms
pub fn emphasis_at(t_ms: i64, w: Timing) -> f32 {
    if t_ms < w.start_ms {
        return 0.0;
    }
    let rise = smooth((t_ms - w.start_ms) as f32 / RISE_MS);
    if t_ms < w.end_ms {
        return rise;
    }
    rise * (1.0 - smooth((t_ms - w.end_ms) as f32 / FALL_MS))
}

/// Scale at full emphasis of a piece `len_ms` long: 1 below [`HELD_MS`], then from 1.01 up to
/// [`HELD_SCALE`] at three seconds.
#[must_use]
#[allow(clippy::cast_precision_loss)] // a few seconds in ms
pub fn held_scale(len_ms: i64) -> f32 {
    let Ok(len) = u64::try_from(len_ms) else {
        return 1.0;
    };
    if len < HELD_MS {
        return 1.0;
    }
    let x = (len - HELD_MS) as f32 / (HELD_FULL_MS - HELD_MS) as f32;
    1.0 + (HELD_SCALE - 1.0) * (0.25 + 0.75 * smooth(x))
}

/// Draw values of a piece at `t_ms`. With `reduced` motion there is no lift, scale, glow or
/// wipe: the piece is bright from its start (a stepwise colour change per piece).
#[must_use]
pub fn fx_at(t_ms: i64, w: Timing, reduced: bool) -> WordFx {
    if reduced {
        return WordFx {
            fill: if t_ms >= w.start_ms { 1.0 } else { 0.0 },
            lift: 0.0,
            scale: 1.0,
            glow: 0.0,
        };
    }
    let e = emphasis_at(t_ms, w);
    WordFx {
        fill: fill_at(t_ms, w),
        lift: LIFT_PX * e,
        scale: 1.0 + (held_scale(w.len_ms()) - 1.0) * e,
        glow: e,
    }
}

/// Which pieces are plain, drawn on their own or grey at `t_ms`. Reduced motion draws nothing
/// on its own: a piece turns bright when it starts.
#[must_use]
pub fn frame(timings: &[Timing], t_ms: i64, reduced: bool) -> Frame {
    if reduced {
        let sung = timings.iter().take_while(|w| t_ms >= w.start_ms).count();
        return Frame {
            sung,
            anim: sung..sung,
        };
    }
    let settled = |w: &Timing| t_ms >= w.end_ms.saturating_add(FALL_TAIL_MS);
    let sung = timings.iter().take_while(|w| settled(w)).count();
    let started = timings.iter().rposition(|w| t_ms >= w.start_ms);
    let end = started.map_or(sung, |i| (i + 1).max(sung));
    Frame {
        sung,
        anim: sung..end,
    }
}

/// `true` when the first letter with a direction in `text` reads right to left (Hebrew, Arabic
/// and their relatives), so the fill runs from the right.
#[must_use]
pub fn is_rtl(text: &str) -> bool {
    text.chars()
        .find(|c| c.is_alphabetic())
        .is_some_and(|c| matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nemo_core::lyrics::LyricWord;

    fn line(text: &str, words: &[(&str, u64, Option<u64>)]) -> LyricLine {
        LyricLine {
            start_ms: words.first().map(|w| w.1),
            end_ms: Some(5_000),
            text: text.to_owned(),
            words: words
                .iter()
                .map(|(t, s, e)| LyricWord {
                    start_ms: *s,
                    end_ms: *e,
                    text: (*t).to_owned(),
                })
                .collect(),
        }
    }

    #[test]
    fn syllables_of_one_word_stay_joined_and_cover_the_line() {
        let l = line(
            "beau-tiful day",
            &[
                ("beau", 1000, None),
                ("-ti", 1200, None),
                ("ful ", 1400, None),
                ("day", 1800, None),
            ],
        );
        let t = texts(&l);
        assert_eq!(t, ["beau", "-ti", "ful ", "day"]);
        assert_eq!(t.concat(), l.text);
    }

    #[test]
    fn word_texts_without_spaces_keep_the_line_spaces() {
        let l = line(
            "la  la, la",
            &[("la", 0, None), ("la,", 500, None), ("la", 900, None)],
        );
        assert_eq!(texts(&l).concat(), "la  la, la");
        // Texts that do not match fall back to the pieces themselves.
        let odd = line("other text", &[("x ", 0, None), ("y", 500, None)]);
        assert_eq!(texts(&odd), ["x ", "y"]);
    }

    fn tm(start: i64, end: i64) -> Timing {
        Timing {
            start_ms: start,
            end_ms: end,
        }
    }

    #[test]
    fn word_state_at_the_edges() {
        let w = tm(1000, 1400);
        assert_eq!(state_at(999, w), WordState::Unsung);
        assert_eq!(state_at(1000, w), WordState::Singing);
        assert_eq!(state_at(1399, w), WordState::Singing);
        assert_eq!(state_at(1400, w), WordState::Sung);
    }

    #[test]
    fn fill_runs_from_zero_to_one_over_the_word() {
        let w = tm(1000, 1400);
        assert!(fill_at(999, w).abs() < 1e-6, "just before");
        assert!(fill_at(1000, w).abs() < 1e-6, "start");
        assert!((fill_at(1200, w) - 0.5).abs() < 1e-6, "middle");
        assert!(fill_at(1300, w) > 0.5 && fill_at(1300, w) < 1.0);
        assert!((fill_at(1400, w) - 1.0).abs() < 1e-6, "end");
        assert!((fill_at(9000, w) - 1.0).abs() < 1e-6);
        let mut last = 0.0;
        for t in 1000..=1400 {
            let f = fill_at(t, w);
            assert!(f >= last, "monotonic at {t}");
            last = f;
        }
    }

    #[test]
    fn held_syllables_scale_up_to_a_cap() {
        assert!((held_scale(999) - 1.0).abs() < 1e-6);
        assert!(held_scale(1000) > 1.0 && held_scale(1000) < 1.02);
        assert!(held_scale(2000) > held_scale(1000));
        assert!((held_scale(3000) - HELD_SCALE).abs() < 1e-6);
        assert!((held_scale(60_000) - HELD_SCALE).abs() < 1e-6);
        assert!((held_scale(-5) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn lift_eases_in_over_100_ms_and_out_over_200_ms() {
        let w = tm(1000, 1600);
        assert!(emphasis_at(999, w).abs() < 1e-6);
        assert!(emphasis_at(1050, w) > 0.0 && emphasis_at(1050, w) < 1.0);
        assert!((emphasis_at(1100, w) - 1.0).abs() < 1e-6);
        assert!((emphasis_at(1599, w) - 1.0).abs() < 1e-6);
        assert!(emphasis_at(1700, w) > 0.0 && emphasis_at(1700, w) < 1.0);
        assert!(emphasis_at(1800, w).abs() < 1e-6);
        let fx = fx_at(1300, w, false);
        assert!((fx.lift - LIFT_PX).abs() < 1e-6 && LIFT_PX <= 16.0);
        assert!(
            fx.glow > 0.99 && (fx.scale - 1.0).abs() < 1e-6,
            "600 ms: no scale"
        );
        let held = fx_at(2000, tm(1000, 3500), false);
        assert!(held.scale > 1.0 && held.scale <= HELD_SCALE);
    }

    #[test]
    fn reduced_motion_is_a_stepwise_colour_change() {
        let w = tm(1000, 1400);
        let before = fx_at(999, w, true);
        let during = fx_at(1001, w, true);
        let after = fx_at(2000, w, true);
        assert!(before.fill.abs() < f32::EPSILON);
        for fx in [during, after] {
            assert!((fx.fill - 1.0).abs() < f32::EPSILON);
            assert!(fx.lift.abs() < f32::EPSILON && fx.glow.abs() < f32::EPSILON);
            assert!((fx.scale - 1.0).abs() < f32::EPSILON);
        }
        let ws = [tm(0, 500), tm(500, 900), tm(900, 1200)];
        let f = frame(&ws, 600, true);
        assert_eq!((f.sung, f.anim), (2, 2..2));
    }

    #[test]
    fn frame_splits_sung_animated_and_grey() {
        let ws = [tm(0, 500), tm(500, 900), tm(900, 1200), tm(1200, 1500)];
        let f = frame(&ws, 600, false);
        assert_eq!((f.sung, f.anim), (0, 0..2), "word 0 still eases out");
        let f = frame(&ws, 800, false);
        assert_eq!((f.sung, f.anim), (1, 1..2));
        assert_eq!(frame(&ws, -5, false).anim, 0..0);
        assert_eq!(frame(&ws, 9000, false).sung, 4);
        assert_eq!(frame(&ws, 9000, false).anim, 4..4);
    }

    #[test]
    fn timings_use_the_next_start_and_a_minimum_length() {
        let l = line(
            "a b c",
            &[("a ", 0, None), ("b ", 1000, Some(1030)), ("c", 2000, None)],
        );
        let t = timings(&l);
        assert_eq!(t[0], tm(0, 1000));
        assert_eq!(t[1], tm(1000, 1100), "a 30 ms word still takes 100 ms");
        assert_eq!(t[2], tm(2000, 5000), "the last one runs to the line end");
    }

    #[test]
    fn unsung_grey_is_dimmer_than_sung_but_above_inactive_lines() {
        use crate::theme::{Neutrals, contrast_ratio};
        for n in [Neutrals::DARK, Neutrals::AMOLED] {
            let grey = n
                .on_surface_variant
                .with_alpha(UNSUNG_ALPHA)
                .over(n.surface);
            let c = contrast_ratio(grey, n.surface);
            // Owner override (2026-11-16): 0.38 reads clearly grey; it is about 2.2:1 on the
            // dark surfaces, below the 3:1 large-text rule (recorded in the spec and the PR).
            assert!(c >= 2.0, "unsung grey {c:.2}:1");
            let sung = contrast_ratio(n.on_surface.over(n.surface), n.surface);
            assert!(sung >= 4.5 && sung > c, "sung {sung:.2}:1");
            let next = n
                .on_surface_variant
                .with_alpha(crate::lyrics_view::line_alpha(1))
                .over(n.surface);
            assert!(contrast_ratio(next, n.surface) < c, "next line is dimmer");
        }
    }

    #[test]
    fn rtl_text_is_found_by_its_first_letter() {
        assert!(is_rtl("שלום עולם"));
        assert!(is_rtl("  مرحبا"));
        assert!(!is_rtl("hello שלום"));
        assert!(!is_rtl("Tiếng Việt 你好"));
    }
}
