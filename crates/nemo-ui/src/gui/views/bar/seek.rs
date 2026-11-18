//! The seek slider of the capsule (`player-bar-redesign` spec): a 4 px rounded track that grows
//! to 6 px under the pointer or while dragging, a thumb that appears on hover, focus and drag, the
//! played part in the song accent, and a time tooltip above the thumb while dragging. One small
//! custom widget, because the stock `slider` cannot animate its track or draw a tooltip. The
//! springs run only while they move, so an idle bar asks for no frames.

use std::time::Instant;

use iced::advanced::layout::{self, Layout, Node};
use iced::advanced::text::{self, Renderer as _, Text};
use iced::advanced::widget::operation::Focusable;
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Renderer as _, Shell, Widget, renderer};
use iced::{
    Border, Color, Element, Event, Length, Pixels, Point, Rectangle, Shadow, Size, alignment,
    mouse, window,
};

use iced::Alignment;
use iced::widget::{Space, container};

use crate::anim::SPATIAL_FAST;
use crate::format;
use crate::gui::app::{App, Message};
use crate::gui::assets::font;
use crate::gui::fx::{focus_color, reduced_motion};
use crate::gui::style::{col, gap, label};
use crate::motion::Spring;
use crate::palette::Palette;
use crate::theme::{color, shape, type_scale};

/// Height of the hit area.
pub const HEIGHT: f32 = 16.0;
/// Track height at rest.
pub const RAIL: f32 = 4.0;
/// Track height under the pointer or while dragging.
pub const RAIL_HOVER: f32 = 6.0;
/// Thumb radius at full size. The track stops this far from both ends so the thumb stays inside.
pub const THUMB: f32 = 7.0;
/// Gap between the focus ring and the thumb.
const RING_GAP: f32 = 3.0;
/// Width of the focus ring (`design.md` section 5).
const RING: f32 = 2.0;
/// Height of the time tooltip.
const TIP_HEIGHT: f32 = 28.0;
/// Padding of the time tooltip, left and right.
const TIP_PAD: f32 = 8.0;
/// Width of one digit of the tooltip text, as a share of its size (tabular figures).
const DIGIT: f32 = 0.62;
/// Air between the tooltip and the track.
const TIP_AIR: f32 = 6.0;

/// The ends of the track inside `bounds`.
#[must_use]
pub fn track(bounds: Rectangle) -> (f32, f32) {
    (bounds.x + THUMB, bounds.x + bounds.width - THUMB)
}

/// The fraction of the track a pointer at `x` is at, between 0 and 1.
#[must_use]
pub fn frac_at(x: f32, start: f32, end: f32) -> f32 {
    if end <= start {
        return 0.0;
    }
    ((x - start) / (end - start)).clamp(0.0, 1.0)
}

/// Track height for a hover value (0 at rest, 1 under the pointer).
#[must_use]
pub fn rail_height(hover: f32) -> f32 {
    RAIL + (RAIL_HOVER - RAIL) * hover.clamp(0.0, 1.0)
}

/// Thumb radius for a hover value: none at rest, full size under the pointer.
#[must_use]
pub fn thumb_radius(hover: f32) -> f32 {
    THUMB * hover.clamp(0.0, 1.0)
}

struct State {
    hover: Spring,
    dragging: bool,
    focused: bool,
    last: Option<Instant>,
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
        let (k, z) = SPATIAL_FAST;
        Self {
            hover: Spring::tuned(0.0, k, z),
            dragging: false,
            focused: false,
            last: None,
        }
    }
}

/// Colours of the slider.
#[derive(Debug, Clone, Copy)]
pub struct Colors {
    /// Played part and thumb.
    pub fill: Color,
    /// Rest of the track.
    pub rail: Color,
    /// Tooltip surface.
    pub tip_bg: Color,
    /// Tooltip text.
    pub tip_ink: Color,
}

/// The seek slider. `frac` is the played fraction (the drag position while dragging).
pub struct SeekBar<'a, M> {
    frac: f32,
    colors: Colors,
    label_at: Box<dyn Fn(f32) -> String + 'a>,
    on_drag: Box<dyn Fn(f32) -> M + 'a>,
    on_release: M,
}

impl<'a, M> SeekBar<'a, M> {
    /// A slider at `frac`. `label_at` gives the tooltip text for a fraction; `on_drag` runs while
    /// the pointer is down, `on_release` once when it comes up.
    pub fn new(
        frac: f32,
        colors: Colors,
        label_at: impl Fn(f32) -> String + 'a,
        on_drag: impl Fn(f32) -> M + 'a,
        on_release: M,
    ) -> Self {
        Self {
            frac: frac.clamp(0.0, 1.0),
            colors,
            label_at: Box::new(label_at),
            on_drag: Box::new(on_drag),
            on_release,
        }
    }
}

