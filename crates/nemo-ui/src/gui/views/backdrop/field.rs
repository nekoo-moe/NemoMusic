//! The drifting window backdrop as an iced widget. It draws a flat colour and the cover's
//! pre-rendered colour fields (`crate::mesh`) with a transform (no blur at draw time, nothing
//! re-blurred per frame). It has no timer: the window's one frame clock is the app's frame
//! ticker (`App::needs_frames` includes the backdrop while it moves), and the widget only reads
//! the time of each redraw. It stops its own clock while it may not move, and it freezes, at the
//! pose it holds, while the window has no focus.

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
    /// The fields composited into one image: the CPU renderer draws this alone.
    pub still: image::Handle,
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
    heartbeat: bool,
    preview: Option<f64>,
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
            heartbeat: false,
            preview: None,
        }
    }

    /// Breathe: grow and shift the fields slowly with the drift clock (the `Gentle heartbeat`
    /// switch). The breath freezes with the drift clock, at the pose it holds.
    #[must_use]
    pub fn heartbeat(mut self, on: bool) -> Self {
        self.heartbeat = on;
        self
    }

    /// Draw as if the drift clock stood at `ms` and the backdrop moved (headless snapshots).
    #[must_use]
    pub fn preview(mut self, ms: Option<f64>) -> Self {
        self.preview = ms;
        self
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
        _shell: &mut Shell<'_, M>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        match event {
            Event::Window(window::Event::Focused) => state.focused = true,
            Event::Window(window::Event::Unfocused) => {
                state.focused = false;
                state.clock.stop();
            }
            Event::Window(window::Event::RedrawRequested(now)) => {
                let moving = Mode::of(Inputs {
                    focused: state.focused,
                    software: matches!(renderer, iced::Renderer::Secondary(_)),
                    ..self.inputs
                }) == Mode::Moving;
                state.clock.tick(*now, moving);
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
        // The CPU renderer (tiny-skia) scales every image on the CPU: it draws one composited
        // image and never moves.
        // A preview (headless snapshots) shows the GPU look on the CPU renderer.
        let software = matches!(renderer, iced::Renderer::Secondary(_)) && self.preview.is_none();
        // A paused or unfocused window holds the clock, so the fields hold their pose.
        let elapsed = self.preview.unwrap_or(state.clock.elapsed_ms);
        renderer.with_layer(b, |r| {
            r.fill_quad(
                renderer::Quad {
                    bounds: b,
                    ..renderer::Quad::default()
                },
                col(self.under),
            );
            for layer in &self.layers {
                if software {
                    draw_still(r, layer, b);
                    continue;
                }
                for (i, handle) in layer.blobs.iter().enumerate() {
                    let (x, y, side) = drift::field_rect(
                        (b.x, b.y, b.width, b.height),
                        i,
                        elapsed,
                        layer.scale,
                        self.heartbeat,
                    );
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

/// The composited still image, scaled by the layer's scale around the window centre.
fn draw_still(r: &mut iced::Renderer, layer: &Layer, b: Rectangle) {
    use adv_image::Renderer as _;
    let (w, h) = (b.width * layer.scale, b.height * layer.scale);
    r.draw_image(
        adv_image::Image {
            handle: layer.still.clone(),
            filter_method: FilterMethod::Linear,
            rotation: Radians(0.0),
            border_radius: iced::border::Radius::default(),
            opacity: layer.opacity.clamp(0.0, 1.0),
            snap: false,
        },
        Rectangle::new(
            iced::Point::new(b.x + (b.width - w) / 2.0, b.y + (b.height - h) / 2.0),
            Size::new(w, h),
        ),
        b,
    );
}

impl<'a, M: 'a> From<Field> for Element<'a, M> {
    fn from(f: Field) -> Self {
        Element::new(f)
    }
}
