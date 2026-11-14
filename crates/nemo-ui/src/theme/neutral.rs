//! The neutral and semantic colour roles of one surface mode.

use super::Rgba;
#[cfg(doc)]
use super::color;

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
