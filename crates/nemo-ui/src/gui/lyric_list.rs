//! The lyric list (`lyrics-rebuild` spec): every line of the document as one cached paragraph,
//! all in one size and weight, left aligned (right aligned for RTL), drawn at its place in the
//! list. Lit lines are bright and full size, the others dim and a little smaller, scaled around
//! their left edge; the lit look comes from the two clocks of `lyrics_view::clock`. The list
//! also fades lines toward its top and bottom edge. It holds no state of its own beyond the
//! shaped paragraphs: scroll, time and the document come in from the reducer on every view.

use std::borrow::Cow;

use iced::advanced::layout::{self, Layout, Node};
use iced::advanced::renderer;
use iced::advanced::widget::{Tree, tree};
use iced::advanced::{Renderer as _, Widget};
use iced::{Color, Element, Font, Length, Point, Rectangle, Size, Transformation, mouse};
use nemo_core::lyrics::LyricsDoc;

use crate::lyrics_view::clock::{Clocks, Timeline};
use crate::lyrics_view::enter::ENTER_SLIDE;
use crate::lyrics_view::layout::visible;
use crate::lyrics_view::metrics::{INACTIVE_ALPHA, Metrics, look};
use crate::lyrics_view::note::GapNote;
use crate::lyrics_view::scroll::Cascade;
use crate::lyrics_view::{self, layout as geometry};

mod paint;
mod shape;
mod words;

use shape::Line;

/// What the list shows and how: everything the view knows, borrowed.
pub struct LyricList<'a> {
    /// The document.
    pub doc: &'a LyricsDoc,
    /// Counts the documents shown, so a new one is shaped again.
    pub rev: u64,
    /// Index of the document line that `tops[0]` is the slot of (0 for the whole list).
    pub first: usize,
    /// Top of every slot and the total height last (see `lyrics_view::layout::tops`).
    pub tops: Cow<'a, [f32]>,
    /// Type size and spacing.
    pub metrics: Metrics,
    /// Width the lines wrap at, px.
    pub wrap_w: f32,
    /// Scroll offset, px from the top of the list.
    pub scroll: f32,
    /// When each line starts and stops.
    pub timeline: &'a Timeline,
    /// Both clocks now.
    pub clocks: Clocks,
    /// The intro or gap note, whose row the lines after it make room for.
    pub note: Option<GapNote>,
    /// Colour of the ink at full alpha.
    pub ink: Color,
    /// Font of the lines (`font.display`, weight 600).
    pub font: Font,
    /// Reduced motion: no scale, colour only.
    pub reduced: bool,
    /// Fade lines toward the top and bottom edge.
    pub fade_edges: bool,
    /// The glides of the lines (the scroll cascade), when the list scrolls.
    pub cascade: Option<&'a Cascade>,
    /// How far the list has slid in after the lyrics arrived, 0 to 1 (1: in place).
    pub enter: f32,
}

/// What the shaped lines were made for.
#[derive(PartialEq)]
struct Key {
    rev: u64,
    first: usize,
    count: usize,
    metrics: [u32; 3],
}

#[derive(Default)]
struct State {
    key: Option<Key>,
    lines: Vec<Option<Line>>,
}

