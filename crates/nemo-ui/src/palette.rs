//! Dynamic color from cover art (`design.md` section 2). No GUI dependency.
//!
//! Pipeline: decode, downsample to 64 px, drop near-grey pixels, median cut, pick the dominant
//! colour, build a dark tonal palette in OKLCH (chroma at most 0.14), then enforce 4.5:1 for every
//! text pair: move the foreground tone in 1-step increments up to 10 steps, and if a pair still
//! fails use the fallback palette (`primary` `#8AB4F8`).

#![allow(
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::manual_midpoint
)] // colour and easing maths: short names are the standard notation, casts are of small bounded values

use crate::theme::{Neutrals, Rgba, color, contrast_ratio};

/// Highest chroma of any accent role.
pub const MAX_CHROMA: f64 = 0.14;
/// Lowest seed chroma of a coloured cover, so the accent never looks washed out.
pub const MIN_SEED_CHROMA: f64 = 0.06;
/// Pixels with less chroma than this are "near-grey" and ignored.
pub const GREY_CHROMA: f64 = 0.04;
/// Edge length (px) the cover is shrunk to before analysis.
pub const SAMPLE_PX: u32 = 64;
/// Minimum contrast of text pairs.
pub const TEXT_CONTRAST: f64 = 4.5;
/// Minimum contrast of non-text pairs (seek fill, focus ring against the surface).
pub const UI_CONTRAST: f64 = 3.0;
/// Largest number of 1-step tone moves when a pair fails.
pub const MAX_TONE_STEPS: u32 = 10;
/// Fraction of non-grey pixels below which the whole image counts as grey.
const GREY_IMAGE_FRACTION: f64 = 0.02;
/// Number of boxes median cut ends with.
const MEDIAN_CUT_BOXES: usize = 8;

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

/// The accent and tint roles derived from a cover (dark scheme). Neutral roles stay fixed in
/// [`crate::theme::color`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Accent for the play button, seek fill and focus ring (tone 80).
    pub primary: Rgba,
    /// Content on `primary` (tone 20).
    pub on_primary: Rgba,
    /// Container filled with the accent (tone 30).
    pub primary_container: Rgba,
    /// Content on `primary_container` (tone 90).
    pub on_primary_container: Rgba,
    /// Quieter container (tone 30, low chroma).
    pub secondary_container: Rgba,
    /// Content on `secondary_container` (tone 90).
    pub on_secondary_container: Rgba,
    /// Second accent, hue shifted (tone 80).
    pub tertiary: Rgba,
    /// Surface tinted by the cover (tone 6).
    pub surface_tint: Rgba,
    /// `true` when this is the fallback palette.
    pub is_fallback: bool,
}

/// Tones (0 to 100, Oklab lightness x 100) of the accent roles. Public so tests can feed bad
/// values to the contrast search.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tones {
    /// `primary`.
    pub primary: f64,
    /// `on-primary`.
    pub on_primary: f64,
    /// `primary-container`.
    pub primary_container: f64,
    /// `on-primary-container`.
    pub on_primary_container: f64,
    /// `secondary-container`.
    pub secondary_container: f64,
    /// `on-secondary-container`.
    pub on_secondary_container: f64,
    /// `tertiary`.
    pub tertiary: f64,
    /// `surface` tint.
    pub surface_tint: f64,
}

impl Tones {
    /// Dark scheme tones from `design.md` section 2.
    pub const DARK: Self = Self {
        primary: 80.0,
        on_primary: 20.0,
        primary_container: 30.0,
        on_primary_container: 90.0,
        secondary_container: 30.0,
        on_secondary_container: 90.0,
        tertiary: 80.0,
        surface_tint: 6.0,
    };

    /// AMOLED: the dark tones with a nearly black tint.
    pub const AMOLED: Self = Self {
        surface_tint: 3.0,
        ..Self::DARK
    };

    /// Light scheme tones: the accent is dark enough for white text, containers are pale.
    pub const LIGHT: Self = Self {
        primary: 40.0,
        on_primary: 100.0,
        primary_container: 90.0,
        on_primary_container: 20.0,
        secondary_container: 90.0,
        on_secondary_container: 20.0,
        tertiary: 40.0,
        surface_tint: 94.0,
    };
}

