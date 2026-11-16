//! The active line of word- and syllable-timed lyrics (`syllable-motion` spec). One paragraph
//! holds the whole line, shaped once per change of the animated range, so wrapping, Vietnamese
//! marks, CJK and RTL come from the same text engine as every other text. Words before the
//! range are bright, words after it grey. Each word in the range is drawn from a second,
//! identically laid out paragraph that shows only those words: grey, then the bright copy
//! clipped to the filled share of the word, with a lift, a glow and (held syllables) a scale
//! applied at draw time, so layout never moves. Nothing is allocated while drawing.

use std::ops::Range;

use iced::advanced::layout::{self, Layout, Node};
use iced::advanced::renderer;
use iced::advanced::text::{
    self, Alignment, LineHeight, Paragraph as _, Shaping, Span, Text, Wrapping,
};
use iced::advanced::widget::{Tree, tree};
use iced::advanced::{Renderer as _, Widget, text::Renderer as _};
use iced::{
    Color, Element, Font, Length, Pixels, Point, Rectangle, Size, Transformation, Vector, mouse,
};

use crate::lyric_pieces::{self, Timing, WordFx};

type Para = <iced::Renderer as text::Renderer>::Paragraph;

/// Padding of a word's clip box, px: room for the glow and the scale.
const PAD: f32 = 3.0;
/// Width of the soft edge of the fill, px.
const EDGE: f32 = 6.0;
/// Offsets of the four glow copies, px.
const GLOW_OFFSETS: [(f32, f32); 4] = [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)];

/// How the line looks, besides its text and timing.
#[derive(Debug, Clone, Copy)]
pub struct Style {
    /// Font size, px.
    pub size: f32,
    /// Row height, px.
    pub line_h: f32,
    /// Wrap width, px.
    pub width: f32,
    /// Font.
    pub font: Font,
    /// Ink of a sung word.
    pub bright: Color,
    /// Ink of a word not yet sung.
    pub grey: Color,
    /// Colour of the glow of the word being sung.
    pub glow: Color,
    /// The text reads right to left: the fill runs from the right.
    pub rtl: bool,
    /// Reduced motion: stepwise colour only.
    pub reduced: bool,
}

/// The widget: `texts[i]` is the text of piece `i`, `timings[i]` its timing.
pub struct SyllableText {
    texts: Vec<String>,
    timings: Vec<Timing>,
    t_ms: i64,
    style: Style,
}

impl SyllableText {
    /// A line at song time `t_ms` (position plus the lyrics offset).
    #[must_use]
    pub fn new(texts: Vec<String>, timings: Vec<Timing>, t_ms: i64, style: Style) -> Self {
        Self {
            texts,
            timings,
            t_ms,
            style,
        }
    }
}

/// What the cached paragraphs were built for.
#[derive(PartialEq)]
struct Key {
    texts: Vec<String>,
    anim: Range<usize>,
    sung: usize,
    bounds: Size,
    metrics: [u32; 2],
    colors: [[u32; 4]; 2],
}

struct State {
    key: Option<Key>,
    /// The whole line: sung words bright, grey ahead, animated words invisible.
    base: Para,
    /// The line with only the animated words visible.
    words: Para,
    /// Boxes of each animated word (one per row it wraps onto), relative to the paragraph.
    boxes: Vec<Vec<Rectangle>>,
}

fn bits(c: Color) -> [u32; 4] {
    [c.r.to_bits(), c.g.to_bits(), c.b.to_bits(), c.a.to_bits()]
}

