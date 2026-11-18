//! The drifting window backdrop as an iced widget. It draws a flat colour and the cover's
//! pre-rendered colour fields (`crate::mesh`) with a transform (no blur at draw time, nothing
//! re-blurred per frame) and paces its own redraws: on each redraw it asks for the next one at
//! the capped drift rate, without a message, so the app's `update` and `view` do not run for the
//! drift. It stops asking while it may not move, and it freezes while the window has no focus.

use iced::advanced::image::{self as adv_image, FilterMethod};
use iced::advanced::layout::{self, Layout, Node};
use iced::advanced::widget::{Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget, renderer};
use iced::widget::image;
use iced::{Element, Event, Length, Radians, Rectangle, Size, mouse, window};

use crate::drift::{self, Clock, Inputs, Mode};
use crate::gui::style::col;
use crate::theme::Rgba;

/// The colour fields of one cover and how strongly they show.
#[derive(Debug, Clone)]
pub struct Layer {
    /// One texture per colour field, in anchor order.
    pub blobs: Vec<image::Handle>,
    /// Opacity, 0 to 1 (1 for the settled cover; the cross-fade for the old and the new one).
    pub opacity: f32,
    /// Scale of the fields around their anchors (1 at rest).
    pub scale: f32,
}

/// The backdrop widget: a flat colour with the colour fields over it.
pub struct Field {
    under: Rgba,
    layers: Vec<Layer>,
    inputs: Inputs,
}

impl Field {
    /// A backdrop of `layers` (bottom first) over the flat colour `under`. `inputs` come from
    /// the app; the widget fills in the window focus and the renderer.
    #[must_use]
    pub fn new(under: Rgba, layers: Vec<Layer>, inputs: Inputs) -> Self {
        Self {
            under,
            layers,
            inputs,
        }
    }
}

struct State {
    clock: Clock,
    focused: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            clock: Clock::default(),
            focused: true,
        }
    }
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for Field {
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
        _tree: &mut Tree,
        _renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> Node {
        Node::new(limits.max())
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        match event {
            Event::Window(window::Event::Focused) => {
                state.focused = true;
                shell.request_redraw();
            }
            Event::Window(window::Event::Unfocused) => state.focused = false,
            Event::Window(window::Event::RedrawRequested(now)) => {
                let moving = Mode::of(Inputs {
                    focused: state.focused,
                    software: matches!(renderer, iced::Renderer::Secondary(_)),
                    ..self.inputs
                }) == Mode::Moving;
                if let Some(next) = state.clock.tick(*now, moving) {
                    shell.request_redraw_at(next);
                }
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        use adv_image::Renderer as _;
        use iced::advanced::Renderer as _;
        let state = tree.state.downcast_ref::<State>();
        let b = layout.bounds();
        renderer.with_layer(b, |r| {
            r.fill_quad(
                renderer::Quad {
                    bounds: b,
                    ..renderer::Quad::default()
                },
                col(self.under),
            );
            for layer in &self.layers {
                for (i, handle) in layer.blobs.iter().enumerate() {
                    // At most `FIELDS` (4) fields: the index always fits.
                    let pose = drift::pose(state.clock.elapsed_ms, u8::try_from(i).unwrap_or(0));
                    let (x, y, side) =
                        drift::blob_rect((b.x, b.y, b.width, b.height), i, pose, layer.scale);
                    r.draw_image(
                        adv_image::Image {
                            handle: handle.clone(),
                            filter_method: FilterMethod::Linear,
                            rotation: Radians(0.0),
                            border_radius: iced::border::Radius::default(),
                            opacity: layer.opacity.clamp(0.0, 1.0),
                            snap: false,
                        },
                        Rectangle::new(iced::Point::new(x, y), Size::new(side, side)),
                        b,
                    );
                }
            }
        });
    }
}

impl<'a, M: 'a> From<Field> for Element<'a, M> {
    fn from(f: Field) -> Self {
        Element::new(f)
    }
}
