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

/// The neutral and semantic colour roles of one surface mode (dark, AMOLED, light). The active
/// set lives in a process-wide slot ([`color::set_active`]); views read it through the
/// [`color`] accessor functions, so switching the mode changes every screen at the next frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Neutrals {
    /// `surface`.
    pub surface: Rgba,
    /// `surface-container-lowest`.
    pub lowest: Rgba,
    /// `surface-container-low`.
    pub low: Rgba,
    /// `surface-container`.
    pub container: Rgba,
    /// `surface-container-high`.
    pub high: Rgba,
    /// `surface-container-highest`.
    pub highest: Rgba,
    /// `outline`.
    pub outline: Rgba,
    /// `on-surface`.
    pub on_surface: Rgba,
    /// `on-surface-variant`.
    pub on_surface_variant: Rgba,
    /// `outline-variant`.
    pub outline_variant: Rgba,
    /// Disabled text.
    pub disabled: Rgba,
    /// Semantic success.
    pub success: Rgba,
    /// Semantic warning.
    pub warning: Rgba,
    /// Semantic error.
    pub error: Rgba,
    /// Semantic info.
    pub info: Rgba,
}

impl Neutrals {
    /// The dark baseline of `design.md` section 2.
    pub const DARK: Self = Self {
        surface: Rgba::from_rgb_hex(0x0E_0F12),
        lowest: Rgba::from_rgb_hex(0x09_0A0C),
        low: Rgba::from_rgb_hex(0x12_1418),
        container: Rgba::from_rgb_hex(0x16_181D),
        high: Rgba::from_rgb_hex(0x1E_2128),
        highest: Rgba::from_rgb_hex(0x27_2A32),
        outline: Rgba::from_rgba_hex(0xFFFF_FF2E),
        on_surface: Rgba::from_rgb_hex(0xF2_F3F5),
        on_surface_variant: Rgba::from_rgb_hex(0xA6_ABB5),
        outline_variant: Rgba::from_rgba_hex(0xFFFF_FF14),
        disabled: Rgba::from_rgb_hex(0x6B_7080),
        success: Rgba::from_rgb_hex(0x6F_CF97),
        warning: Rgba::from_rgb_hex(0xE5_B567),
        error: Rgba::from_rgb_hex(0xEB_7B7B),
        info: Rgba::from_rgb_hex(0x7F_B2E5),
    };

    /// True black surfaces for OLED panels (owner request); the panes keep a faint lift so the
    /// layout still reads.
    pub const AMOLED: Self = Self {
        surface: Rgba::from_rgb_hex(0x00_0000),
        lowest: Rgba::from_rgb_hex(0x00_0000),
        low: Rgba::from_rgb_hex(0x07_0708),
        container: Rgba::from_rgb_hex(0x0C_0C0E),
        high: Rgba::from_rgb_hex(0x16_1619),
        highest: Rgba::from_rgb_hex(0x22_2226),
        ..Self::DARK
    };

    /// Light surfaces; semantic colours are the dark-on-light variants.
    pub const LIGHT: Self = Self {
        surface: Rgba::from_rgb_hex(0xFA_FAFD),
        lowest: Rgba::from_rgb_hex(0xE6_E8EE),
        low: Rgba::from_rgb_hex(0xF4_F5F9),
        container: Rgba::from_rgb_hex(0xF3_F4F8),
        high: Rgba::from_rgb_hex(0xE8_EAF0),
        highest: Rgba::from_rgb_hex(0xDD_DFE6),
        outline: Rgba::from_rgba_hex(0x0000_002E),
        on_surface: Rgba::from_rgb_hex(0x1A_1C20),
        on_surface_variant: Rgba::from_rgb_hex(0x45_4A55),
        outline_variant: Rgba::from_rgba_hex(0x0000_0014),
        disabled: Rgba::from_rgb_hex(0x8A_8F9C),
        success: Rgba::from_rgb_hex(0x1B_6E3F),
        warning: Rgba::from_rgb_hex(0x80_5300),
        error: Rgba::from_rgb_hex(0xB3_261E),
        info: Rgba::from_rgb_hex(0x1D_5799),
    };

    /// Mix two sets (`t` 0 to 1); opaque roles mix in Oklab, the two alpha roles per channel.
    #[must_use]
    pub fn mix(&self, other: &Self, t: f32) -> Self {
        let t = f64::from(t.clamp(0.0, 1.0));
        let m = |a: Rgba, b: Rgba| crate::palette::mix_rgb(a, b, t);
        let ma = |a: Rgba, b: Rgba| {
            let lerp = |x: u8, y: u8| {
                let v = f64::from(x) + (f64::from(y) - f64::from(x)) * t;
                // A convex mix of two bytes stays in 0..=255.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let out = v.round() as u8;
                out
            };
            Rgba {
                r: lerp(a.r, b.r),
                g: lerp(a.g, b.g),
                b: lerp(a.b, b.b),
                a: lerp(a.a, b.a),
            }
        };
        Self {
            surface: m(self.surface, other.surface),
            lowest: m(self.lowest, other.lowest),
            low: m(self.low, other.low),
            container: m(self.container, other.container),
            high: m(self.high, other.high),
            highest: m(self.highest, other.highest),
            outline: ma(self.outline, other.outline),
            on_surface: m(self.on_surface, other.on_surface),
            on_surface_variant: m(self.on_surface_variant, other.on_surface_variant),
            outline_variant: ma(self.outline_variant, other.outline_variant),
            disabled: m(self.disabled, other.disabled),
            success: m(self.success, other.success),
            warning: m(self.warning, other.warning),
            error: m(self.error, other.error),
            info: m(self.info, other.info),
        }
    }

    /// Every (name, foreground, background) text pair that must reach 4.5:1.
    #[must_use]
    pub fn text_pairs(&self) -> Vec<(&'static str, Rgba, Rgba)> {
        let mut out = Vec::new();
        for bg in [self.surface, self.container, self.high, self.highest] {
            for (name, fg) in [
                ("on-surface", self.on_surface),
                ("on-surface-variant", self.on_surface_variant),
                ("success", self.success),
                ("warning", self.warning),
                ("error", self.error),
                ("info", self.info),
            ] {
                out.push((name, fg, bg));
            }
        }
        out
    }
}

/// Color roles of the neutral dark baseline (`design.md` section 2). The constants are the dark
/// baseline; views call the lower-case functions, which read the active mode.
pub mod color {
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
    /// Glass tint base of the active mode.
    #[must_use]
    pub fn tint_base() -> Rgba {
        color::surface_container()
    }
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
