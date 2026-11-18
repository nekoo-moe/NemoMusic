//! What one timed piece looks like at a playback position (`lyrics-rebuild` spec, after Better
//! Lyrics). Pure: time is passed in, every number is a function of `(t, timing)`.
//!
//! - **Sweep**: a soft edge crosses the piece from `-0.2` to `1.4` piece widths. It starts half a
//!   piece length before the piece and lasts two lengths, so the edge is at the end of the piece
//!   when the piece ends, and the rest is the tail moving past. Bright ink is behind the edge,
//!   grey ahead of it, and the edge is 0.1 piece wide.
//! - **Glow**: a halo that grows as the edge arrives and fades after the piece ends.
//! - **Wobble**: the whole piece shifts and stretches a little for one second.
//! - **Letter wave and swell**: pieces held for [`LONG_MS`] lift their letters one after the
//!   other; pieces held for [`SWELL_MS`] also swell them.

use super::Timing;
use crate::motion::cubic_bezier;

/// The sweep starts this share of the piece length before the piece.
pub const LEAD_RATIO: f32 = 0.5;
/// The sweep lasts this many piece lengths.
pub const SWIPE_RATIO: f32 = 2.0;
/// Where the edge of the sweep starts, in piece widths from the start of the piece.
pub const EDGE_FROM: f32 = -0.2;
/// Where the edge of the sweep ends, in piece widths.
pub const EDGE_TO: f32 = 1.4;
/// Width of the soft edge, in piece widths.
pub const SOFT_EDGE: f32 = 0.1;
/// The glow fades over this many piece lengths after the piece, at least [`GLOW_FADE_MIN_MS`].
pub const GLOW_FADE_RATIO: f32 = 1.2;
/// Shortest glow fade, ms.
pub const GLOW_FADE_MIN_MS: f32 = 1200.0;
/// Length of the wobble, ms.
pub const WOBBLE_MS: f32 = 1000.0;
/// Shift of the wobble at its peak, em.
pub const WOBBLE_SHIFT_EM: f32 = 0.05;
/// Scale of the wobble at its peak.
pub const WOBBLE_SCALE: f32 = 1.025;
/// A piece held this long gets the letter wave, ms.
pub const LONG_MS: i64 = 1500;
/// A piece held this long also gets the swell, ms.
pub const SWELL_MS: i64 = 2000;
/// Length of one letter's wave, ms.
pub const WAVE_MS: f32 = 900.0;
/// Lift of a letter at the crest, em.
pub const WAVE_CREST_EM: f32 = 0.05;
/// Lift of a letter once it settles, em.
pub const WAVE_SETTLE_EM: f32 = 0.02;
/// Scale of a swollen letter at the crest.
pub const SWELL_CREST: f32 = 1.11;
/// Letters and highlights return to rest over this long after the piece ends, ms.
pub const FADE_OUT_MS: f32 = 500.0;
/// The wave starts at the last letter this share of the piece length after the first.
const WAVE_SPREAD: f32 = 0.6;
/// The crest comes at this share of one letter's wave.
const CREST_AT: f32 = 0.4;

/// Milliseconds as f32 (a few seconds: exact).
fn ms(d: i64) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let v = d.clamp(-100_000_000, 100_000_000) as f32;
    v
}

/// A small count as f32.
fn count(i: usize) -> f32 {
    f32::from(u16::try_from(i).unwrap_or(u16::MAX))
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// CSS `ease`.
fn ease(x: f32) -> f32 {
    cubic_bezier(0.25, 0.1, 0.25, 1.0, x)
}

/// Where a piece is: not reached by its sweep, in motion, or finished with all of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordState {
    /// The sweep has not reached it: grey, no motion.
    Upcoming,
    /// The sweep, the glow, the wobble or the letter wave is running.
    Active,
    /// Everything has settled: plain bright ink.
    Past,
}

/// When nothing of the piece moves any more, ms of song time.
#[must_use]
pub fn settle_ms(w: Timing) -> i64 {
    let len = w.len_ms();
    // Integer maths: 1.5 lengths for the sweep, 1.2 lengths (at least 1.2 s) for the glow, the
    // wave (60% of a length plus 0.9 s) and then the fade-out.
    let sweep = w.start_ms + len * 3 / 2;
    let wobble = w.start_ms + 1000;
    let glow = w.end_ms + (len * 6 / 5).max(1200);
    let wave = if len >= LONG_MS {
        w.end_ms.max(w.start_ms + len * 3 / 5 + 900) + 500
    } else {
        0
    };
    sweep.max(wobble).max(glow).max(wave)
}

/// State of a piece at `t_ms`. With `reduced` motion a piece is `Upcoming` until it starts and
/// `Past` after: it turns bright when it starts.
#[must_use]
pub fn state_at(t_ms: i64, w: Timing, reduced: bool) -> WordState {
    if reduced {
        return if t_ms >= w.start_ms {
            WordState::Past
        } else {
            WordState::Upcoming
        };
    }
    if t_ms < w.start_ms - w.len_ms() / 2 {
        WordState::Upcoming
    } else if t_ms >= settle_ms(w) {
        WordState::Past
    } else {
        WordState::Active
    }
}

