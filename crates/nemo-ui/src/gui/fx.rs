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

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Instant;

use iced::advanced::layout::{self, Layout, Node};
use iced::advanced::widget::operation::Focusable;
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget, overlay, renderer};
use iced::keyboard::key::Named;
use iced::{
    Border, Color, Element, Event, Length, Rectangle, Shadow, Size, Vector, keyboard, mouse, window,
};

use crate::anim;
use crate::motion::Spring;

static REDUCED_MOTION: AtomicBool = AtomicBool::new(false);

static FOCUS_RGBA: AtomicU32 = AtomicU32::new(0x8A_B4_F8_FF);

/// Colour of the keyboard focus ring (`design.md` section 5: 2 px `primary`, 2 px offset). The
/// app sets it to the cover's accent before each frame.
pub fn set_focus_color(c: Color) {
    FOCUS_RGBA.store(u32::from_be_bytes(c.into_rgba8()), Ordering::Relaxed);
}

fn focus_color() -> Color {
    let [red, green, blue, alpha] = FOCUS_RGBA.load(Ordering::Relaxed).to_be_bytes();
    Color::from_rgba8(red, green, blue, f32::from(alpha) / 255.0)
}

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

/// Where the hover play button sits in its host.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BadgeAnchor {
    /// Bottom right corner, this many px from both edges (Spotify cards).
    BottomRight(f32),
    /// Vertically centred, this many px from the right edge (quick access tiles).
    CenterRight(f32),
    /// Vertically centred, this many px from the left edge (track rows).
    CenterLeft(f32),
    /// The centre of the host (row thumbnails).
    Center,
}

/// A round button that fades in over its host on hover (play on a card, a row, a tile).
pub struct Badge<'a, M> {
    /// Diameter in px.
    pub size: f32,
    /// Position.
    pub anchor: BadgeAnchor,
    /// Disc colour.
    pub fill: Color,
    /// Glyph colour.
    pub ink: Color,
    /// The glyph.
    pub glyph: iced::widget::svg::Handle,
    /// Message when the disc is pressed (the host itself keeps its own press).
    pub on_press: Box<dyn Fn() -> M + 'a>,
}

impl<M> Badge<'_, M> {
    fn rect(&self, host: Rectangle, hover: f32) -> Rectangle {
        let s = self.size;
        let (x, y) = match self.anchor {
            BadgeAnchor::BottomRight(i) => {
                (host.x + host.width - s - i, host.y + host.height - s - i)
            }
            BadgeAnchor::CenterRight(i) => (
                host.x + host.width - s - i,
                host.y + (host.height - s) / 2.0,
            ),
            BadgeAnchor::CenterLeft(i) => (host.x + i, host.y + (host.height - s) / 2.0),
            BadgeAnchor::Center => (
                host.x + (host.width - s) / 2.0,
                host.y + (host.height - s) / 2.0,
            ),
        };
        // Slides up 6 px while it fades in.
        Rectangle {
            x,
            y: y + 6.0 * (1.0 - hover),
            width: s,
            height: s,
        }
    }
}

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
    badge: Option<Badge<'a, M>>,
    scale: f32,
    zoom: f32,
    pop: f32,
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
            badge: None,
            scale: 1.0,
            zoom: 0.0,
            pop: 0.0,
        }
    }

    /// A fixed scale around the centre of the child (the cover breathing with play and pause).
    #[must_use]
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// Grow by this fraction on hover (0.03 is 3%).
    #[must_use]
    pub fn zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom;
        self
    }

    /// Start the entrance this fraction smaller (0.02: it grows from 98% to 100%).
    #[must_use]
    pub fn pop(mut self, pop: f32) -> Self {
        self.pop = pop;
        self
    }

    /// A play button that fades in on hover and sends its own message.
    #[must_use]
    pub fn badge(mut self, badge: Badge<'a, M>) -> Self {
        self.badge = Some(badge);
        self
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
        .pop(anim::POP)
        .into()
}

struct State {
    hover: Spring,
    press: Spring,
    down: bool,
    last: Option<Instant>,
    /// Keyboard focus (set by Tab through the focus operations); draws the ring.
    focused: bool,
}

impl Focusable for State {
    fn is_focused(&self) -> bool {
        self.focused
    }