impl LyricList<'_> {
    /// How far a line can be from its place in the layout now, px.
    fn reach(&self, viewport_h: f32) -> f32 {
        self.cascade.map_or(0.0, Cascade::reach) + self.slide(viewport_h)
    }

    /// How far below its place the list is while it slides in, px.
    fn slide(&self, viewport_h: f32) -> f32 {
        (1.0 - self.enter).clamp(0.0, 1.0) * ENTER_SLIDE * viewport_h
    }

    fn count(&self) -> usize {
        self.tops.len().saturating_sub(1)
    }

    fn key(&self) -> Key {
        let bits = |v: f32| v.to_bits();
        Key {
            rev: self.rev,
            first: self.first,
            count: self.count(),
            metrics: [
                bits(self.metrics.size),
                bits(self.metrics.line_h),
                bits(self.wrap_w),
            ],
        }
    }

    /// Shape the lines near the viewport that are not shaped yet.
    fn prepare(&self, st: &mut State, viewport_h: f32) {
        let key = self.key();
        if st.key.as_ref() != Some(&key) {
            st.lines = std::iter::repeat_with(|| None).take(key.count).collect();
            st.key = Some(key);
        }
        let margin = self.metrics.slot_height(1) + self.reach(viewport_h);
        let range = visible(
            &self.tops,
            self.note.as_ref(),
            &self.metrics,
            self.scroll,
            viewport_h,
            margin,
        );
        for j in range {
            let (Some(slot), Some(src)) = (st.lines.get_mut(j), self.doc.lines.get(self.first + j))
            else {
                continue;
            };
            if slot.is_none() {
                *slot = Some(shape::shape(
                    src,
                    self.doc.granularity,
                    &self.metrics,
                    self.wrap_w,
                    self.font,
                ));
            }
        }
    }

    /// Draw line `j` (an index into the shaped lines) whose slot starts `top` px below `bounds`.
    fn draw_line(
        &self,
        r: &mut iced::Renderer,
        line: &Line,
        j: usize,
        bounds: Rectangle,
        top: f32,
    ) {
        let met = &self.metrics;
        let idx = self.first + j;
        let slot =
            self.tops.get(j + 1).copied().unwrap_or(0.0) - self.tops.get(j).copied().unwrap_or(0.0);
        let text_h = (slot - 2.0 * met.gap).max(met.line_h);
        let fade = if self.fade_edges {
            lyrics_view::edge_fade(top + met.gap + text_h / 2.0, bounds.height)
        } else {
            1.0
        };
        let fade = fade * self.enter.clamp(0.0, 1.0);
        if fade <= 0.01 {
            return;
        }
        let window = self.timeline.window(idx);
        let lit = window.map_or(0.0, |w| w.lit_progress(&self.clocks));
        let ink_p = window.map_or(0.0, |w| w.ink_progress(&self.clocks));
        let lk = look(lit, self.reduced);
        let pos = Point::new(bounds.x, bounds.y + top + met.gap);
        let origin = Point::new(
            if line.rtl { pos.x + self.wrap_w } else { pos.x },
            pos.y + text_h / 2.0,
        );
        let draw = |r: &mut iced::Renderer| {
            self.ink_line(r, line, (pos, bounds), (lit, ink_p, fade));
        };
        if (lk.scale - 1.0).abs() < 1e-3 {
            draw(r);
            return;
        }
        let xf = Transformation::translate(origin.x, origin.y)
            * Transformation::scale(lk.scale)
            * Transformation::translate(-origin.x, -origin.y);
        r.with_transformation(xf, draw);
    }
}

impl LyricList<'_> {
    /// Paint a line at `pos` inside `clip`. `lit` is how lit the line is (scale and bright ink of
    /// word-timed lines), `ink` how bright the ink of a line without word timing is (it fades in
    /// over 0.33 s), `fade` the soft edge mask.
    fn ink_line(
        &self,
        r: &mut iced::Renderer,
        line: &Line,
        (pos, clip): (Point, Rectangle),
        (lit, ink, fade): (f32, f32, f32),
    ) {
        if line.words.is_empty() || lit <= 0.0 {
            // Without word timing the whole line takes the ink; an unlit timed line is all grey.
            let alpha = if line.words.is_empty() {
                look(ink, self.reduced).alpha
            } else {
                INACTIVE_ALPHA
            };
            paint::plain(
                r,
                line,
                pos,
                paint::with_alpha(self.ink, alpha * fade),
                clip,
            );
            return;
        }
        words::timed(
            r,
            &words::Paint {
                line,
                pos,
                clip,
                ink: self.ink,
                bright: lit * fade,
                grey: INACTIVE_ALPHA * fade,
                t_ms: self.clocks.audio_ms,
                reduced: self.reduced,
                size: self.metrics.size,
                line_h: self.metrics.line_h,
            },
        );
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for LyricList<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        _renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> Node {
        let size = limits.max();
        self.prepare(tree.state.downcast_mut::<State>(), size.height);
        Node::new(size)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        if !bounds.intersects(viewport) {
            return;
        }
        let st = tree.state.downcast_ref::<State>();
        let range = visible(
            &self.tops,
            self.note.as_ref(),
            &self.metrics,
            self.scroll,
            bounds.height,
            self.reach(bounds.height),
        );
        let slide = self.slide(bounds.height);
        renderer.with_layer(bounds, |r| {
            for j in range {
                let Some(Some(line)) = st.lines.get(j) else {
                    continue;
                };
                let top = geometry::top_of(&self.tops, self.note.as_ref(), &self.metrics, j)
                    - self.scroll
                    + self.cascade.map_or(0.0, |c| c.offset(self.first + j))
                    + slide;
                self.draw_line(r, line, j, bounds, top);
            }
        });
    }
}

impl<'a, Message: 'a> From<LyricList<'a>> for Element<'a, Message> {
    fn from(list: LyricList<'a>) -> Self {
        Element::new(list)
    }
}
