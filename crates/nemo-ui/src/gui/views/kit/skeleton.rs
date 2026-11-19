//! The skeleton kit (skeleton-loading spec): soft rounded blocks, circles and text lines, and
//! ready-made shapes that mirror the real layouts (same outer size, so nothing jumps when the
//! content arrives). One public concept, [`Skeleton`]: the look of the moment (shimmer position
//! and opacity) with a builder for every shape.
//!
//! Blocks are `surface-container-high` at a low alpha over the pane. One soft band of
//! `surface-container-highest` crosses the whole page, not each block, so the blocks of a page
//! shimmer as one.

use iced::advanced::Widget;
use iced::advanced::layout::{self, Layout, Node};
use iced::advanced::renderer;
use iced::advanced::widget::Tree;
use iced::gradient::Linear;
use iced::widget::{Space, column, container, row};
use iced::{Alignment, Background, Border, Element, Length, Rectangle, Size, mouse};

use crate::gui::app::{App, ArtShape, Message};
use crate::gui::style::{col, gap};
use crate::skeleton::{Region, Show, band_position};
use crate::theme::{TextStyle, color, shape, type_scale};

use super::ROW_H;

mod shapes;
pub use shapes::LIST_ITEM_H;
#[cfg(test)]
pub(crate) mod measure;
#[cfg(test)]
mod tests;

/// Alpha of a block: `surface-container-high` over the pane.
const BASE_ALPHA: f32 = 0.8;
/// Alpha of the band: `surface-container-highest`.
const BAND_ALPHA: f32 = 0.9;
/// Alpha of the band over an inset block: `on-surface`, a light touch.
const INSET_BAND_ALPHA: f32 = 0.14;
/// Share of the viewport width the band covers.
const BAND_SHARE: f32 = 0.35;
/// Narrowest and widest band in px.
const BAND_MIN: f32 = 200.0;
const BAND_MAX: f32 = 480.0;
/// A text line block is this share of the type line high.
const LINE_SHARE: f32 = 0.7;
/// Width of the duration block at the end of a row.
const TIME_W: f32 = 36.0;
/// Cover slot of a row, as the real rows draw it.
const COVER: f32 = 44.0;

/// The look of a skeleton at this moment. Copy it into every block of a page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Skeleton {
    /// Loop position of the band, 0 to 1.
    phase: f32,
    /// Opacity of everything drawn (falls to 0 while the content fades in).
    opacity: f32,
    /// Draw the band (off with reduced motion).
    animate: bool,
    /// Nested in another block: one tone further from the pane, so it shows on the block under it.
    inset: bool,
}

impl Skeleton {
    /// The look of `region` now, or `None` when the region shows its content.
    #[must_use]
    pub fn of(app: &App, region: Region) -> Option<Self> {
        let opacity = match app.skeletons.show(region) {
            Show::Content => return None,
            Show::Skeleton => 1.0,
            Show::Fade(a) => a,
        };
        Some(Self {
            phase: app.skeletons.phase(),
            opacity,
            animate: !app.ui.reduced_motion,
            inset: false,
        })
    }

    /// The look for blocks drawn on top of another block (the lines and cover inside a card, the
    /// hero or the Top result).
    #[must_use]
    pub fn inset(self) -> Self {
        Self {
            inset: true,
            ..self
        }
    }

    /// A look at full opacity without the band, for tests and measurements.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn still() -> Self {
        Self {
            phase: 0.0,
            opacity: 1.0,
            animate: false,
            inset: false,
        }
    }

    /// A rounded block `width` wide (any length) and `height` px high with corner `radius`.
    #[must_use]
    pub fn block<'a, M: 'a>(self, width: Length, height: f32, radius: f32) -> Element<'a, M> {
        Element::new(Block {
            look: self,
            width,
            height,
            radius,
        })
    }

    /// A circle of `diameter` px.
    #[must_use]
    pub fn circle<'a, M: 'a>(self, diameter: f32) -> Element<'a, M> {
        self.block(Length::Fixed(diameter), diameter, shape::FULL)
    }

    /// A text line of `width` px in type style `style`. The outer height is the type line, as a
    /// real label is; the block inside is [`LINE_SHARE`] of it.
    #[must_use]
    pub fn line<'a, M: 'a>(self, width: f32, style: TextStyle) -> Element<'a, M> {
        let line = f32::from(style.line);
        container(self.block(Length::Fixed(width), (line * LINE_SHARE).round(), shape::XS))
            .width(Length::Fixed(width))
            .height(Length::Fixed(line))
            .align_y(Alignment::Center)
            .into()
    }

    /// A track row `width` wide and [`ROW_H`] high: cover, two lines, duration.
    #[must_use]
    pub fn row<'a, M: 'a>(self, width: f32, seed: usize) -> Element<'a, M> {
        // Slightly different line lengths, so a list does not look stamped.
        let t = [0.42_f32, 0.34, 0.5][seed % 3];
        let text_w = (width - COVER - gap::MD - gap::XL - TIME_W - gap::MD).max(80.0);
        let lines = column![
            self.line(text_w * t / 0.5, type_scale::BODY_MD),
            self.line(text_w * 0.3, type_scale::BODY_SM),
        ];
        container(
            row![
                self.block(Length::Fixed(COVER), COVER, ArtShape::Row.radius(COVER)),
                Space::new().width(gap::MD),
                lines,
                Space::new().width(Length::Fill),
                self.block(Length::Fixed(TIME_W), 12.0, shape::XS),
            ]
            .align_y(Alignment::Center),
        )
        .width(Length::Fixed(width))
        .height(Length::Fixed(ROW_H))
        .align_y(Alignment::Center)
        .into()
    }
}

