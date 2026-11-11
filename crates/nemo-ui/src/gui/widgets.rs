//! Small custom widgets.

use iced::advanced::Renderer as _;
use iced::advanced::layout::{self, Layout, Node};
use iced::advanced::renderer;
use iced::advanced::widget::{Tree, Widget};
use iced::{Element, Length, Rectangle, Size, Vector, mouse};

/// Shows a tall child shifted up by `offset` px and clipped to its own bounds.
///
/// The lyrics list uses it: the offset comes from a spring (`design.md` section 10), which a
/// stock `scrollable` cannot be given as a plain value (it only scrolls through tasks).
/// It does not forward events: the content must not be interactive.
pub struct Offset<'a, Message> {
    content: Element<'a, Message>,
    offset: f32,
}

impl<'a, Message> Offset<'a, Message> {
    /// Wrap `content`, shifted up by `offset` px.
    pub fn new(content: impl Into<Element<'a, Message>>, offset: f32) -> Self {
        Self {
            content: content.into(),
            offset,
        }
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for Offset<'_, Message> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> Node {
        let max = limits.max();
        let child_limits = layout::Limits::new(Size::ZERO, Size::new(max.width, f32::INFINITY));
        let child =
            self.content
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, &child_limits);
        Node::with_children(max, vec![child])
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let Some(child_layout) = layout.children().next() else {
            return;
        };
        let offset = self.offset;
        renderer.with_layer(bounds, |renderer| {
            renderer.with_translation(Vector::new(0.0, -offset), |renderer| {
                let visible = Rectangle {
                    y: bounds.y + offset,
                    ..bounds
                };
                self.content.as_widget().draw(
                    &tree.children[0],
                    renderer,
                    theme,
                    style,
                    child_layout,
                    cursor,
                    &visible,
                );
            });
        });
    }
}

impl<'a, Message: 'a> From<Offset<'a, Message>> for Element<'a, Message> {
    fn from(widget: Offset<'a, Message>) -> Self {
        Element::new(widget)
    }
}
