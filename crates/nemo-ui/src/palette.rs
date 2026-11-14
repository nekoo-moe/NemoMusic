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

mod extract;
mod oklch;
#[cfg(test)]
mod tests;

pub use extract::{PaletteError, dominant_color, from_image_bytes, from_rgba, seed_from_pixels};
pub use oklch::{Oklch, mix_rgb, oklab_to_rgb, oklch_to_rgb, rgb_to_oklab, rgb_to_oklch};

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