/// One rounded block. Draws a single quad; with the band under it, a gradient quad.
struct Block {
    look: Skeleton,
    width: Length,
    height: f32,
    radius: f32,
}

impl Block {
    fn background(&self, bounds: Rectangle, viewport: &Rectangle) -> Background {
        let k = self.look.opacity;
        let (base, glow) = if self.look.inset {
            (
                col(color::surface_container_highest().with_alpha(BASE_ALPHA * k)),
                col(color::on_surface().with_alpha(INSET_BAND_ALPHA * k)),
            )
        } else {
            (
                col(color::surface_container_high().with_alpha(BASE_ALPHA * k)),
                col(color::surface_container_highest().with_alpha(BAND_ALPHA * k)),
            )
        };
        if !self.look.animate || bounds.width <= 0.0 {
            return Background::Color(base);
        }
        let band_w = (viewport.width * BAND_SHARE).clamp(BAND_MIN, BAND_MAX);
        // The band centre goes from one band width before the viewport to one after it.
        let centre =
            viewport.x - band_w / 2.0 + band_position(self.look.phase) * (viewport.width + band_w);
        let (lo, hi) = (centre - band_w / 2.0, centre + band_w / 2.0);
        if hi <= bounds.x || lo >= bounds.x + bounds.width {
            return Background::Color(base);
        }
        let at = |x: f32| ((x - bounds.x) / bounds.width).clamp(0.0, 1.0);
        let grad = Linear::new(iced::Radians(std::f32::consts::FRAC_PI_2))
            .add_stop(0.0, base)
            .add_stop(at(lo), base)
            .add_stop(at(centre), glow)
            .add_stop(at(hi), base)
            .add_stop(1.0, base);
        Background::Gradient(grad.into())
    }
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for Block {
    fn size(&self) -> Size<Length> {
        Size::new(self.width, Length::Fixed(self.height))
    }

    fn layout(&mut self, _: &mut Tree, _: &iced::Renderer, limits: &layout::Limits) -> Node {
        layout::atomic(limits, self.width, Length::Fixed(self.height))
    }

    fn draw(
        &self,
        _: &Tree,
        renderer: &mut iced::Renderer,
        _: &iced::Theme,
        _: &renderer::Style,
        layout: Layout<'_>,
        _: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        use iced::advanced::Renderer as _;
        let bounds = layout.bounds();
        if bounds.intersection(viewport).is_none() {
            return;
        }
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border {
                    radius: self.radius.into(),
                    ..Border::default()
                },
                shadow: iced::Shadow::default(),
                snap: false,
            },
            self.background(bounds, viewport),
        );
    }
}

/// `real` while the region shows content, the skeleton while it waits, and both while the
/// content fades in: the content rises out of the pane colour (no slide) under the skeleton,
/// which fades out. Both ends match the plain states, so there is no jump.
pub fn swap<'a>(
    app: &App,
    region: Region,
    real: impl FnOnce() -> Element<'a, Message>,
    skeleton: impl FnOnce(Skeleton) -> Element<'a, Message>,
) -> Element<'a, Message> {
    let Some(look) = Skeleton::of(app, region) else {
        return real();
    };
    if look.opacity >= 1.0 {
        return skeleton(look);
    }
    let content = crate::gui::fx::Fx::new(real()).passive().entrance(
        1.0 - look.opacity,
        0.0,
        super::pane_bg(),
    );
    iced::widget::stack![content, skeleton(look)].into()
}

/// The blank page of the 150 ms wait: nothing yet, nothing flashing.
#[must_use]
pub fn blank<'a, M: 'a>() -> Element<'a, M> {
    Space::new().into()
}
