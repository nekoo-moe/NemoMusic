//! Drawing a word- or syllable-timed line (`lyrics-rebuild` spec). iced 0.14 has no gradient text
//! fill and no blur, so the line paragraph is drawn several times with clip rectangles:
//!
//! - the whole line in grey, with a hole where a piece is moving;
//! - the pieces that are done in bright ink, merged into one clip per row;
//! - every moving piece on its own (see `sweep`): grey, then bright behind the soft edge (a hard
//!   part and a few thin strips with falling alpha), under a halo of offset copies, wobbling.
//!
//! Which piece is which comes from `lyric_pieces::motion::state_at`, so a piece that is upcoming
//! or settled costs nothing but its share of the base and the merged clip.

use iced::advanced::text::Renderer as _;
use iced::{Color, Point, Rectangle};

use super::paint::with_alpha;
use super::shape::{Line, Word};
use crate::lyric_pieces::motion::{WordState, state_at};

mod clips;
mod sweep;

use clips::Clips;

/// Most moving pieces drawn on their own at once; any more are drawn plain bright.
const MAX_MOVING: usize = 8;
/// A moving piece is cut out of the base with this much room above and below, in line heights.
pub(super) const VERTICAL_ROOM: f32 = 0.25;
/// Room kept before the first piece of a row so its side bearing is not cut, px.
pub(super) const START_PAD: f32 = 2.0;

/// What a timed line is drawn with.
pub(super) struct Paint<'a> {
    /// The shaped line.
    pub line: &'a Line,
    /// Top left of the paragraph.
    pub pos: Point,
    /// The list's bounds: nothing is drawn outside.
    pub clip: Rectangle,
    /// The ink at full alpha.
    pub ink: Color,
    /// Alpha of bright ink: how lit the line is, times the edge fade.
    pub bright: f32,
    /// Alpha of unsung ink: the unlit alpha times the edge fade.
    pub grey: f32,
    /// The audio clock, ms.
    pub t_ms: i64,
    /// Reduced motion: no sweep, glow or wobble.
    pub reduced: bool,
    /// Font size, px.
    pub size: f32,
    /// Row height, px.
    pub line_h: f32,
}

impl Paint<'_> {
    /// A box of the paragraph in list coordinates.
    pub(super) fn at(&self, b: &Rectangle) -> Rectangle {
        Rectangle {
            x: self.pos.x + b.x,
            y: self.pos.y + b.y,
            ..*b
        }
    }

    /// `r` taller by [`VERTICAL_ROOM`] of a row above and below.
    pub(super) fn roomy(&self, r: Rectangle) -> Rectangle {
        let room = VERTICAL_ROOM * self.line_h;
        Rectangle {
            y: r.y - room,
            height: r.height + 2.0 * room,
            ..r
        }
    }
}

/// Draw the line.
pub(super) fn timed(r: &mut iced::Renderer, paint: &Paint<'_>) {
    let mut moving = [0_usize; MAX_MOVING];
    let mut count = 0;
    let mut clips = Clips::new(paint.clip);
    for (k, word) in paint.line.words.iter().enumerate() {
        if state_at(paint.t_ms, word.timing, paint.reduced) == WordState::Active
            && count < MAX_MOVING
        {
            moving[count] = k;
            count += 1;
            for b in &word.boxes {
                clips.cut(paint.roomy(paint.at(b)));
            }
        }
    }
    let grey = with_alpha(paint.ink, paint.grey);
    for piece in clips.iter() {
        r.fill_paragraph(&paint.line.para, paint.pos, grey, piece);
    }
    done_words(r, paint, &moving[..count]);
    for k in &moving[..count] {
        if let Some(word) = paint.line.words.get(*k) {
            sweep::moving(r, paint, word);
        }
    }
}

/// The pieces that are done, in bright ink: neighbours on a row are merged into one clip.
fn done_words(r: &mut iced::Renderer, paint: &Paint<'_>, moving: &[usize]) {
    let bright = with_alpha(paint.ink, paint.bright);
    let mut run: Option<Rectangle> = None;
    let flush = |r: &mut iced::Renderer, run: &mut Option<Rectangle>| {
        if let Some(rect) = run.take() {
            let rect = paint.roomy(rect);
            let lead = if rect.x <= paint.pos.x + 0.5 {
                START_PAD
            } else {
                0.0
            };
            let rect = Rectangle {
                x: rect.x - lead,
                width: rect.width + lead,
                ..rect
            };
            if let Some(c) = rect.intersection(&paint.clip) {
                r.fill_paragraph(&paint.line.para, paint.pos, bright, c);
            }
        }
    };
    for (k, word) in paint.line.words.iter().enumerate() {
        let done = !moving.contains(&k) && is_done(paint, word);
        for b in &word.boxes {
            let b = paint.at(b);
            match run {
                Some(cur) if done && touches(&cur, &b) => run = Some(cur.union(&b)),
                _ if done => {
                    flush(r, &mut run);
                    run = Some(b);
                }
                _ => {}
            }
        }
        if !done {
            flush(r, &mut run);
        }
    }
    flush(r, &mut run);
}

/// A piece the sweep has passed for good (or one that is moving but out of room to move alone).
fn is_done(paint: &Paint<'_>, word: &Word) -> bool {
    match state_at(paint.t_ms, word.timing, paint.reduced) {
        WordState::Past => true,
        WordState::Upcoming => false,
        // More moving pieces than we draw alone: bright, without motion.
        WordState::Active => paint.t_ms >= word.timing.start_ms,
    }
}

/// `b` follows `cur` on the same row (either side, for right-to-left text).
fn touches(cur: &Rectangle, b: &Rectangle) -> bool {
    (cur.y - b.y).abs() < 1.0
        && ((b.x - (cur.x + cur.width)).abs() < 1.5 || (cur.x - (b.x + b.width)).abs() < 1.5)
}
