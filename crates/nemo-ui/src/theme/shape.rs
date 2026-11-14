//! Shape (corner radius) scale and the corner-style factor.

/// `shape.xs`.
pub const XS: f32 = 4.0;
/// `shape.sm`.
pub const SM: f32 = 8.0;
/// `shape.md`.
pub const MD: f32 = 12.0;
/// `shape.lg`.
pub const LG: f32 = 16.0;
/// `shape.xl`.
pub const XL: f32 = 28.0;
/// `shape.full`.
pub const FULL: f32 = 999.0;

use std::sync::atomic::{AtomicU32, Ordering};

/// Corner scale in percent (the "corner style" setting); 100 is the M3 scale.
static SCALE_PERCENT: AtomicU32 = AtomicU32::new(100);

/// Set the corner scale in percent (clamped to 0 to 200).
pub fn set_scale_percent(p: u32) {
    SCALE_PERCENT.store(p.min(200), Ordering::Relaxed);
}

/// The corner scale as a factor.
#[must_use]
pub fn scale() -> f32 {
    // At most 200, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let p = SCALE_PERCENT.load(Ordering::Relaxed) as f32;
    p / 100.0
}

/// `shape.xs` scaled by the corner style.
#[must_use]
pub fn xs() -> f32 {
    XS * scale()
}
/// `shape.sm` scaled by the corner style.
#[must_use]
pub fn sm() -> f32 {
    SM * scale()
}
/// `shape.md` scaled by the corner style.
#[must_use]
pub fn md() -> f32 {
    MD * scale()
}
/// `shape.lg` scaled by the corner style.
#[must_use]
pub fn lg() -> f32 {
    LG * scale()
}
/// `shape.xl` scaled by the corner style.
#[must_use]
pub fn xl() -> f32 {
    XL * scale()
}
/// Any radius scaled by the corner style (pills stay pills).
#[must_use]
pub fn scaled(r: f32) -> f32 {
    if r >= FULL { r } else { r * scale() }
}
