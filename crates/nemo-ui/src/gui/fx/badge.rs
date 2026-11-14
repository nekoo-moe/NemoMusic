//! The round hover button of an `Fx` and where it sits in its host.

use iced::{Color, Rectangle};

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
    pub(super) fn rect(&self, host: Rectangle, hover: f32) -> Rectangle {
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