    fn focus(&mut self) {
        self.focused = true;
    }

    fn unfocus(&mut self) {
        self.focused = false;
    }
}

impl Default for State {
    fn default() -> Self {
        let (k, z) = anim::EFFECTS_FAST;
        Self {
            hover: Spring::tuned(0.0, k, z),
            press: Spring::tuned(0.0, k, z),
            down: false,
            last: None,
            focused: false,
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
        if self.interactive {
            let state = tree.state.downcast_mut::<State>();
            operation.focusable(None, layout.bounds(), state);
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
        if self.interactive {
            let state = tree.state.downcast_mut::<State>();
            match event {
                // A mouse press anywhere ends keyboard focus: the ring is for the keyboard.
                Event::Mouse(mouse::Event::ButtonPressed(_)) => state.focused = false,
                // Enter or Space on the focused control presses it: the child button gets a
                // click at its centre.
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(Named::Enter | Named::Space),
                    ..
                }) if state.focused => {
                    if let Some(child) = layout.children().next() {
                        let at = mouse::Cursor::Available(layout.bounds().center());
                        for click in [
                            mouse::Event::ButtonPressed(mouse::Button::Left),
                            mouse::Event::ButtonReleased(mouse::Button::Left),
                        ] {
                            self.content.as_widget_mut().update(
                                &mut tree.children[0],
                                &Event::Mouse(click),
                                child,
                                at,
                                renderer,
                                clipboard,
                                shell,
                                viewport,
                            );
                        }
                    }
                    shell.capture_event();
                    shell.request_redraw();
                    return;
                }
                _ => {}
            }
        }
        if let Some(badge) = &self.badge
            && let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event
        {
            let state = tree.state.downcast_ref::<State>();
            if state.hover.value > 0.3 && cursor.is_over(badge.rect(layout.bounds(), 1.0)) {
                shell.publish((badge.on_press)());
                shell.capture_event();
                shell.request_redraw();
                return;
            }
        }
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
        let s = self.scale * (1.0 + self.zoom * hover) * (1.0 - self.pop * (1.0 - self.progress));
        if (s - 1.0).abs() > 1e-4 {
            let c = moved.center();
            let t = iced::Transformation::translate(0.0, dy)
                * iced::Transformation::translate(c.x, c.y)
                * iced::Transformation::scale(s)
                * iced::Transformation::translate(-c.x, -c.y);
            renderer.with_transformation(t, draw_child);
        } else if dy.abs() > f32::EPSILON {
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

        if let Some(badge) = &self.badge
            && hover > 0.02
        {
            use iced::advanced::svg::{Renderer as _, Svg};
            let host = Rectangle {
                y: moved.y,
                ..bounds
            };
            let r = badge.rect(host, hover);
            renderer.fill_quad(
                renderer::Quad {
                    bounds: r,
                    border: Border {
                        radius: (badge.size / 2.0).into(),
                        ..Border::default()
                    },
                    shadow: Shadow {
                        color: Color::from_rgba(0.0, 0.0, 0.0, 0.35 * hover),
                        offset: Vector::new(0.0, 4.0),
                        blur_radius: 12.0,
                    },
                    snap: false,
                },
                Color {
                    a: badge.fill.a * hover,
                    ..badge.fill
                },
            );
            let g = badge.size * 0.5;
            let glyph = Rectangle {
                x: r.x + (r.width - g) / 2.0,
                y: r.y + (r.height - g) / 2.0,
                width: g,
                height: g,
            };
            renderer.draw_svg(
                Svg::new(badge.glyph.clone()).color(Color {
                    a: badge.ink.a * hover,
                    ..badge.ink
                }),
                glyph,
                glyph,
            );
        }

        if self.interactive && state.focused {
            // 2 px ring, 1 px air between it and the control.
            let ring = Rectangle {
                x: moved.x - 3.0,
                y: moved.y - 3.0,
                width: moved.width + 6.0,
                height: moved.height + 6.0,
            };
            renderer.fill_quad(
                renderer::Quad {
                    bounds: ring,
                    border: Border {
                        radius: (self.radius + 3.0).into(),
                        width: 2.0,
                        color: focus_color(),
                    },
                    shadow: Shadow::default(),
                    snap: false,
                },
                Color::TRANSPARENT,
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
