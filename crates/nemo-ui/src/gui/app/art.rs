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

/// Largest picture decoded for one slot, in device px (the largest fetch step).
pub const MAX_ART_PX: u32 = 1080;

/// A window scale factor in percent (100 is 1x), so it can be part of a cache key.
#[must_use]
pub fn scale_pct(factor: f32) -> u32 {
    if !factor.is_finite() {
        return 100;
    }
    // Clamped to 50..=400 first, so the cast is exact and in range.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let pct = (factor * 100.0).round().clamp(50.0, 400.0) as u32;
    pct
}

/// Device px of a slot `slot` logical px wide at `scale_pct`, rounded up, at most
/// [`MAX_ART_PX`].
#[must_use]
pub fn device_px(slot: u32, scale_pct: u32) -> u32 {
    slot.saturating_mul(scale_pct)
        .div_ceil(100)
        .clamp(1, MAX_ART_PX)
}

/// How a decoded picture is drawn in its slot (`design.md` section 9: never upscale).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArtFit {
    /// The source has at least the slot's device px: fill the slot (it was cropped to 1:1).
    Fill,
    /// The source is smaller: draw it at its native size, this many logical px wide, centred on
    /// `surface-container`.
    Native(f32),
}

/// The fit of a square source `source_px` wide in a slot `slot` logical px wide at `scale_pct`.
/// The slot's device px are not capped here, so a source is never drawn above 1.0x.
#[must_use]
pub fn art_fit(source_px: u32, slot: u32, scale_pct: u32) -> ArtFit {
    let scale = scale_pct.max(1);
    if source_px >= slot.saturating_mul(scale).div_ceil(100) {
        ArtFit::Fill
    } else {
        // Picture sizes are at most a few thousand px, exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let native = source_px as f32 * 100.0 / scale as f32;
        ArtFit::Native(native)
    }
}

/// Edge of a decoded (raw RGBA) picture handle; `None` for other handles.
#[must_use]
pub fn handle_px(handle: &image::Handle) -> Option<u32> {
    match handle {
        image::Handle::Rgba { width, height, .. } => Some((*width).min(*height)),
        _ => None,
    }
}

/// One picture a view wants: where it comes from, the slot size, the window scale and the shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtSpec {
    /// Source.
    pub artwork: Artwork,
    /// Slot size in logical px.
    pub size: u32,
    /// Window scale factor in percent ([`scale_pct`]).
    pub scale_pct: u32,
    /// Corner shape.
    pub shape: ArtShape,
}

impl ArtSpec {
    /// Cache key: the source address plus slot, scale and shape. `None` without artwork.
    #[must_use]
    pub fn key(&self) -> Option<String> {
        art_key(&self.artwork)
            .map(|k| format!("{k}|{}|{}|{:?}", self.size, self.scale_pct, self.shape))
    }

    /// Pixels to fetch and decode: the slot times the window scale, so one source pixel lands on
    /// one screen pixel ([`device_px`]).
    #[must_use]
    pub fn px(&self) -> u32 {
        device_px(self.size, self.scale_pct)
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
    /// The colour fields of the window backdrop.
    pub mesh: MeshView,
}

/// The colour mesh of one cover, ready to draw: field textures and the flat colour under them,
/// per base mode.
#[derive(Debug, Clone)]
pub struct MeshView {
    /// Field textures in [`crate::canvas::MODES`] order, in anchor order.
    pub blobs: [Vec<image::Handle>; 3],
    /// The flat colour under the fields, per mode.
    pub under: [crate::theme::Rgba; 3],
    /// The fields composited into one image per mode (the CPU renderer draws this).
    pub stills: [image::Handle; 3],
}

impl MeshView {
    /// Upload-ready handles of an analysed mesh (raw RGBA, nothing decodes on the UI thread).
    #[must_use]
    pub fn new(m: &crate::mesh::Mesh) -> Self {
        let side = crate::mesh::BLOB_PX;
        Self {
            blobs: crate::canvas::MODES.map(|base| {
                (0..m.n)
                    .map(|i| image::Handle::from_rgba(side, side, m.texture(base, i).to_vec()))
                    .collect()
            }),
            under: m.base,
            stills: crate::canvas::MODES.map(|base| {
                let (w, h) = crate::mesh::STILL_PX;
                image::Handle::from_rgba(w, h, m.still(base).to_vec())
            }),
        }
    }

    /// The field textures for a base mode.
    #[must_use]
    pub fn blobs(&self, base: BaseMode) -> &[image::Handle] {
        &self.blobs[crate::canvas::mode_index(base)]
    }

    /// The composited still image for a base mode.
    #[must_use]
    pub fn still(&self, base: BaseMode) -> image::Handle {
        self.stills[crate::canvas::mode_index(base)].clone()
    }

    /// The flat colour under the fields for a base mode.
    #[must_use]
    pub fn under(&self, base: BaseMode) -> crate::theme::Rgba {
        self.under[crate::canvas::mode_index(base)]
    }
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

#[cfg(test)]
mod tests {
    use super::{ArtFit, ArtShape, ArtSpec, MAX_ART_PX, art_fit, device_px, scale_pct};
    use nemo_core::track::Artwork;

    fn spec(size: u32, scale: f32) -> ArtSpec {
        ArtSpec {
            artwork: Artwork::LocalFile("cover.jpg".into()),
            size,
            scale_pct: scale_pct(scale),
            shape: ArtShape::Card,
        }
    }

    #[test]
    fn art_asks_for_the_slot_size_times_the_window_scale() {
        assert_eq!(spec(160, 1.0).px(), 160);
        assert_eq!(spec(160, 1.25).px(), 200);
        assert_eq!(spec(160, 1.5).px(), 240);
        assert_eq!(spec(44, 1.75).px(), 77);
        assert_eq!(spec(160, 2.0).px(), 320);
        assert_eq!(spec(480, 2.0).px(), 960);
        assert_eq!(spec(480, 3.0).px(), MAX_ART_PX);
    }

    #[test]
    fn a_new_scale_is_a_new_cache_entry() {
        assert_ne!(spec(160, 1.0).key(), spec(160, 2.0).key());
        assert_eq!(spec(160, 2.0).key(), spec(160, 2.0).key());
    }

    #[test]
    fn a_source_at_least_the_slot_fills_it_and_a_smaller_one_is_drawn_native_and_centred() {
        // 160 px card at 200 %: 320 device px.
        assert_eq!(art_fit(320, 160, 200), ArtFit::Fill);
        assert_eq!(art_fit(544, 160, 200), ArtFit::Fill);
        assert_eq!(art_fit(226, 160, 200), ArtFit::Native(113.0));
        assert_eq!(art_fit(120, 160, 100), ArtFit::Native(120.0));
        // 125 %: 44 px rows need 55 device px.
        assert_eq!(art_fit(55, 44, 125), ArtFit::Fill);
        assert_eq!(art_fit(54, 44, 125), ArtFit::Native(43.2));
        // Above the fetch cap the source is still never stretched.
        assert_eq!(art_fit(1080, 480, 300), ArtFit::Native(360.0));
    }

    #[test]
    fn odd_scale_factors_stay_in_range() {
        assert_eq!(scale_pct(f32::NAN), 100);
        assert_eq!(scale_pct(0.0), 50);
        assert_eq!(scale_pct(9.0), 400);
        assert_eq!(device_px(0, 100), 1);
    }
}
