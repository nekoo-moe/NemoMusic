//! Themes: which surfaces (dark, AMOLED, light), which accent source (the cover or a curated
//! seed), how strong the accent is and how round the corners are. Pure data and maths; the GUI
//! applies it. Every palette keeps text contrast at 4.5:1 or better on its surfaces (tests below).

use crate::palette::{MAX_CHROMA, Oklch, Palette, Seed, build_palette_on, mix_rgb, oklch_to_rgb};
use crate::theme::Rgba;

mod choice;
mod fade;
#[cfg(test)]
mod tests;

pub use choice::{AccentIntensity, BaseMode, CornerStyle, ThemeChoice};
pub use fade::{BgFade, SchemeFade};

/// The accent palette for a choice. `cover` is the seed of the playing cover (used by
/// [`ThemeChoice::Dynamic`] only); without one the fallback palette shows.
#[must_use]
pub fn accent_palette(
    choice: ThemeChoice,
    base: BaseMode,
    intensity: AccentIntensity,
    cover: Option<Seed>,
) -> Palette {
    let n = base.neutrals();
    let seed = match choice.seed() {
        Some(s) => s,
        None => match cover {
            Some(s) => s,
            None => return Palette::fallback_on(&n),
        },
    };
    let scaled = Seed {
        hue: seed.hue,
        chroma: (seed.chroma * intensity.factor()).min(MAX_CHROMA),
    };
    build_palette_on(scaled, base.tones(), &n)
}

/// Three colours for a preview swatch: `primary`, `primary-container`, `tertiary`.
#[must_use]
pub fn swatch(choice: ThemeChoice, base: BaseMode, intensity: AccentIntensity) -> [Rgba; 3] {
    if choice == ThemeChoice::Dynamic {
        // A fixed three-hue sample of "whatever the cover is".
        let tones = base.tones();
        let at = |hue: f64| {
            oklch_to_rgb(Oklch {
                l: tones.primary / 100.0,
                c: 0.11 * intensity.factor().min(1.2),
                h: hue,
            })
        };
        return [at(25.0), at(145.0), at(265.0)];
    }
    let p = accent_palette(choice, base, intensity, None);
    [p.primary, p.primary_container, p.tertiary]
}

/// Mix helper kept for the swatch strip of the settings screen.
#[must_use]
pub fn blend(a: Rgba, b: Rgba, t: f64) -> Rgba {
    mix_rgb(a, b, t)
}

/// Alpha of the panes over the background field: they stay mostly opaque (text contrast is
/// checked against this exact mix) and let the colour of the song show through.
pub const PANE_ALPHA: f32 = 0.74;

/// The three colours of the window background for a seed (a cover, a page or a theme). Dark,
/// low chroma, and darker toward the corner opposite the light source. Without a seed the field
/// is a faint cool grey.
#[must_use]
pub fn bg_colors(seed: Option<Seed>, base: BaseMode) -> [Rgba; 3] {
    let s = seed.unwrap_or(Seed {
        hue: 262.0,
        chroma: 0.02,
    });
    // Chroma stays low whatever the cover: the field tints, the cover stays the colourful part.
    let c = (s.chroma * 0.55).min(0.045);
    let (l, shifts): ([f64; 3], [f64; 3]) = match base {
        BaseMode::Dark => ([0.27, 0.20, 0.14], [0.0, 38.0, -34.0]),
        BaseMode::Amoled => ([0.17, 0.11, 0.06], [0.0, 38.0, -34.0]),
        BaseMode::Light => ([0.93, 0.90, 0.95], [0.0, 38.0, -34.0]),
    };
    let chroma_scale = if base.is_light() { 0.6 } else { 1.0 };
    [0, 1, 2].map(|i| {
        oklch_to_rgb(Oklch {
            l: l[i],
            c: c * chroma_scale * [1.0, 0.8, 0.7][i],
            h: (s.hue + shifts[i]).rem_euclid(360.0),
        })
    })
}