impl SyllableText {
    fn rebuild(&self, st: &mut State, bounds: Size, key: Key) {
        let s = &self.style;
        let frame = &key;
        let span = |i: usize, t: &str| {
            let color = if i < frame.sung {
                s.bright
            } else if frame.anim.contains(&i) {
                Color::TRANSPARENT
            } else {
                s.grey
            };
            Span::<(), Font>::new(t.to_owned()).color(color)
        };
        let base: Vec<Span<'_, (), Font>> = self
            .texts
            .iter()
            .enumerate()
            .map(|(i, t)| span(i, t))
            .collect();
        // The animated words carry no colour: they take the one given when drawing.
        let words: Vec<Span<'_, (), Font>> = self
            .texts
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let sp = Span::<(), Font>::new(t.to_owned());
                if frame.anim.contains(&i) {
                    sp
                } else {
                    sp.color(Color::TRANSPARENT)
                }
            })
            .collect();
        let shape = |spans: &[Span<'_, (), Font>]| {
            Para::with_spans(Text {
                content: spans,
                bounds,
                size: Pixels(s.size),
                line_height: LineHeight::Absolute(Pixels(s.line_h)),
                font: s.font,
                align_x: Alignment::Default,
                align_y: iced::alignment::Vertical::Top,
                shaping: Shaping::Advanced,
                wrapping: Wrapping::WordOrGlyph,
            })
        };
        st.base = shape(&base);
        st.boxes.clear();
        if key.anim.is_empty() {
            st.words = Para::default();
        } else {
            st.words = shape(&words);
            for i in key.anim.clone() {
                st.boxes.push(st.words.span_bounds(i));
            }
        }
        st.key = Some(key);
    }

    /// Draw one animated word: glow, grey, then the bright copy clipped to the filled share.
    fn draw_word(
        &self,
        renderer: &mut iced::Renderer,
        words: &Para,
        boxes: &[Rectangle],
        origin: Point,
        fx: WordFx,
        viewport: Rectangle,
    ) {
        let s = &self.style;
        let Some(first) = boxes.first() else {
            return;
        };
        let union = boxes.iter().fold(*first, |a, b| a.union(b));
        let centre = Point::new(
            origin.x + union.x + union.width / 2.0,
            origin.y + union.y + union.height / 2.0,
        );
        let lift_pad = fx.lift.ceil();
        let clip_of = |r: &Rectangle| Rectangle {
            x: origin.x + r.x - PAD,
            y: origin.y + r.y - PAD - lift_pad,
            width: r.width + 2.0 * PAD,
            height: r.height + 2.0 * PAD + lift_pad,
        };
        let m = Transformation::translate(0.0, -fx.lift)
            * Transformation::translate(centre.x, centre.y)
            * Transformation::scale(fx.scale)
            * Transformation::translate(-centre.x, -centre.y);
        renderer.with_transformation(m, |r| {
            if fx.glow > 0.02 {
                let glow = Color {
                    a: s.glow.a * 0.22 * fx.glow,
                    ..s.glow
                };
                let clip = clip_of(&union);
                if let Some(c) = clip.intersection(&viewport) {
                    for (dx, dy) in GLOW_OFFSETS {
                        r.fill_paragraph(words, origin + Vector::new(dx, dy), glow, c);
                    }
                }
            }
            let total: f32 = boxes.iter().map(|b| b.width).sum();
            let mut reveal = fx.fill * total;
            for b in boxes {
                let share = reveal.clamp(0.0, b.width);
                reveal -= share;
                let full = clip_of(b);
                if share < b.width
                    && let Some(c) = full.intersection(&viewport)
                {
                    r.fill_paragraph(words, origin, s.grey, c);
                }
                if share <= 0.0 {
                    continue;
                }
                // Bright part, then one soft step of half-bright ahead of the edge.
                let (bright, edge) = split(&full, b, origin, share, s.rtl);
                if let Some(c) = bright.intersection(&viewport) {
                    r.fill_paragraph(words, origin, s.bright, c);
                }
                if share < b.width {
                    let mid = mix(s.grey, s.bright, 0.5);
                    if let Some(c) = edge.intersection(&viewport) {
                        r.fill_paragraph(words, origin, mid, c);
                    }
                }
            }
        });
    }
}

/// The bright clip and the soft-edge clip of a word box `b` filled by `share` px.
fn split(
    full: &Rectangle,
    b: &Rectangle,
    origin: Point,
    share: f32,
    rtl: bool,
) -> (Rectangle, Rectangle) {
    let edge_w = EDGE.min(b.width - share).max(0.0);
    if rtl {
        let right = origin.x + b.x + b.width;
        let left = right - share;
        let bright = Rectangle {
            x: left,
            width: right + PAD - left,
            ..*full
        };
        let edge = Rectangle {
            x: left - edge_w,
            width: edge_w,
            ..*full
        };
        (bright, edge)
    } else {
        let left = origin.x + b.x;
        let right = left + share;
        let bright = Rectangle {
            x: left - PAD,
            width: right - left + PAD,
            ..*full
        };
        let edge = Rectangle {
            x: right,
            width: edge_w,
            ..*full
        };
        (bright, edge)
    }
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for SyllableText {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State {
            key: None,
            base: Para::default(),
            words: Para::default(),
            boxes: Vec::new(),
        })
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.style.width), Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        _renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> Node {
        let st = tree.state.downcast_mut::<State>();
        layout::sized(
            limits,
            Length::Fixed(self.style.width),
            Length::Shrink,
            |limits| {
                let bounds = limits.max();
                let f = lyric_pieces::frame(&self.timings, self.t_ms, self.style.reduced);
                let key = Key {
                    texts: Vec::new(),
                    anim: f.anim,
                    sung: f.sung,
                    bounds,
                    metrics: [self.style.size.to_bits(), self.style.line_h.to_bits()],
                    colors: [bits(self.style.bright), bits(self.style.grey)],
                };
                let same = st.key.as_ref().is_some_and(|k| {
                    k.anim == key.anim
                        && k.sung == key.sung
                        && k.bounds == key.bounds
                        && k.metrics == key.metrics
                        && k.colors == key.colors
                        && k.texts == self.texts
                });
                if !same {
                    let key = Key {
                        texts: self.texts.clone(),
                        ..key
                    };
                    self.rebuild(st, bounds, key);
                }
                st.base.min_bounds()
            },
        )
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
        if !layout.bounds().intersects(viewport) {
            return;
        }
        let st = tree.state.downcast_ref::<State>();
        let origin = layout.position();
        // The span colours carry the alpha: the draw colour must be opaque (tiny-skia multiplies).
        let opaque = Color {
            a: 1.0,
            ..self.style.bright
        };
        renderer.fill_paragraph(&st.base, origin, opaque, *viewport);
        let Some(key) = st.key.as_ref() else {
            return;
        };
        for (k, i) in key.anim.clone().enumerate() {
            let (Some(w), Some(boxes)) = (self.timings.get(i), st.boxes.get(k)) else {
                continue;
            };
            let fx = lyric_pieces::fx_at(self.t_ms, *w, self.style.reduced);
            self.draw_word(renderer, &st.words, boxes, origin, fx, *viewport);
        }
    }
}

impl<'a, Message: 'a> From<SyllableText> for Element<'a, Message> {
    fn from(w: SyllableText) -> Self {
        Element::new(w)
    }
}
