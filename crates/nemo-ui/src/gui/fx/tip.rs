//! [`TipFade`]: the enter motion of an iced tooltip (app-motion spec, task 2). iced draws the
//! tooltip bubble in an overlay that gets no events, so this wrapper keeps the clock: it notes
//! when the cursor came over the control, asks for frames only during the `motion.fast` fade
//! after the delay, and draws the bubble risen 4 px with its text fading in from the bubble
//! colour. Reduced motion shows it at once. Idle (no hover) asks for nothing.

use std::time::{Duration, Instant};

use iced::advanced::layout::{self, Layout, Node};
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget, overlay, renderer};
use iced::{Border, Color, Element, Event, Length, Rectangle, Size, Vector, mouse, window};

use super::reduced_motion;
use crate::theme::motion::FAST_MS;

/// Rise of the bubble while it fades in, px.
const RISE: f32 = 4.0;

/// See the module docs.
pub struct TipFade<'a, M> {
    content: Element<'a, M>,
    delay: Duration,
    fill: Color,
    radius: f32,
}

impl<'a, M: 'a> TipFade<'a, M> {
    /// Wrap a tooltip that opens after `delay`; `fill` and `radius` are the bubble's.
    pub fn new(
        content: impl Into<Element<'a, M>>,
        delay: Duration,
        fill: Color,
        radius: f32,
    ) -> Self {
        Self {
            content: content.into(),
            delay,
            fill,
            radius,
        }
    }
}

#[derive(Default)]
struct State {
    hover_at: Option<Instant>,
    now: Option<Instant>,
}

fn fade() -> Duration {
    Duration::from_millis(u64::from(FAST_MS))
}

impl State {
    /// Fade progress of the bubble, 0 to 1.
    fn progress(&self, delay: Duration) -> f32 {
        match (self.hover_at, self.now) {
            (Some(at), Some(now)) if !reduced_motion() => {
                let shown = now.saturating_duration_since(at).saturating_sub(delay);
                (shown.as_secs_f32() / fade().as_secs_f32()).clamp(0.0, 1.0)
            }
            _ => 1.0,
        }
    }
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for TipFade<'_, M> {
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

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
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
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        let state = tree.state.downcast_mut::<State>();
        let over = cursor.is_over(layout.bounds());
        match (over, state.hover_at) {
            (true, None) => {
                let at = Instant::now();
                state.hover_at = Some(at);
                shell.request_redraw_at(at + self.delay);
            }
            (false, Some(_)) => state.hover_at = None,
            _ => {}
        }
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            state.now = Some(*now);
            if let Some(at) = state.hover_at
                && !reduced_motion()
            {
                let open = at + self.delay;
                if *now < open {
                    shell.request_redraw_at(open);
                } else if *now < open + fade() {
                    shell.request_redraw();
                }
            }
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
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
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
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, M, iced::Theme, iced::Renderer>> {
        let p = tree.state.downcast_ref::<State>().progress(self.delay);
        let inner = self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )?;
        if p >= 1.0 {
            return Some(inner);
        }
        Some(overlay::Element::new(Box::new(FadeOverlay {
            inner,
            p,
            fill: self.fill,
            radius: self.radius,
        })))
    }
}

struct FadeOverlay<'b, M> {
    inner: overlay::Element<'b, M, iced::Theme, iced::Renderer>,
    p: f32,
    fill: Color,
    radius: f32,
}

impl<M> overlay::Overlay<M, iced::Theme, iced::Renderer> for FadeOverlay<'_, M> {
    fn layout(&mut self, renderer: &iced::Renderer, bounds: Size) -> Node {
        self.inner.as_overlay_mut().layout(renderer, bounds)
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        use iced::advanced::Renderer as _;
        let dy = RISE * (1.0 - self.p);
        renderer.with_translation(Vector::new(0.0, dy), |r| {
            self.inner
                .as_overlay()
                .draw(r, theme, style, layout, cursor);
            // The overlay is a group: each child is one bubble.
            for bubble in layout.children() {
                r.fill_quad(
                    renderer::Quad {
                        bounds: bubble.bounds(),
                        border: Border {
                            radius: self.radius.into(),
                            ..Border::default()
                        },
                        ..renderer::Quad::default()
                    },
                    Color {
                        a: self.fill.a * (1.0 - self.p),
                        ..self.fill
                    },
                );
            }
        });
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.inner
            .as_overlay()
            .mouse_interaction(layout, cursor, renderer)
    }
}

impl<'a, M: 'a> From<TipFade<'a, M>> for Element<'a, M> {
    fn from(t: TipFade<'a, M>) -> Self {
        Element::new(t)
    }
}
