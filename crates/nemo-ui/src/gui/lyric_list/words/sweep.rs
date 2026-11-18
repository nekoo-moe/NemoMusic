//! One moving piece of a timed line: its halo, its grey and bright ink with the soft edge of the
//! sweep, its wobble, and for held pieces the wave and swell of its letters.

use iced::advanced::Renderer as _;
use iced::advanced::text::Renderer as _;
use iced::{Color, Point, Rectangle, Transformation};

use super::super::paint::with_alpha;
use super::super::shape::Word;
use super::{Paint, START_PAD};
use crate::lyric_pieces::motion::{self, SOFT_EDGE, WordFx};

/// Offset copies that make the halo: `(dx, dy)` in em and the alpha of the copy. Four close ones
/// and four wider ones, so the halo falls off softly.
const HALO: [(f32, f32, f32); 8] = [
    (0.035, 0.0, 0.10),
    (-0.035, 0.0, 0.10),
    (0.0, 0.035, 0.10),
    (0.0, -0.035, 0.10),
    (0.07, 0.07, 0.06),
    (-0.07, 0.07, 0.06),
    (0.07, -0.07, 0.06),
    (-0.07, -0.07, 0.06),
];
/// Reach of the halo around the swept part, em.
const HALO_REACH_EM: f32 = 0.12;
/// Most strips the soft edge is cut into, and the fewest.
const STRIPS: (f32, f32) = (2.0, 6.0);
/// Width of one strip of the soft edge, px.
const STRIP_PX: f32 = 2.0;

/// Draw the moving piece `word`.
pub(super) fn moving(r: &mut iced::Renderer, paint: &Paint<'_>, word: &Word) {
    let fx = motion::word_fx(paint.t_ms, word.timing, paint.reduced);
    let Some(first) = word.boxes.first() else {
        return;
    };
    let hull = word
        .boxes
        .iter()
        .fold(paint.at(first), |acc, b| acc.union(&paint.at(b)));
    let wobble = fx.wobble;
    if wobble.shift_em.abs() < 1e-4 && (wobble.scale - 1.0).abs() < 1e-4 {
        paint_word(r, paint, word, &fx);
        return;
    }
    let (cx, cy) = (hull.x + hull.width / 2.0, hull.y + hull.height / 2.0);
    let xf = Transformation::translate(wobble.shift_em * paint.size, 0.0)
        * Transformation::translate(cx, cy)
        * Transformation::scale(wobble.scale)
        * Transformation::translate(-cx, -cy);
    r.with_transformation(xf, |r| paint_word(r, paint, word, &fx));
}

fn paint_word(r: &mut iced::Renderer, paint: &Paint<'_>, word: &Word, fx: &WordFx) {
    let swept = (fx.edge * word.total).clamp(0.0, word.total);
    if fx.glow * paint.bright > 0.02 && swept > 0.0 {
        halo(r, paint, word, swept, fx.glow);
    }
    if word.letters.is_empty() || paint.reduced {
        ink(r, paint, word, fx.edge);
    } else {
        letters(r, paint, word, fx.edge);
    }
}

/// Call `f` with the rectangle of every row of `word` between `a` and `b` px along the piece, from
/// its start (the right for right-to-left text).
fn for_each_range(
    paint: &Paint<'_>,
    word: &Word,
    (from, to): (f32, f32),
    mut each: impl FnMut(Rectangle),
) {
    let mut acc = 0.0;
    for bx in &word.boxes {
        let abs = paint.at(bx);
        let (lo, hi) = (from.max(acc), to.min(acc + abs.width));
        if hi > lo {
            let (x0, x1) = if paint.line.rtl {
                (
                    abs.x + abs.width - (hi - acc),
                    abs.x + abs.width - (lo - acc),
                )
            } else {
                (abs.x + (lo - acc), abs.x + (hi - acc))
            };
            // Room before the piece, where its side bearing is; none past it.
            let lead = if lo <= acc + 0.01 { START_PAD } else { 0.0 };
            let (x0, x1) = if paint.line.rtl {
                (x0, x1 + lead)
            } else {
                (x0 - lead, x1)
            };
            each(paint.roomy(Rectangle {
                x: x0,
                y: abs.y,
                width: x1 - x0,
                height: abs.height,
            }));
        }
        acc += abs.width;
    }
}