/// Position of the sweep edge in piece widths from the start of the piece (`EDGE_FROM` to
/// `EDGE_TO`), linear in time.
#[must_use]
pub fn sweep_edge(t_ms: i64, w: Timing) -> f32 {
    let len = ms(w.len_ms());
    let since = ms(t_ms - w.start_ms) + LEAD_RATIO * len;
    let p = (since / (SWIPE_RATIO * len)).clamp(0.0, 1.0);
    EDGE_FROM + (EDGE_TO - EDGE_FROM) * p
}

/// Share of bright ink at `x` piece widths from the start of the piece when the edge is at
/// `edge`: 1 behind the soft edge, 0 ahead of it.
#[must_use]
pub fn coverage(x: f32, edge: f32) -> f32 {
    ((edge - x) / SOFT_EDGE).clamp(0.0, 1.0)
}

/// Strength of the glow, 0 to 1: it grows as the edge enters the piece, and fades over
/// `max(1.2 lengths, 1.2 s)` after the piece ends, with `ease`.
#[must_use]
pub fn glow_at(t_ms: i64, w: Timing) -> f32 {
    let rise = smooth(sweep_edge(t_ms, w) / 0.4);
    let fade = (GLOW_FADE_RATIO * ms(w.len_ms())).max(GLOW_FADE_MIN_MS);
    let after = ms(t_ms - w.end_ms);
    let fall = if after <= 0.0 {
        1.0
    } else {
        1.0 - ease(after / fade)
    };
    rise * fall
}

/// The wobble of a whole piece.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wobble {
    /// Shift to the right, em.
    pub shift_em: f32,
    /// Scale (applied around the centre of the piece).
    pub scale: f32,
}

/// The wobble at `t_ms`: from rest to the peak at 12.5% of its second, back to rest at 75%.
#[must_use]
pub fn wobble_at(t_ms: i64, w: Timing) -> Wobble {
    let x = ms(t_ms - w.start_ms) / WOBBLE_MS;
    let k = if !(0.0..1.0).contains(&x) {
        0.0
    } else if x < 0.125 {
        smooth(x / 0.125)
    } else if x < 0.75 {
        1.0 - smooth((x - 0.125) / 0.625)
    } else {
        0.0
    };
    Wobble {
        shift_em: WOBBLE_SHIFT_EM * k,
        scale: 1.0 + (WOBBLE_SCALE - 1.0) * k,
    }
}

/// `true` for a piece held long enough for the letter wave.
#[must_use]
pub fn is_long(w: Timing) -> bool {
    w.len_ms() >= LONG_MS
}

/// One letter of a long piece.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LetterFx {
    /// Lift, em, upward.
    pub lift_em: f32,
    /// Scale around the centre of the letter.
    pub scale: f32,
}

/// Letter `i` of `n` at `t_ms`: lifts to [`WAVE_CREST_EM`] and settles at [`WAVE_SETTLE_EM`],
/// each letter a little after the one before it, and returns to rest over [`FADE_OUT_MS`] after
/// the piece ends. A piece held [`SWELL_MS`] or more also swells the letter.
#[must_use]
pub fn letter_at(i: usize, n: usize, t_ms: i64, w: Timing) -> LetterFx {
    let len = ms(w.len_ms());
    let delay = count(i) / count(n.max(1)) * WAVE_SPREAD * len;
    let tau = (ms(t_ms - w.start_ms) - delay) / WAVE_MS;
    let rest = LetterFx {
        lift_em: 0.0,
        scale: 1.0,
    };
    if tau <= 0.0 {
        return rest;
    }
    let settled = WAVE_SETTLE_EM / WAVE_CREST_EM;
    let crest = if tau < CREST_AT {
        smooth(tau / CREST_AT)
    } else if tau < 1.0 {
        1.0 - (1.0 - settled) * smooth((tau - CREST_AT) / (1.0 - CREST_AT))
    } else {
        settled
    };
    let after = ms(t_ms - w.end_ms);
    let out = if after <= 0.0 {
        1.0
    } else {
        1.0 - smooth(after / FADE_OUT_MS)
    };
    let k = crest * out;
    let swell = if w.len_ms() >= SWELL_MS {
        SWELL_CREST - 1.0
    } else {
        0.0
    };
    LetterFx {
        lift_em: WAVE_CREST_EM * k,
        scale: 1.0 + swell * k,
    }
}

/// Everything about a piece at one position that the list draws.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WordFx {
    /// The sweep edge, piece widths.
    pub edge: f32,
    /// Glow strength, 0 to 1.
    pub glow: f32,
    /// The wobble.
    pub wobble: Wobble,
}

/// The look of a piece at `t_ms`. With `reduced` motion there is no soft sweep, glow or wobble:
/// the piece is all bright from its start.
#[must_use]
pub fn word_fx(t_ms: i64, w: Timing, reduced: bool) -> WordFx {
    if reduced {
        return WordFx {
            edge: if t_ms >= w.start_ms {
                EDGE_TO
            } else {
                EDGE_FROM
            },
            glow: 0.0,
            wobble: wobble_rest(),
        };
    }
    WordFx {
        edge: sweep_edge(t_ms, w),
        glow: glow_at(t_ms, w),
        wobble: wobble_at(t_ms, w),
    }
}

fn wobble_rest() -> Wobble {
    Wobble {
        shift_em: 0.0,
        scale: 1.0,
    }
}

#[cfg(test)]
mod tests;