impl<'a, M: Clone + 'a> From<SeekBar<'a, M>> for Element<'a, M> {
    fn from(bar: SeekBar<'a, M>) -> Self {
        Element::new(bar)
    }
}

fn quad(renderer: &mut iced::Renderer, bounds: Rectangle, color: Color, radius: f32) {
    renderer.fill_quad(
        renderer::Quad {
            bounds,
            border: Border {
                radius: radius.into(),
                ..Border::default()
            },
            shadow: Shadow::default(),
            snap: false,
        },
        color,
    );
}

impl<M: Clone> SeekBar<'_, M> {
    /// Drag, release and hover handling for one event.
    fn pointer(
        &self,
        state: &mut State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
        shell: &mut Shell<'_, M>,
    ) {
        let (start, end) = track(bounds);
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                // A press anywhere ends keyboard focus: the ring is for the keyboard.
                state.focused = false;
                if let Some(p) = cursor.position_over(bounds) {
                    state.dragging = true;
                    shell.publish((self.on_drag)(frac_at(p.x, start, end)));
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) if state.dragging => {
                shell.publish((self.on_drag)(frac_at(position.x, start, end)));
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.dragging => {
                state.dragging = false;
                shell.publish(self.on_release.clone());
                shell.capture_event();
            }
            _ => {}
        }
    }

    /// Move the hover spring toward its target, asking for frames only while it moves.
    fn animate(state: &mut State, want: f32, event: &Event, shell: &mut Shell<'_, M>) {
        let changed = (state.hover.target - want).abs() > f32::EPSILON;
        state.hover.target = want;
        if reduced_motion() {
            state.hover.finish();
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
            if state.hover.is_settled() {
                state.last = None;
            } else {
                shell.request_redraw();
            }
        } else if changed {
            shell.request_redraw();
        }
    }

    /// The time tooltip above the thumb, on its own layer so nothing covers it.
    fn draw_tip(&self, renderer: &mut iced::Renderer, thumb_x: f32, top: f32, clip: Rectangle) {
        let text = (self.label_at)(self.frac);
        let size = f32::from(type_scale::LABEL_MD.size);
        // A text length is a few characters, exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let width = text.chars().count() as f32 * size * DIGIT + 2.0 * TIP_PAD;
        let x = (thumb_x - width / 2.0)
            .clamp(clip.x + TIP_AIR, (clip.x + clip.width - width).max(clip.x));
        let area = Rectangle {
            x,
            y: top - TIP_AIR - TIP_HEIGHT,
            width,
            height: TIP_HEIGHT,
        };
        renderer.with_layer(clip, |r| {
            r.fill_quad(
                renderer::Quad {
                    bounds: area,
                    border: Border {
                        radius: shape::sm().into(),
                        ..Border::default()
                    },
                    shadow: Shadow::default(),
                    snap: false,
                },
                self.colors.tip_bg,
            );
            r.fill_text(
                Text {
                    content: text,
                    bounds: area.size(),
                    size: Pixels(size),
                    line_height: text::LineHeight::Absolute(Pixels(TIP_HEIGHT)),
                    font: font(false),
                    align_x: text::Alignment::Center,
                    align_y: alignment::Vertical::Center,
                    shaping: text::Shaping::Basic,
                    wrapping: text::Wrapping::None,
                },
                Point::new(area.center_x(), area.center_y()),
                self.colors.tip_ink,
                clip,
            );
        });
    }
}