/// Seed of a palette: hue in degrees and chroma (already clamped to [`MAX_CHROMA`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Seed {
    /// Hue in degrees.
    pub hue: f64,
    /// Chroma, 0 for a grey cover.
    pub chroma: f64,
}

fn role(seed: Seed, chroma_scale: f64, hue_shift: f64, tone: f64) -> Rgba {
    oklch_to_rgb(Oklch {
        l: tone.clamp(0.0, 100.0) / 100.0,
        c: (seed.chroma * chroma_scale).min(MAX_CHROMA),
        h: (seed.hue + hue_shift).rem_euclid(360.0),
    })
}

fn render(seed: Seed, t: &Tones) -> Palette {
    Palette {
        primary: role(seed, 1.0, 0.0, t.primary),
        on_primary: role(seed, 0.3, 0.0, t.on_primary),
        primary_container: role(seed, 0.8, 0.0, t.primary_container),
        on_primary_container: role(seed, 0.2, 0.0, t.on_primary_container),
        secondary_container: role(seed, 0.35, 0.0, t.secondary_container),
        on_secondary_container: role(seed, 0.15, 0.0, t.on_secondary_container),
        tertiary: role(seed, 0.8, 60.0, t.tertiary),
        surface_tint: role(seed, 0.12, 0.0, t.surface_tint),
        is_fallback: false,
    }
}

