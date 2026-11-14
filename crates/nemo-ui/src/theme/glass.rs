//! Glass tokens: blur, tint, edge and shadow numbers.

use super::{Rgba, color};

/// Blur radius in px, tier High.
pub const BLUR_HIGH_PX: f32 = 24.0;
/// Blur radius in px, tier Medium.
pub const BLUR_MEDIUM_PX: f32 = 12.0;
/// Default tint alpha over `surface-container`.
pub const TINT_ALPHA: f32 = 0.55;
/// Lowest allowed tint alpha.
pub const TINT_ALPHA_MIN: f32 = 0.40;
/// Highest alpha before the tonal fallback is used.
pub const TINT_ALPHA_MAX: f32 = 0.90;
/// Tint step when raising alpha for legibility.
pub const TINT_ALPHA_STEP: f32 = 0.05;
/// Inner edge highlight alpha.
pub const EDGE_ALPHA: f32 = 0.22;
/// Top edge highlight alpha.
pub const EDGE_ALPHA_TOP: f32 = 0.32;
/// Max specular highlight alpha.
pub const SPECULAR_ALPHA_MAX: f32 = 0.12;
/// Max lens offset in px (High tier only).
pub const LENS_MAX_PX: f32 = 6.0;
/// Corner radius for bar and sheets.
pub const RADIUS: f32 = super::shape::XL;
/// Shadow offset y in px.
pub const SHADOW_OFFSET_Y: f32 = 8.0;
/// Shadow blur in px.
pub const SHADOW_BLUR: f32 = 24.0;
/// Shadow color (`#00000052`).
pub const SHADOW: Rgba = Rgba::from_rgba_hex(0x0000_0052);
/// Glass tint base color.
pub const TINT_BASE: Rgba = color::SURFACE_CONTAINER;
/// Glass tint base of the active mode.
#[must_use]
pub fn tint_base() -> Rgba {
    color::surface_container()
}
/// Edge highlight base color (white).
pub const EDGE: Rgba = Rgba::from_rgb_hex(0xFF_FFFF);