fn fill(r: &mut iced::Renderer, paint: &Paint<'_>, color: Color, rect: Rectangle, shift: Point) {
    if let Some(c) = rect.intersection(&paint.clip) {
        r.fill_paragraph(
            &paint.line.para,
            Point::new(paint.pos.x + shift.x, paint.pos.y + shift.y),
            color,
            c,
        );
    }
}

/// The halo: offset copies of the swept part at a low alpha, under the ink.
fn halo(r: &mut iced::Renderer, paint: &Paint<'_>, word: &Word, swept: f32, strength: f32) {
    let reach = HALO_REACH_EM * paint.size;
    for (dx, dy, alpha) in HALO {
        let shift = Point::new(dx * paint.size, dy * paint.size);
        let color = with_alpha(paint.ink, alpha * strength * paint.bright);
        for_each_range(paint, word, (0.0, swept), |rect| {
            let grown = Rectangle {
                x: rect.x - reach,
                y: rect.y - reach,
                width: rect.width + 2.0 * reach,
                height: rect.height + 2.0 * reach,
            };
            fill(r, paint, color, grown, shift);
        });
    }
}

/// Grey ink for the whole piece, then bright ink behind the soft edge: a hard part and strips
/// with falling alpha. `edge` is in piece widths.
fn ink(r: &mut iced::Renderer, paint: &Paint<'_>, word: &Word, edge: f32) {
    let grey = with_alpha(paint.ink, paint.grey);
    let origin = Point::ORIGIN;
    for_each_range(paint, word, (0.0, word.total), |rect| {
        fill(r, paint, grey, rect, origin);
    });
    let hard = ((edge - SOFT_EDGE) * word.total).clamp(0.0, word.total);
    let tip = (edge * word.total).clamp(0.0, word.total);
    let bright = with_alpha(paint.ink, paint.bright);
    if hard > 0.0 {
        for_each_range(paint, word, (0.0, hard), |rect| {
            fill(r, paint, bright, rect, origin);
        });
    }
    if tip <= hard {
        return;
    }
    // The strips share the part between the hard part and the tip; far fewer for a short word.
    let n = ((tip - hard) / STRIP_PX).round().clamp(STRIPS.0, STRIPS.1);
    let step = (tip - hard) / n;
    let mut k = 0.0;
    while k < n {
        let alpha = 1.0 - (k + 0.5) / n;
        let color = with_alpha(paint.ink, paint.bright * alpha);
        let range = (hard + k * step, hard + (k + 1.0) * step);
        for_each_range(paint, word, range, |rect| {
            fill(r, paint, color, rect, origin);
        });
        k += 1.0;
    }
}

/// The letters of a held piece, each on its own: lifted and swollen by its wave, grey with the
/// bright copy on top by how far the edge has passed it.
fn letters(r: &mut iced::Renderer, paint: &Paint<'_>, word: &Word, edge: f32) {
    let Some(first) = word.boxes.first() else {
        return;
    };
    let base = paint.at(first);
    let n = word.letters.len();
    let total = word.total.max(1.0);
    for (i, letter) in word.letters.iter().enumerate() {
        let wave = motion::letter_at(i, n, paint.t_ms, word.timing);
        let at = Point::new(base.x + letter.x, base.y - wave.lift_em * paint.size);
        let cover = motion::coverage((letter.x + letter.width / 2.0) / total, edge);
        let grey = with_alpha(paint.ink, paint.grey);
        let bright = with_alpha(paint.ink, paint.bright * cover);
        let draw = |r: &mut iced::Renderer| {
            r.fill_paragraph(&letter.para, at, grey, paint.clip);
            if cover > 0.0 {
                r.fill_paragraph(&letter.para, at, bright, paint.clip);
            }
        };
        if (wave.scale - 1.0).abs() < 1e-3 {
            draw(r);
            continue;
        }
        let (cx, cy) = (at.x + letter.width / 2.0, at.y + paint.line_h / 2.0);
        let xf = Transformation::translate(cx, cy)
            * Transformation::scale(wave.scale)
            * Transformation::translate(-cx, -cy);
        r.with_transformation(xf, draw);
    }
}
