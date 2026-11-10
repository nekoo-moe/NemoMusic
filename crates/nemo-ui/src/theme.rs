//! Design tokens from `design.md` (dark baseline). No GUI dependency.
//!
//! Views must take every color, size and duration from here, never a literal.

/// An sRGB color with 8-bit channels and 8-bit alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel (255 is opaque).
    pub a: u8,
}

impl Rgba {
    /// Opaque color from a `0xRRGGBB` value.
    #[must_use]
    pub const fn from_rgb_hex(hex: u32) -> Self {
        let [_, r, g, b] = hex.to_be_bytes();
        Self { r, g, b, a: 255 }
    }

    /// Color from a `0xRRGGBBAA` value.
    #[must_use]
    pub const fn from_rgba_hex(hex: u32) -> Self {
        let [r, g, b, a] = hex.to_be_bytes();
        Self { r, g, b, a }
    }

    /// Same color with alpha set from a 0.0 to 1.0 fraction (clamped).
    #[must_use]
    pub fn with_alpha(self, alpha: f32) -> Self {
        Self {
            // Clamped to 0..=255 first, so the cast cannot truncate or wrap.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            a: (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
            ..self
        }
    }

    /// Alpha as a 0.0 to 1.0 fraction.
    #[must_use]
    pub fn alpha(self) -> f32 {
        f32::from(self.a) / 255.0
    }

    /// Composite `self` over an opaque `backdrop`; the result is opaque.
    #[must_use]
    pub fn over(self, backdrop: Rgba) -> Rgba {
        let al = self.alpha();
        let mix = |f: u8, b: u8| {
            let v = f32::from(f) * al + f32::from(b) * (1.0 - al);
            // Convex mix of two u8 values stays within 0..=255.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let out = v.round() as u8;
            out
        };
        Rgba {
            r: mix(self.r, backdrop.r),
            g: mix(self.g, backdrop.g),
            b: mix(self.b, backdrop.b),
            a: 255,
        }
    }

    /// WCAG 2.x relative luminance (alpha ignored).
    #[must_use]
    pub fn relative_luminance(self) -> f64 {
        let lin = |c: u8| {
            let s = f64::from(c) / 255.0;
            if s <= 0.039_28 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(self.r) + 0.7152 * lin(self.g) + 0.0722 * lin(self.b)
    }
}

/// WCAG 2.x contrast ratio between two colors, from 1.0 to 21.0. Alpha is ignored:
/// composite with [`Rgba::over`] first.
#[must_use]
pub fn contrast_ratio(a: Rgba, b: Rgba) -> f64 {
    let (la, lb) = (a.relative_luminance(), b.relative_luminance());
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Color roles of the neutral dark baseline (`design.md` section 2).
pub mod color {
    use super::Rgba;

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
}

/// Spacing scale in logical px (`design.md` section 4). Unit is 4.
pub mod spacing {
    /// Base unit.
    pub const UNIT: u16 = 4;
    /// All allowed spacing values.
    pub const SCALE: [u16; 8] = [4, 8, 12, 16, 24, 32, 48, 64];
    /// 4 px.
    pub const XS: u16 = 4;
    /// 8 px.
    pub const SM: u16 = 8;
    /// 12 px; also the player bar inset.
    pub const MD: u16 = 12;
    /// 16 px.
    pub const LG: u16 = 16;
    /// 24 px; content padding.
    pub const XL: u16 = 24;
}

/// Shape (corner radius) scale in logical px.
pub mod shape {
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
}

/// Layout numbers (`design.md` section 4).
pub mod layout {
    /// Player bar height.
    pub const PLAYER_BAR_HEIGHT: f32 = 88.0;
    /// Player bar inset from the window edges.
    pub const PLAYER_BAR_INSET: f32 = 12.0;
    /// Default window size.
    pub const DEFAULT_WINDOW: (f32, f32) = (1280.0, 800.0);
    /// Minimum window size.
    pub const MIN_WINDOW: (f32, f32) = (960.0, 600.0);
}

/// Non-spring fade durations in milliseconds (`design.md` section 8).
pub mod motion {
    /// `motion.fast`.
    pub const FAST_MS: u32 = 100;
    /// `motion.base`.
    pub const BASE_MS: u32 = 200;
    /// `motion.slow`; nothing animates longer.
    pub const SLOW_MS: u32 = 400;
    /// Accent crossfade.
    pub const ACCENT_CROSSFADE_MS: u32 = 400;
    /// 60 fps frame budget in milliseconds.
    pub const FRAME_BUDGET_MS: f64 = 16.6;
}

/// Glass tokens (`design.md` section 6). Tier Low draws no live blur.
pub mod glass {
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
    /// Edge highlight base color (white).
    pub const EDGE: Rgba = Rgba::from_rgb_hex(0xFF_FFFF);
}

/// One row of the type scale: size and line height in px.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextStyle {
    /// Font size in px.
    pub size: u16,
    /// Line height in px.
    pub line: u16,
}

/// Type scale (`design.md` section 3).
pub mod type_scale {
    use super::TextStyle;