impl Palette {
    /// Every (name, foreground, background) pair that must reach 4.5:1, on the dark baseline.
    #[must_use]
    pub fn text_pairs(&self) -> [(&'static str, Rgba, Rgba); 8] {
        self.text_pairs_on(&Neutrals::DARK)
    }

    /// Every (name, foreground, background) pair that must reach 4.5:1 on the neutral set `n`.
    #[must_use]
    pub fn text_pairs_on(&self, n: &Neutrals) -> [(&'static str, Rgba, Rgba); 8] {
        [
            ("on-primary / primary", self.on_primary, self.primary),
            (
                "on-primary-container / primary-container",
                self.on_primary_container,
                self.primary_container,
            ),
            (
                "on-secondary-container / secondary-container",
                self.on_secondary_container,
                self.secondary_container,
            ),
            ("primary / surface", self.primary, n.surface),
            ("tertiary / surface", self.tertiary, n.surface),
            ("primary / surface-tint", self.primary, self.surface_tint),
            ("on-surface / surface-tint", n.on_surface, self.surface_tint),
            (
                "on-surface-variant / surface-tint",
                n.on_surface_variant,
                self.surface_tint,
            ),
        ]
    }

    /// Worst contrast over [`Self::text_pairs`].
    #[must_use]
    pub fn worst_contrast(&self) -> f64 {
        self.worst_contrast_on(&Neutrals::DARK)
    }

    /// Worst contrast over [`Self::text_pairs_on`].
    #[must_use]
    pub fn worst_contrast_on(&self, n: &Neutrals) -> f64 {
        self.text_pairs_on(n)
            .iter()
            .map(|(_, f, b)| contrast_ratio(*f, *b))
            .fold(f64::INFINITY, f64::min)
    }

    /// `true` when every text pair reaches 4.5:1 and `primary` reaches 3:1 as a non-text colour.
    #[must_use]
    pub fn passes(&self) -> bool {
        self.passes_on(&Neutrals::DARK)
    }

    /// [`Self::passes`] on the neutral set `n`.
    #[must_use]
    pub fn passes_on(&self, n: &Neutrals) -> bool {
        self.text_pairs_on(n)
            .iter()
            .all(|(_, f, b)| contrast_ratio(*f, *b) >= TEXT_CONTRAST)
            && contrast_ratio(self.primary, n.surface) >= UI_CONTRAST
            && contrast_ratio(self.primary, self.surface_tint) >= UI_CONTRAST
    }

    /// The fallback palette, derived from `#8AB4F8` so it follows the same tone rules.
    #[must_use]
    pub fn fallback() -> Self {
        let lch = rgb_to_oklch(color::PRIMARY_FALLBACK);
        let seed = Seed {
            hue: lch.h,
            chroma: lch.c.min(MAX_CHROMA),
        };
        let mut p = render(seed, &Tones::DARK);
        // The brief fixes `primary`; the other roles follow the tone rules.
        p.primary = color::PRIMARY_FALLBACK;
        p.is_fallback = true;
        p
    }

    /// The fallback for another surface mode: the dark fallback on dark surfaces, a tone-40
    /// derivation of the same hue on light ones.
    #[must_use]
    pub fn fallback_on(n: &Neutrals) -> Self {
        if n.surface.relative_luminance() < 0.5 {
            return Self::fallback();
        }
        let lch = rgb_to_oklch(color::PRIMARY_FALLBACK);
        let seed = Seed {
            hue: lch.h,
            chroma: lch.c.min(MAX_CHROMA),
        };
        let mut p = render(seed, &Tones::LIGHT);
        p.is_fallback = true;
        p
    }

    /// Mix two palettes in Oklab; `t` is clamped to 0 to 1. `t = 0` gives `self`, `t = 1` `other`.
    #[must_use]
    pub fn mix(&self, other: &Palette, t: f32) -> Palette {
        let t = f64::from(t.clamp(0.0, 1.0));
        let m = |a: Rgba, b: Rgba| mix_rgb(a, b, t);
        Palette {
            primary: m(self.primary, other.primary),
            on_primary: m(self.on_primary, other.on_primary),
            primary_container: m(self.primary_container, other.primary_container),
            on_primary_container: m(self.on_primary_container, other.on_primary_container),
            secondary_container: m(self.secondary_container, other.secondary_container),
            on_secondary_container: m(self.on_secondary_container, other.on_secondary_container),
            tertiary: m(self.tertiary, other.tertiary),
            surface_tint: m(self.surface_tint, other.surface_tint),
            is_fallback: if t < 0.5 {
                self.is_fallback
            } else {
                other.is_fallback
            },
        }
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::fallback()
    }
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

/// Build a palette from a seed, enforcing contrast. Each failing foreground moves one tone step
/// at a time (away from its background) for at most [`MAX_TONE_STEPS`] steps; if a pair still
/// fails the fallback palette is returned.
#[must_use]
pub fn build_palette(seed: Seed, tones: Tones) -> Palette {
    build_palette_on(seed, tones, &Neutrals::DARK)
}

/// [`build_palette`] against the neutral set `n` (dark, AMOLED or light surfaces). The fallback
/// when a pair cannot be fixed is [`Palette::fallback_on`].
#[must_use]
pub fn build_palette_on(seed: Seed, tones: Tones, n: &Neutrals) -> Palette {
    let mut t = tones;
    // Two passes: moving `primary` for the surface check can change its own pair with on-primary.
    for _ in 0..2 {
        let p = render(seed, &t);
        t.on_primary = fix_tone(
            t.on_primary,
            |tone| {
                render(
                    seed,
                    &Tones {
                        on_primary: tone,
                        ..t
                    },
                )
                .on_primary
            },
            p.primary,
        );
        t.on_primary_container = fix_tone(
            t.on_primary_container,
            |tone| {
                render(
                    seed,
                    &Tones {
                        on_primary_container: tone,
                        ..t
                    },
                )
                .on_primary_container
            },
            p.primary_container,
        );
        t.on_secondary_container = fix_tone(
            t.on_secondary_container,
            |tone| {
                render(
                    seed,
                    &Tones {
                        on_secondary_container: tone,
                        ..t
                    },
                )
                .on_secondary_container
            },
            p.secondary_container,
        );
        t.primary = fix_tone(
            t.primary,
            |tone| render(seed, &Tones { primary: tone, ..t }).primary,
            n.surface,
        );
        t.tertiary = fix_tone(
            t.tertiary,
            |tone| {
                render(
                    seed,
                    &Tones {
                        tertiary: tone,
                        ..t
                    },
                )
                .tertiary
            },
            n.surface,
        );
    }
    let p = render(seed, &t);
    if p.passes_on(n) {
        p
    } else {
        Palette::fallback_on(n)
    }
}

/// Move `tone` away from the background in 1-step increments until `make(tone)` reaches 4.5:1
/// against `bg`, at most [`MAX_TONE_STEPS`] steps. Returns the last tone tried.
fn fix_tone(tone: f64, make: impl Fn(f64) -> Rgba, bg: Rgba) -> f64 {
    let reaches = |t: f64| contrast_ratio(make(t), bg) >= TEXT_CONTRAST;
    if reaches(tone) {
        return tone;
    }
    // Away from the background: lighter foreground on a dark background, darker on a light one.
    let dir = if make(tone).relative_luminance() >= bg.relative_luminance() {
        1.0
    } else {
        -1.0
    };
    let mut cur = tone;
    for _ in 0..MAX_TONE_STEPS {
        cur = (cur + dir).clamp(0.0, 100.0);
        if reaches(cur) {
            return cur;
        }
    }
    cur
}

/// Why a palette could not be derived from image bytes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PaletteError {
    /// The bytes are not a supported image.
    #[error("could not decode cover image")]
    Decode,
    /// The pixel buffer does not match the stated size.
    #[error("pixel buffer does not match the image size")]
    BadBuffer,
}

/// Decode `bytes` (JPEG, PNG or WebP), shrink to 64 px and derive the palette.
///
/// # Errors
/// [`PaletteError::Decode`] when the bytes are not a supported image.
pub fn from_image_bytes(bytes: &[u8]) -> Result<Palette, PaletteError> {
    let img = image::load_from_memory(bytes).map_err(|_| PaletteError::Decode)?;
    let small = img.thumbnail(SAMPLE_PX, SAMPLE_PX).to_rgba8();
    from_rgba(small.width(), small.height(), small.as_raw())
}

/// Derive the palette from raw RGBA8 pixels (any size; large images are not resized here, so
/// callers shrink first).
///
/// # Errors
/// [`PaletteError::BadBuffer`] when `pixels.len() != width * height * 4`.
pub fn from_rgba(width: u32, height: u32, pixels: &[u8]) -> Result<Palette, PaletteError> {
    let expected = u64::from(width) * u64::from(height) * 4;
    if u64::try_from(pixels.len()).ok() != Some(expected) {
        return Err(PaletteError::BadBuffer);
    }
    let seed = seed_from_pixels(pixels);
    Ok(build_palette(seed, Tones::DARK))
}

/// Pick the seed (hue, chroma) of a cover from RGBA8 pixels.
#[must_use]
pub fn seed_from_pixels(pixels: &[u8]) -> Seed {
    match dominant_color(pixels) {
        Some(c) => {
            let lch = rgb_to_oklch(c);
            Seed {
                hue: lch.h,
                chroma: lch.c.clamp(MIN_SEED_CHROMA, MAX_CHROMA),
            }
        }
        // Whole image is grey (or empty): a neutral accent with no hue.
        None => Seed {
            hue: 0.0,
            chroma: 0.0,
        },
    }
}

/// Dominant non-grey colour by median cut, or `None` when the whole image is grey (or has no
/// opaque pixel). Boxes are scored by `count x (chroma + 0.04)`, so a small vivid area can win
/// over a large muddy one without letting noise dominate.
#[must_use]
pub fn dominant_color(pixels: &[u8]) -> Option<Rgba> {
    let mut colored: Vec<[u8; 3]> = Vec::new();
    let mut opaque = 0_usize;
    for px in pixels.as_chunks::<4>().0 {
        if px[3] < 128 {
            continue;
        }
        opaque += 1;
        let c = Rgba {
            r: px[0],
            g: px[1],
            b: px[2],
            a: 255,
        };
        if rgb_to_oklch(c).c >= GREY_CHROMA {
            colored.push([px[0], px[1], px[2]]);
        }
    }
    if opaque == 0 {
        return None;
    }
    // Counts are tiny (at most 4096), so the float conversion is exact.
    #[allow(clippy::cast_precision_loss)]
    let fraction = colored.len() as f64 / opaque as f64;
    if colored.is_empty() || fraction < GREY_IMAGE_FRACTION {
        return None;
    }
    let boxes = median_cut(colored, MEDIAN_CUT_BOXES);
    boxes
        .iter()
        .map(|b| {
            let avg = average(b);
            #[allow(clippy::cast_precision_loss)]
            let score = b.len() as f64 * (rgb_to_oklch(avg).c + 0.04);
            (score, avg)
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, c)| c)
}

fn average(pixels: &[[u8; 3]]) -> Rgba {
    let n = pixels.len().max(1) as u64;
    let sum = pixels.iter().fold([0_u64; 3], |mut acc, p| {
        for (a, v) in acc.iter_mut().zip(p) {
            *a += u64::from(*v);
        }
        acc
    });
    // The mean of u8 values fits in u8.
    #[allow(clippy::cast_possible_truncation)]
    let ch = |i: usize| (sum[i] / n) as u8;
    Rgba {
        r: ch(0),
        g: ch(1),
        b: ch(2),
        a: 255,
    }
}

/// Split the pixel set at the median of its widest channel until there are `target` boxes (or no
/// box can be split).
fn median_cut(pixels: Vec<[u8; 3]>, target: usize) -> Vec<Vec<[u8; 3]>> {
    let mut boxes = vec![pixels];
    while boxes.len() < target {
        // Split the box with the widest channel range among those with 2+ pixels.
        let pick = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() >= 2)
            .map(|(i, b)| (i, widest_channel(b)))
            .max_by_key(|(_, (_, range))| *range);
        let Some((index, (channel, range))) = pick else {
            break;
        };
        if range == 0 {
            break;
        }
        let mut b = boxes.swap_remove(index);
        b.sort_unstable_by_key(|p| p[channel]);
        let upper = b.split_off(b.len() / 2);
        boxes.push(b);
        boxes.push(upper);
    }
    boxes
}

fn widest_channel(pixels: &[[u8; 3]]) -> (usize, u8) {
    (0..3)
        .map(|ch| {
            let (lo, hi) = pixels.iter().fold((u8::MAX, u8::MIN), |(lo, hi), p| {
                (lo.min(p[ch]), hi.max(p[ch]))
            });
            (ch, hi - lo)
        })
        .max_by_key(|(_, range)| *range)
        .unwrap_or((0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba as ImgRgba, RgbaImage};
    use std::io::Cursor;

    fn png(img: &RgbaImage) -> Vec<u8> {
        let mut out = Vec::new();
        img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .expect("invariant: encoding a small RGBA image cannot fail");
        out
    }

    fn solid(r: u8, g: u8, b: u8) -> Vec<u8> {
        png(&RgbaImage::from_pixel(96, 96, ImgRgba([r, g, b, 255])))
    }

    /// Deterministic noise, no `rand` dependency.
    fn noise() -> Vec<u8> {
        let mut state = 0x1234_5678_u32;
        let mut img = RgbaImage::new(96, 96);
        for p in img.pixels_mut() {
            let mut next = || {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                state.to_be_bytes()[0]
            };
            *p = ImgRgba([next(), next(), next(), 255]);
        }
        png(&img)
    }

    fn accent_roles(p: &Palette) -> [Rgba; 6] {
        [
            p.primary,
            p.primary_container,
            p.secondary_container,
            p.tertiary,
            p.on_primary,
            p.surface_tint,
        ]
    }

    fn assert_good(name: &str, p: &Palette) {
        for (pair, fg, bg) in p.text_pairs() {
            let ratio = contrast_ratio(fg, bg);
            assert!(ratio >= TEXT_CONTRAST, "{name}: {pair} is {ratio:.2}:1");
        }
        assert!(
            contrast_ratio(p.primary, color::SURFACE) >= UI_CONTRAST,
            "{name}: primary non-text"
        );
        for r in accent_roles(p) {
            let c = rgb_to_oklch(r).c;
            assert!(c <= MAX_CHROMA + 0.01, "{name}: chroma {c:.3}");
        }
    }

    #[test]
    fn oklch_round_trips() {
        for hex in [
            0x80_80_80, 0xFF_00_00, 0x00_FF_00, 0x33_66_CC, 0xFF_FF_FF, 0x00_00_00,
        ] {
            let c = Rgba::from_rgb_hex(hex);
            let back = oklch_to_rgb(rgb_to_oklch(c));
            for (a, b) in [(c.r, back.r), (c.g, back.g), (c.b, back.b)] {
                assert!(a.abs_diff(b) <= 2, "{hex:06X}: {a} vs {b}");
            }
        }
    }

    #[test]
    fn all_black_and_all_white_and_grey_give_neutral_passing_palettes() {
        for (name, bytes) in [
            ("black", solid(0, 0, 0)),
            ("white", solid(255, 255, 255)),
            ("grey", solid(128, 128, 128)),
        ] {
            let p = from_image_bytes(&bytes).expect("decodes");
            assert_good(name, &p);
            assert!(!p.is_fallback, "{name} should not need the fallback");
            assert!(rgb_to_oklch(p.primary).c < 0.01, "{name} accent is neutral");
        }
    }

    #[test]
    fn saturated_covers_keep_their_hue() {
        // Oklch hues: red about 29, green about 142, blue about 264.
        for (name, rgb, hue) in [
            ("red", (230, 20, 30), 29.0),
            ("green", (20, 200, 60), 142.0),
            ("blue", (20, 40, 230), 264.0),
        ] {
            let p = from_image_bytes(&solid(rgb.0, rgb.1, rgb.2)).expect("decodes");
            assert_good(name, &p);
            let lch = rgb_to_oklch(p.primary);
            let diff = (lch.h - hue + 540.0).rem_euclid(360.0) - 180.0;
            assert!(diff.abs() < 15.0, "{name}: hue {:.0} vs {hue}", lch.h);
            assert!(lch.c > 0.05, "{name}: accent should be coloured");
        }
    }

    #[test]
    fn noise_gives_a_passing_palette() {
        let p = from_image_bytes(&noise()).expect("decodes");
        assert_good("noise", &p);
    }

    #[test]
    fn a_small_vivid_area_beats_a_large_grey_one() {
        let mut img = RgbaImage::from_pixel(64, 64, ImgRgba([120, 120, 120, 255]));
        for y in 0..16 {
            for x in 0..16 {
                img.put_pixel(x, y, ImgRgba([20, 60, 230, 255]));
            }
        }
        let p = from_image_bytes(&png(&img)).expect("decodes");
        let h = rgb_to_oklch(p.primary).h;
        assert!((h - 264.0).abs() < 20.0, "hue {h}");
    }

    #[test]
    fn large_images_are_shrunk_and_transparent_pixels_ignored() {
        let mut img = RgbaImage::from_pixel(400, 300, ImgRgba([0, 0, 0, 0]));
        for p in img.pixels_mut().take(30_000) {
            *p = ImgRgba([230, 20, 30, 255]);
        }
        let p = from_image_bytes(&png(&img)).expect("decodes");
        assert_good("transparent", &p);
    }

    #[test]
    fn bad_input_is_an_error_not_a_panic() {
        assert_eq!(from_image_bytes(b"not an image"), Err(PaletteError::Decode));
        assert_eq!(from_rgba(2, 2, &[0; 3]), Err(PaletteError::BadBuffer));
        assert!(from_rgba(0, 0, &[]).is_ok());
    }

    #[test]
    fn fallback_palette_passes_and_uses_the_brief_primary() {
        let f = Palette::fallback();
        assert_good("fallback", &f);
        assert_eq!(f.primary, color::PRIMARY_FALLBACK);
        assert!(f.is_fallback);
    }

    #[test]
    fn one_step_tone_search_repairs_a_slightly_low_tone() {
        let seed = Seed {
            hue: 264.0,
            chroma: 0.12,
        };
        // Tone 50 on the surface is below 4.5:1; the search moves it up within 10 steps.
        let bad = Tones {
            primary: 50.0,
            ..Tones::DARK
        };
        let broken = render(seed, &bad);
        assert!(contrast_ratio(broken.primary, color::SURFACE) < TEXT_CONTRAST);
        let fixed = build_palette(seed, bad);
        assert!(fixed.passes());
        assert!(
            !fixed.is_fallback,
            "a small miss should not need the fallback"
        );
    }

    #[test]
    fn a_hopeless_tone_uses_the_fallback() {
        let seed = Seed {
            hue: 264.0,
            chroma: 0.12,
        };
        let hopeless = Tones {
            on_primary: 80.0,
            primary: 80.0,
            primary_container: 85.0,
            on_primary_container: 82.0,
            ..Tones::DARK
        };
        // on-primary (tone 80) over primary (tone 80) is 1:1 and 10 steps cannot fix it.
        let p = build_palette(seed, hopeless);
        assert!(p.is_fallback);
        assert!(p.passes());
    }

    #[test]
    fn mix_endpoints_and_midpoints_stay_legible() {
        let red = from_image_bytes(&solid(230, 20, 30)).expect("decodes");
        let blue = from_image_bytes(&solid(20, 40, 230)).expect("decodes");
        assert_eq!(red.mix(&blue, 0.0), red);
        assert_eq!(red.mix(&blue, 1.0), blue);
        for i in 0..=20_u8 {
            let m = red.mix(&blue, f32::from(i) / 20.0);
            assert!(m.passes(), "step {i}: worst {:.2}", m.worst_contrast());
        }
    }
}
