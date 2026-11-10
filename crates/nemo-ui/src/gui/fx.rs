//! `Fx`: one wrapper widget that gives its child the Material motion of this app.
//!
//! * Animated **state layers**: a rounded overlay that fades to 8% on hover and 10% on press with
//!   the `effects.fast` spring.
//! * A hover **lift**: the child rises a few px with a soft shadow (`spatial.fast`).
//! * An **entrance**: a fade and a slide up, driven from outside by a 0 to 1 progress.
//!
//! The widget owns its hover and press springs. It asks for a redraw only while a spring still
//! moves, so an idle window draws no frames. With reduced motion every value jumps to its target.
//! The child keeps its own layout and events; `Fx` only watches the cursor.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use iced::advanced::layout::{self, Layout, Node};
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget, overlay, renderer};
use iced::{Border, Color, Element, Event, Length, Rectangle, Shadow, Size, Vector, mouse, window};

use crate::anim;
use crate::motion::Spring;

static REDUCED_MOTION: AtomicBool = AtomicBool::new(false);

/// Tell every [`Fx`] whether motion is reduced (the app sets it before each frame is built).
pub fn set_reduced_motion(on: bool) {
    REDUCED_MOTION.store(on, Ordering::Relaxed);
}

/// `true` when motion is reduced.
#[must_use]
pub fn reduced_motion() -> bool {
    REDUCED_MOTION.load(Ordering::Relaxed)
}

/// State layer opacity on hover. M3 says 8%; iced blends in linear light, which makes a white
/// layer read about twice as strong, so the drawn value is 0.045 (measured: the result sits at
/// about 8% in sRGB terms).
pub const HOVER: f32 = 0.045;
/// State layer opacity on press (M3: 10%, same correction).
pub const PRESS: f32 = 0.06;

/// Wrap `content` in an [`Fx`].
pub fn fx<'a, M: 'a>(content: impl Into<Element<'a, M>>) -> Fx<'a, M> {
    Fx::new(content)
}

/// See the module docs.
pub struct Fx<'a, M> {
    content: Element<'a, M>,
    layer: Color,
    radius: f32,
    lift: f32,
    interactive: bool,
    progress: f32,
    slide: f32,
    fade_to: Color,
}

impl<'a, M: 'a> Fx<'a, M> {
    /// A wrapper with state layers on `on-surface`, radius 0.
    pub fn new(content: impl Into<Element<'a, M>>) -> Self {
        Self {
            content: content.into(),
            layer: Color::from_rgb8(0xF2, 0xF3, 0xF5),
            radius: 0.0,
            lift: 0.0,
            interactive: true,
            progress: 1.0,
            slide: 0.0,
            fade_to: Color::TRANSPARENT,
        }
    }

    /// Corner radius of the state layer (match the child's).
    #[must_use]
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Colour of the state layer.
    #[must_use]
    pub fn layer(mut self, layer: Color) -> Self {
        self.layer = layer;
        self
    }

    /// Rise by `px` on hover, with a shadow.
    #[must_use]
    pub fn lift(mut self, px: f32) -> Self {
        self.lift = px;
        self
    }

    /// Entrance progress (1 = settled): the child slides up `slide` px and fades from `fade_to`,
    /// the colour of whatever is behind it.
    #[must_use]
    pub fn entrance(mut self, progress: f32, slide: f32, fade_to: Color) -> Self {
        self.progress = progress.clamp(0.0, 1.0);
        self.slide = slide;
        self.fade_to = fade_to;
        self
    }

    /// No hover and press layers (an entrance-only wrapper).
    #[must_use]
    pub fn passive(mut self) -> Self {
        self.interactive = false;
        self
    }
}

/// Entrance-only wrapper: `progress` from [`anim::Anim::reveal`], sliding [`anim::SLIDE_PX`].
pub fn entrance<'a, M: 'a>(
    content: impl Into<Element<'a, M>>,
    progress: f32,
    fade_to: Color,
) -> Element<'a, M> {
    if progress >= 0.999 {
        return content.into();
    }
    Fx::new(content)
        .passive()
        .entrance(progress, anim::SLIDE_PX, fade_to)
        .into()
}

struct State {
    hover: Spring,
    press: Spring,
    down: bool,
    last: Option<Instant>,
}

impl Default for State {
    fn default() -> Self {
        let (k, z) = anim::EFFECTS_FAST;
        Self {
            hover: Spring::tuned(0.0, k, z),
            press: Spring::tuned(0.0, k, z),
            down: false,
            last: None,
        }
    }
}