    /// `label-sm` 11/16.
    pub const LABEL_SM: TextStyle = TextStyle { size: 11, line: 16 };
    /// `body-sm` 12/18.
    pub const BODY_SM: TextStyle = TextStyle { size: 12, line: 18 };
    /// `body-md` 14/20.
    pub const BODY_MD: TextStyle = TextStyle { size: 14, line: 20 };
    /// `title-sm` 16/24.
    pub const TITLE_SM: TextStyle = TextStyle { size: 16, line: 24 };
    /// `title-md` 20/28.
    pub const TITLE_MD: TextStyle = TextStyle { size: 20, line: 28 };
    /// `label-md` 12/16.
    pub const LABEL_MD: TextStyle = TextStyle { size: 12, line: 16 };
    /// `title-lg` 22/28: shelf and section headers.
    pub const TITLE_LG: TextStyle = TextStyle { size: 22, line: 28 };
    /// `headline-sm` 24/32.
    pub const HEADLINE_SM: TextStyle = TextStyle { size: 24, line: 32 };
    /// `headline-md` 28/36.
    pub const HEADLINE_MD: TextStyle = TextStyle { size: 28, line: 36 };
    /// `headline-lg` 32/40.
    pub const HEADLINE_LG: TextStyle = TextStyle { size: 32, line: 40 };
    /// `display-md` 40/48.
    pub const DISPLAY_MD: TextStyle = TextStyle { size: 40, line: 48 };
    /// Regular weight.
    pub const WEIGHT_REGULAR: u16 = 400;
    /// Medium weight (buttons, chips, navigation labels).
    pub const WEIGHT_MEDIUM: u16 = 500;
    /// Semibold weight.
    pub const WEIGHT_SEMIBOLD: u16 = 600;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrast_known_values() {
        let white = Rgba::from_rgb_hex(0xFF_FFFF);
        let black = Rgba::from_rgb_hex(0x00_0000);
        assert!((contrast_ratio(white, black) - 21.0).abs() < 1e-9);
        assert!((contrast_ratio(white, white) - 1.0).abs() < 1e-9);
        // Symmetric.
        assert!((contrast_ratio(black, white) - contrast_ratio(white, black)).abs() < 1e-12);
    }

    #[test]
    fn design_pairs_meet_aa_over_surface() {
        let pairs = [
            ("on-surface", color::ON_SURFACE),
            ("on-surface-variant", color::ON_SURFACE_VARIANT),
            ("success", color::SUCCESS),
            ("warning", color::WARNING),
            ("error", color::ERROR),
        ];
        for (name, fg) in pairs {
            let ratio = contrast_ratio(fg, color::SURFACE);
            assert!(ratio >= 4.5, "{name} over surface is {ratio:.2}:1");
        }
    }

    #[test]
    fn info_and_fallback_primary_meet_aa_over_surface() {
        for fg in [color::INFO, color::PRIMARY_FALLBACK] {
            assert!(contrast_ratio(fg, color::SURFACE) >= 4.5);
        }
    }

    #[test]
    fn spacing_is_multiple_of_four() {
        for v in spacing::SCALE {
            assert_eq!(v % spacing::UNIT, 0, "{v} is not a multiple of 4");
        }
        for v in [
            spacing::XS,
            spacing::SM,
            spacing::MD,
            spacing::LG,
            spacing::XL,
        ] {
            assert!(spacing::SCALE.contains(&v));
        }
    }

    #[test]
    fn glass_tint_over_surface_keeps_text_legible() {
        let tint = glass::TINT_BASE.with_alpha(glass::TINT_ALPHA);
        let bar = tint.over(color::SURFACE);
        assert!(contrast_ratio(color::ON_SURFACE, bar) >= 4.5);
    }

    #[test]
    fn alpha_round_trip() {
        assert_eq!(Rgba::from_rgb_hex(0).with_alpha(0.55).a, 140);
        assert_eq!(Rgba::from_rgb_hex(0).with_alpha(2.0).a, 255);
        assert_eq!(color::OUTLINE_VARIANT.a, 0x14);
    }
}
