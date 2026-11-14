//! OKLCH and Oklab colour types and conversions to and from sRGB.

use crate::theme::Rgba;

/// A colour in OKLCH: lightness 0 to 1, chroma, hue in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklch {
    /// Perceptual lightness, 0.0 (black) to 1.0 (white).
    pub l: f64,
    /// Chroma (colourfulness); 0.0 is grey.
    pub c: f64,
    /// Hue angle in degrees, 0 to 360.
    pub h: f64,
}

fn to_linear(u: u8) -> f64 {
    let s = f64::from(u) / 255.0;
    if s <= 0.040_45 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

fn from_linear(v: f64) -> f64 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// sRGB to Oklab `(L, a, b)`.
#[must_use]
pub fn rgb_to_oklab(c: Rgba) -> (f64, f64, f64) {
    let (r, g, b) = (to_linear(c.r), to_linear(c.g), to_linear(c.b));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    (
        0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    )
}

/// Oklab to linear RGB, not clamped.
fn oklab_to_linear(l: f64, a: f64, b: f64) -> (f64, f64, f64) {
    let l_ = l + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m_ = l - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s_ = l - 0.089_484_177_5 * a - 1.291_485_548 * b;
    let (l3, m3, s3) = (l_.powi(3), m_.powi(3), s_.powi(3));
    (
        4.076_741_662_1 * l3 - 3.307_711_591_3 * m3 + 0.230_969_929_2 * s3,
        -1.268_438_004_6 * l3 + 2.609_757_401_1 * m3 - 0.341_319_396_5 * s3,
        -0.004_196_086_3 * l3 - 0.703_418_614_7 * m3 + 1.707_614_701 * s3,
    )
}

/// Convert an 8-bit unit float to `u8` (rounded, clamped).
fn to_u8(v: f64) -> u8 {
    // Clamped to 0..=255 first, so the cast cannot truncate or wrap.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let out = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    out
}

/// Oklab `(L, a, b)` to an opaque sRGB colour. Out-of-gamut values are clipped per channel.
#[must_use]
pub fn oklab_to_rgb(l: f64, a: f64, b: f64) -> Rgba {
    let (r, g, bl) = oklab_to_linear(l, a, b);
    Rgba {
        r: to_u8(from_linear(r)),
        g: to_u8(from_linear(g)),
        b: to_u8(from_linear(bl)),
        a: 255,
    }
}

/// sRGB to OKLCH.
#[must_use]
pub fn rgb_to_oklch(c: Rgba) -> Oklch {
    let (l, a, b) = rgb_to_oklab(c);
    let chroma = a.hypot(b);
    let hue = if chroma < 1e-6 {
        0.0
    } else {
        b.atan2(a).to_degrees().rem_euclid(360.0)
    };
    Oklch {
        l,
        c: chroma,
        h: hue,
    }
}

fn in_gamut(l: f64, c: f64, h: f64) -> bool {
    let (a, b) = (c * h.to_radians().cos(), c * h.to_radians().sin());
    let (r, g, bl) = oklab_to_linear(l, a, b);
    let eps = 1e-4;
    [r, g, bl].iter().all(|v| *v >= -eps && *v <= 1.0 + eps)
}

/// OKLCH to sRGB. When the colour is outside the sRGB gamut the chroma is reduced (lightness
/// and hue are kept), so the result never shifts hue.
#[must_use]
pub fn oklch_to_rgb(color: Oklch) -> Rgba {
    let l = color.l.clamp(0.0, 1.0);
    let mut c = color.c.max(0.0);
    if !in_gamut(l, c, color.h) {
        let (mut lo, mut hi) = (0.0, c);
        for _ in 0..24 {
            let mid = 0.5 * (lo + hi);
            if in_gamut(l, mid, color.h) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        c = lo;
    }
    let rad = color.h.to_radians();
    oklab_to_rgb(l, c * rad.cos(), c * rad.sin())
}

/// Mix two opaque colours in Oklab (`t` 0 to 1).
#[must_use]
pub fn mix_rgb(a: Rgba, b: Rgba, t: f64) -> Rgba {
    if t <= 0.0 {
        return a;
    }
    if t >= 1.0 {
        return b;
    }
    let (la, aa, ba) = rgb_to_oklab(a);
    let (lb, ab, bb) = rgb_to_oklab(b);
    oklab_to_rgb(la + (lb - la) * t, aa + (ab - aa) * t, ba + (bb - ba) * t)
}