impl State {
    fn moving(&self) -> bool {
        !self.hover.is_settled() || !self.press.is_settled()
    }
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for Fx<'_, M> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> Node {
        let child = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        Node::with_children(child.size(), vec![child])
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        if let Some(child) = layout.children().next() {
            self.content
                .as_widget_mut()
                .operate(&mut tree.children[0], child, renderer, operation);
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) {
        if let Some(child) = layout.children().next() {
            self.content.as_widget_mut().update(
                &mut tree.children[0],
                event,
                child,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }
        if !self.interactive && self.lift <= 0.0 {
            return;
        }
        let state = tree.state.downcast_mut::<State>();
        let over = cursor.is_over(layout.bounds());
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if over => {
                state.down = true;
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => state.down = false,
            _ => {}
        }
        let hover = if over { 1.0 } else { 0.0 };
        let press = if over && state.down { 1.0 } else { 0.0 };
        let changed = (state.hover.target - hover).abs() > f32::EPSILON
            || (state.press.target - press).abs() > f32::EPSILON;
        state.hover.target = hover;
        state.press.target = press;

        if reduced_motion() {
            state.hover.finish();
            state.press.finish();
            if changed {
                shell.request_redraw();
            }
            return;
        }
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let dt = state
                .last
                .map_or(1.0 / 60.0, |l| {
                    now.saturating_duration_since(l).as_secs_f32()
                })
                .min(0.05);
            state.last = Some(*now);
            state.hover.step(dt);
            state.press.step(dt);
            if state.moving() {
                shell.request_redraw();
            } else {
                state.last = None;
            }
        } else if changed {
            shell.request_redraw();
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        layout
            .children()
            .next()
            .map_or(mouse::Interaction::None, |child| {
                self.content.as_widget().mouse_interaction(
                    &tree.children[0],
                    child,
                    cursor,
                    viewport,
                    renderer,
                )
            })
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        use iced::advanced::Renderer as _;
        let Some(child) = layout.children().next() else {
            return;
        };
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        let hover = state.hover.value.clamp(0.0, 1.0);
        let press = state.press.value.clamp(0.0, 1.0);
        let rise = self.lift * hover;
        let slide = (1.0 - self.progress) * self.slide;
        let dy = slide - rise;
        let moved = Rectangle {
            y: bounds.y + dy,
            ..bounds
        };

        if rise > 0.05 {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: moved,
                    border: Border {
                        radius: self.radius.into(),
                        ..Border::default()
                    },
                    shadow: Shadow {
                        color: Color::from_rgba(0.0, 0.0, 0.0, 0.40 * hover),
                        offset: Vector::new(0.0, 6.0 * hover),
                        blur_radius: 18.0 * hover,
                    },
                    snap: false,
                },
                Color::TRANSPARENT,
            );
        }

        let draw_child = |r: &mut iced::Renderer| {
            self.content.as_widget().draw(
                &tree.children[0],
                r,
                theme,
                style,
                child,
                cursor,
                viewport,
            );
        };
        if dy.abs() > f32::EPSILON {
            renderer.with_translation(Vector::new(0.0, dy), draw_child);
        } else {
            draw_child(renderer);
        }

        let alpha = (HOVER * hover + PRESS * press).min(PRESS + 0.01);
        if self.interactive && alpha > 0.002 {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: moved,
                    border: Border {
                        radius: self.radius.into(),
                        ..Border::default()
                    },
                    shadow: Shadow::default(),
                    snap: false,
                },
                Color {
                    a: self.layer.a * alpha,
                    ..self.layer
                },
            );
        }

        if self.progress < 0.999 {
            let cover = Rectangle {
                x: bounds.x - 2.0,
                y: bounds.y - 2.0,
                width: bounds.width + 4.0,
                height: bounds.height + 4.0 + self.slide.max(0.0),
            };
            renderer.fill_quad(
                renderer::Quad {
                    bounds: cover,
                    ..renderer::Quad::default()
                },
                Color {
                    a: self.fade_to.a * (1.0 - self.progress),
                    ..self.fade_to
                },
            );
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, M, iced::Theme, iced::Renderer>> {
        let child = layout.children().next()?;
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            child,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, M: 'a> From<Fx<'a, M>> for Element<'a, M> {
    fn from(fx: Fx<'a, M>) -> Self {
        Element::new(fx)
    }
}
