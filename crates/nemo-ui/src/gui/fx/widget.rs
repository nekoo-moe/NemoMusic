//! The iced `Widget` implementation of `Fx`: layout, events, drawing and overlays.

use iced::advanced::layout::{self, Layout, Node};
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget, overlay, renderer};
use iced::keyboard::key::Named;
use iced::{
    Border, Color, Event, Length, Rectangle, Shadow, Size, Vector, keyboard, mouse, window,
};

use super::state::State;
use super::{Fx, HOVER, PRESS, focus_color, reduced_motion};

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