impl<M: Clone> Widget<M, iced::Theme, iced::Renderer> for SeekBar<'_, M> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(HEIGHT))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> Node {
        layout::atomic(limits, Length::Fill, Length::Fixed(HEIGHT))
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<State>();
        operation.focusable(None, layout.bounds(), state);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();
        self.pointer(state, event, bounds, cursor, shell);
        let want = if cursor.is_over(bounds) || state.dragging || state.focused {
            1.0
        } else {
            0.0
        };
        Self::animate(state, want, event, shell);
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
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
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let b = layout.bounds();
        let hover = state.hover.value.clamp(0.0, 1.0);
        let (start, end) = track(b);
        let h = rail_height(hover);
        let rail = Rectangle {
            x: start,
            y: b.center_y() - h / 2.0,
            width: end - start,
            height: h,
        };
        quad(renderer, rail, self.colors.rail, h / 2.0);
        let thumb_x = start + (end - start) * self.frac;
        let played = Rectangle {
            width: thumb_x - start,
            ..rail
        };
        if played.width > 0.5 {
            quad(renderer, played, self.colors.fill, h / 2.0);
        }
        let r = thumb_radius(hover);
        if r > 0.5 {
            let thumb = Rectangle {
                x: thumb_x - r,
                y: b.center_y() - r,
                width: 2.0 * r,
                height: 2.0 * r,
            };
            quad(renderer, thumb, self.colors.fill, r);
            if state.focused {
                let ring = Rectangle {
                    x: thumb.x - RING_GAP,
                    y: thumb.y - RING_GAP,
                    width: thumb.width + 2.0 * RING_GAP,
                    height: thumb.height + 2.0 * RING_GAP,
                };
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: ring,
                        border: Border {
                            radius: (r + RING_GAP).into(),
                            width: RING,
                            color: focus_color(),
                        },
                        shadow: Shadow::default(),
                        snap: false,
                    },
                    Color::TRANSPARENT,
                );
            }
        }
        if state.dragging {
            self.draw_tip(renderer, thumb_x, b.center_y() - r, *viewport);
        }
    }
}

/// Width of each time label beside the slider.
const TIME_W: f32 = 40.0;

/// `1:23  ---o---  2:47`: the times use tabular figures in a fixed width, so nothing shifts as
/// they tick. While dragging, the left time and the tooltip follow the thumb.
pub(super) fn row(app: &App, accent: Palette) -> Element<'_, Message> {
    let duration = app.ui.snapshot.duration;
    let frac = match (app.seek_drag, duration) {
        (Some(f), _) => f,
        (None, Some(d)) if !d.is_zero() => {
            (app.ui.position.as_secs_f32() / d.as_secs_f32()).clamp(0.0, 1.0)
        }
        _ => 0.0,
    };
    let shown = match (app.seek_drag, duration) {
        (Some(f), Some(d)) => d.mul_f32(f.clamp(0.0, 1.0)),
        _ => app.ui.position,
    };
    let dim = col(color::on_surface_variant());
    let time = |text: String, align: Alignment| {
        container(label(text, type_scale::LABEL_MD, false, dim))
            .width(Length::Fixed(TIME_W))
            .align_x(align)
    };
    let colors = Colors {
        fill: col(accent.primary),
        rail: col(color::on_surface().with_alpha(0.20)),
        tip_bg: col(color::surface_container_highest()),
        tip_ink: col(color::on_surface()),
    };
    let bar = SeekBar::new(
        frac,
        colors,
        move |f| duration.map_or_else(|| "-:--".to_owned(), |d| format::clock(d.mul_f32(f))),
        Message::SeekDrag,
        Message::SeekRelease,
    );
    iced::widget::row![
        time(format::clock(shown), Alignment::End),
        Space::new().width(gap::SM),
        bar,
        Space::new().width(gap::SM),
        time(
            duration.map_or_else(|| "-:--".to_owned(), format::clock),
            Alignment::Start
        ),
    ]
    .align_y(Alignment::Center)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pointer_maps_to_a_fraction_of_the_track_and_never_leaves_it() {
        assert!((frac_at(10.0, 10.0, 110.0)).abs() < f32::EPSILON);
        assert!((frac_at(60.0, 10.0, 110.0) - 0.5).abs() < f32::EPSILON);
        assert!((frac_at(110.0, 10.0, 110.0) - 1.0).abs() < f32::EPSILON);
        assert!((frac_at(-40.0, 10.0, 110.0)).abs() < f32::EPSILON);
        assert!((frac_at(900.0, 10.0, 110.0) - 1.0).abs() < f32::EPSILON);
        assert!((frac_at(50.0, 10.0, 10.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn the_track_stays_a_thumb_radius_inside_the_bounds() {
        let b = Rectangle {
            x: 20.0,
            y: 0.0,
            width: 200.0,
            height: HEIGHT,
        };
        assert_eq!(track(b), (20.0 + THUMB, 220.0 - THUMB));
    }

    #[test]
    fn the_track_grows_from_four_to_six_and_the_thumb_from_nothing() {
        assert!((rail_height(0.0) - 4.0).abs() < f32::EPSILON);
        assert!((rail_height(1.0) - 6.0).abs() < f32::EPSILON);
        assert!((rail_height(3.0) - 6.0).abs() < f32::EPSILON);
        assert!(thumb_radius(0.0).abs() < f32::EPSILON);
        assert!((thumb_radius(1.0) - THUMB).abs() < f32::EPSILON);
    }
}
