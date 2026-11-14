//! Cover art shapes, specs and the cached cover and canvas views.

use super::job::art_key;
use crate::scheme::BaseMode;
use crate::theme::shape;
use iced::widget::image;
use nemo_core::track::{Artwork, TrackId};
use std::hash::Hash;

/// Shape of a picture slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtShape {
    /// Card art, radius `shape.md`.
    Card,
    /// Row art, radius `shape.sm`.
    Row,
    /// Hero art, radius `shape.lg`.
    Hero,
    /// Artists: a circle.
    Circle,
    /// Large shelf card, radius 20.
    Tile,
    /// List thumbnail of 48 to 56 px, radius `shape.md`.
    Thumb,
}

/// Corner radius of the large shelf cards.
pub const TILE_RADIUS: f32 = 20.0;

impl ArtShape {
    /// Corner radius in px for a slot `size` px wide.
    #[must_use]
    pub fn radius(self, size: f32) -> f32 {
        match self {
            Self::Circle => size / 2.0,
            Self::Card | Self::Thumb => shape::md(),
            Self::Row => shape::sm(),
            Self::Hero => 24.0,
            Self::Tile => TILE_RADIUS,
        }
    }
}

/// One picture a view wants: where it comes from, the slot size and the shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtSpec {
    /// Source.
    pub artwork: Artwork,
    /// Slot size in logical px.
    pub size: u32,
    /// Corner shape.
    pub shape: ArtShape,
}

impl ArtSpec {
    /// Cache key: the source address plus slot and shape. `None` without artwork.
    #[must_use]
    pub fn key(&self) -> Option<String> {
        art_key(&self.artwork).map(|k| format!("{k}|{}|{:?}", self.size, self.shape))
    }

    /// Pixels to decode: 1.5 times the slot for sharp covers on high-DPI screens, at most 480.
    #[must_use]
    pub fn px(&self) -> u32 {
        (self.size * 3 / 2).clamp(48, 480)
    }

    /// Corner radius as a fraction of the width.
    #[must_use]
    pub fn fraction(&self) -> f32 {
        // Slot sizes are small integers, exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let size = self.size.max(1) as f32;
        self.shape.radius(size) / size
    }
}

/// State of one decoded picture in the cache.
#[derive(Debug, Clone)]
pub enum ArtEntry {
    /// Requested, not yet here.
    Loading,
    /// Ready to draw (raw pixels, no decoding on the UI thread).
    Ready(image::Handle),
    /// There is no art (or it failed): draw the placeholder.
    Missing,
}

/// The current track's cover, analysed.
#[derive(Debug, Clone)]
pub struct CoverView {
    /// Track this cover belongs to.
    pub track: TrackId,
    /// Full cover with `shape.lg` corners (hero and Now Playing).
    pub art: image::Handle,
    /// Cover with `shape.md` corners for the 64 px slot in the bar.
    pub art_bar: image::Handle,
    /// Colours sampled left, middle, right from the blurred backdrop.
    pub strip: [crate::theme::Rgba; 3],
    /// Pre-blurred backdrop texture.
    pub backdrop: image::Handle,
    /// Glass tint alpha, or `None` for the tonal fallback.
    pub tint_alpha: Option<f32>,
    /// Scrim alpha over the Now Playing backdrop.
    pub scrim_alpha: f32,
    /// The song-coloured canvas behind the pane and the full-screen Now Playing.
    pub canvas: CanvasView,
}

/// The canvas of one cover, ready to draw: a texture and pane gradient stops per base mode.
#[derive(Debug, Clone)]
pub struct CanvasView {
    /// Textures in [`crate::canvas::MODES`] order.
    pub textures: [image::Handle; 3],
    /// Gradient stops in the same order.
    pub stops: [[crate::theme::Rgba; crate::canvas::STOPS]; 3],
}

impl CanvasView {
    /// Upload-ready handles of an analysed canvas (raw RGBA, nothing decodes on the UI thread).
    #[must_use]
    pub fn new(c: &crate::canvas::Canvas) -> Self {
        Self {
            textures: c
                .textures
                .clone()
                .map(|px| image::Handle::from_rgba(c.px, c.px, px)),
            stops: c.stops,
        }
    }

    /// Texture for a base mode.
    #[must_use]
    pub fn texture(&self, base: BaseMode) -> image::Handle {
        self.textures[crate::canvas::mode_index(base)].clone()
    }

    /// Gradient stops for a base mode.
    #[must_use]
    pub fn stops(&self, base: BaseMode) -> [crate::theme::Rgba; crate::canvas::STOPS] {
        self.stops[crate::canvas::mode_index(base)]
    }
}
