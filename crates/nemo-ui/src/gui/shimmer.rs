//! The loading line (`lyrics-rebuild` spec): one line of text in the size of the lyrics, grey,
//! with a soft highlight that crosses it again and again while the lyrics are fetched. Text only,
//! no spinner. iced has no gradient text, so the highlight is a few thin clipped strips of bright
//! ink with a soft alpha, like the soft edge of the word sweep.

use iced::advanced::Widget;
use iced::advanced::layout::{self, Layout, Node};
use iced::advanced::renderer;
use iced::advanced::text::{
    self, Alignment, LineHeight, Paragraph as _, Renderer as _, Shaping, Text,
};
use iced::advanced::widget::{Tree, tree};
use iced::{Color, Element, Font, Length, Pixels, Point, Rectangle, Size, mouse};

use crate::lyrics_view::enter::shimmer_alpha;
use crate::lyrics_view::metrics::{INACTIVE_ALPHA, Metrics};

type Para = <iced::Renderer as text::Renderer>::Paragraph;

/// Strips the highlight is cut into.
const STRIPS: u8 = 10;

/// The shimmering loading line.
pub struct Shimmer {
    /// The text.
    pub text: String,
    /// Size and line height.
    pub metrics: Metrics,
    /// Where the highlight is, 0 (before the text) to 1 (past it).
    pub phase: f32,
    /// Ink at full alpha.
    pub ink: Color,
    /// Font of the line.
    pub font: Font,
    /// Draw the highlight (off with reduced motion).
    pub animate: bool,
}

#[derive(Default)]
struct State {
    key: Option<(String, u32, u32)>,
    para: Para,
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for Shimmer {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(self.metrics.line_h))
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        _renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> Node {
        let width = limits.max().width;
        let st = tree.state.downcast_mut::<State>();
        let key = (
            self.text.clone(),
            self.metrics.size.to_bits(),
            width.to_bits(),
        );
        if st.key.as_ref() != Some(&key) {
            st.para = Para::with_text(Text {
                content: self.text.as_str(),
                bounds: Size::new(width, self.metrics.line_h),
                size: Pixels(self.metrics.size),
                line_height: LineHeight::Absolute(Pixels(self.metrics.line_h)),
                font: self.font,
                align_x: Alignment::Default,
                align_y: iced::alignment::Vertical::Top,
                shaping: Shaping::Advanced,
                wrapping: text::Wrapping::None,
            });
            st.key = Some(key);
        }
        Node::new(Size::new(width, self.metrics.line_h))
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
        let Some(clip) = bounds.intersection(viewport) else {
            return;
        };
        let st = tree.state.downcast_ref::<State>();
        let pos = Point::new(bounds.x, bounds.y);
        let grey = Color {
            a: INACTIVE_ALPHA,
            ..self.ink
        };
        renderer.fill_paragraph(&st.para, pos, grey, clip);
        if !self.animate {
            return;
        }
        let text_w = st.para.min_bounds().width.max(1.0);
        let band = (text_w * 0.6).max(80.0);
        let centre = -band / 2.0 + self.phase * (text_w + band);
        let step = band / f32::from(STRIPS);
        for k in 0..STRIPS {
            let offset = (f32::from(k) + 0.5) / f32::from(STRIPS) * 2.0 - 1.0;
            let strip = Rectangle {
                x: pos.x + centre - band / 2.0 + f32::from(k) * step,
                y: pos.y,
                width: step + 0.5,
                height: self.metrics.line_h,
            };
            if let Some(c) = strip.intersection(&clip) {
                let bright = Color {
                    a: shimmer_alpha(offset),
                    ..self.ink
                };
                renderer.fill_paragraph(&st.para, pos, bright, c);
            }
        }
    }
}

impl<'a, Message: 'a> From<Shimmer> for Element<'a, Message> {
    fn from(s: Shimmer) -> Self {
        Element::new(s)
    }
}
