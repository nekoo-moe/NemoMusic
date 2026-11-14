//! Colour roles: the active neutral set, its accessors and the dark baseline constants.

use std::sync::RwLock;

use super::{Neutrals, Rgba};

static ACTIVE: RwLock<Neutrals> = RwLock::new(Neutrals::DARK);

/// Make `n` the neutral set every view draws with.
pub fn set_active(n: Neutrals) {
    if let Ok(mut g) = ACTIVE.write() {
        *g = n;
    }
}

/// The neutral set in use now.
#[must_use]
pub fn active() -> Neutrals {
    ACTIVE.read().map_or(Neutrals::DARK, |g| *g)
}

/// Active `surface`.
#[must_use]
pub fn surface() -> Rgba {
    active().surface
}
/// Active `surface-container-lowest`.
#[must_use]
pub fn surface_container_lowest() -> Rgba {
    active().lowest
}
/// Active `surface-container-low`.
#[must_use]
pub fn surface_container_low() -> Rgba {
    active().low
}
/// Active `surface-container`.
#[must_use]
pub fn surface_container() -> Rgba {
    active().container
}
/// Active `surface-container-high`.
#[must_use]
pub fn surface_container_high() -> Rgba {
    active().high
}
/// Active `surface-container-highest`.
#[must_use]
pub fn surface_container_highest() -> Rgba {
    active().highest
}
/// Active `outline`.
#[must_use]
pub fn outline() -> Rgba {
    active().outline
}
/// Active `on-surface`.
#[must_use]
pub fn on_surface() -> Rgba {
    active().on_surface
}
/// Active `on-surface-variant`.
#[must_use]
pub fn on_surface_variant() -> Rgba {
    active().on_surface_variant
}
/// Active `outline-variant`.
#[must_use]
pub fn outline_variant() -> Rgba {
    active().outline_variant
}
/// Active disabled text colour.
#[must_use]
pub fn disabled() -> Rgba {
    active().disabled
}
/// Active success colour.
#[must_use]
pub fn success() -> Rgba {
    active().success
}
/// Active warning colour.
#[must_use]
pub fn warning() -> Rgba {
    active().warning
}
/// Active error colour.
#[must_use]
pub fn error() -> Rgba {
    active().error
}
/// Active info colour.
#[must_use]
pub fn info() -> Rgba {
    active().info
}

/// `surface`.
pub const SURFACE: Rgba = Rgba::from_rgb_hex(0x0E_0F12);
/// `surface-container-lowest`: the window behind the three panes.
pub const SURFACE_CONTAINER_LOWEST: Rgba = Rgba::from_rgb_hex(0x09_0A0C);
/// `surface-container-low` (between `surface` and `surface-container`): the sidebar.
pub const SURFACE_CONTAINER_LOW: Rgba = Rgba::from_rgb_hex(0x12_1418);
/// `surface-container`.
pub const SURFACE_CONTAINER: Rgba = Rgba::from_rgb_hex(0x16_181D);
/// `surface-container-high`.
pub const SURFACE_CONTAINER_HIGH: Rgba = Rgba::from_rgb_hex(0x1E_2128);
/// `surface-container-highest`: chips, search pill, floating capsule base.
pub const SURFACE_CONTAINER_HIGHEST: Rgba = Rgba::from_rgb_hex(0x27_2A32);
/// `outline` (white at about 18%): chip borders.
pub const OUTLINE: Rgba = Rgba::from_rgba_hex(0xFFFF_FF2E);
/// `on-surface`.
pub const ON_SURFACE: Rgba = Rgba::from_rgb_hex(0xF2_F3F5);
/// `on-surface-variant`.
pub const ON_SURFACE_VARIANT: Rgba = Rgba::from_rgb_hex(0xA6_ABB5);
/// `outline-variant` (white at about 8%).
pub const OUTLINE_VARIANT: Rgba = Rgba::from_rgba_hex(0xFFFF_FF14);
/// Disabled text; never for content.
pub const DISABLED: Rgba = Rgba::from_rgb_hex(0x6B_7080);
/// Semantic success.
pub const SUCCESS: Rgba = Rgba::from_rgb_hex(0x6F_CF97);
/// Semantic warning.
pub const WARNING: Rgba = Rgba::from_rgb_hex(0xE5_B567);
/// Semantic error.
pub const ERROR: Rgba = Rgba::from_rgb_hex(0xEB_7B7B);
/// Semantic info.
pub const INFO: Rgba = Rgba::from_rgb_hex(0x7F_B2E5);
/// Fallback `primary` when a derived palette fails contrast.
pub const PRIMARY_FALLBACK: Rgba = Rgba::from_rgb_hex(0x8A_B4F8);
/// `scrim` base (black); use at alpha 0.60 or more under text.
pub const SCRIM: Rgba = Rgba::from_rgb_hex(0x00_0000);
