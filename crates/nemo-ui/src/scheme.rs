//! Themes: which surfaces (dark, AMOLED, light), which accent source (the cover or a curated
//! seed), how strong the accent is and how round the corners are. Pure data and maths; the GUI
//! applies it. Every palette keeps text contrast at 4.5:1 or better on its surfaces (tests below).

use std::sync::atomic::{AtomicU32, Ordering};

use crate::palette::{MAX_CHROMA, Oklch, Palette, Seed, build_palette_on, mix_rgb, oklch_to_rgb};
use crate::theme::{Neutrals, Rgba};

mod choice;
mod fade;
#[cfg(test)]
mod tests;

pub use choice::{AccentIntensity, BaseMode, CornerStyle, ThemeChoice};
pub use fade::{BgFade, SchemeFade};

/// The seed the song gives the window: the playing cover's seed while `Match colours to the
/// playing song` is on (song-colour-theme spec), else none.
#[must_use]
pub fn song_seed(song_colour_theme: bool, cover: Option<Seed>) -> Option<Seed> {
    cover.filter(|_| song_colour_theme)
}

/// The accent palette for a choice. `song` is [`song_seed`]: when there is one it wins over the
/// theme (rule table in the song-colour-theme design); without it the theme's own seed shows,
/// and [`ThemeChoice::Dynamic`] has the fallback palette.
#[must_use]
pub fn accent_palette(
    choice: ThemeChoice,
    base: BaseMode,
    intensity: AccentIntensity,
    song: Option<Seed>,
) -> Palette {
    let n = base.neutrals();
    let Some(seed) = song.or(choice.seed()) else {
        return Palette::fallback_on(&n);
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

/// The seed of the window background: the song's ([`song_seed`]) when there is one, else the
/// theme's own. The page being viewed plays no part (living-backdrop spec), so the window follows
/// every song.
#[must_use]
pub fn field_seed(choice: ThemeChoice, song: Option<Seed>) -> Option<Seed> {
    song.or(choice.seed())
}

/// Tone (Oklab lightness x 100) of the pane tint: dark 8, AMOLED 4, light 94 (`design.md`
/// section 2, song-colour-theme).
const fn pane_tone(base: BaseMode) -> f64 {
    match base {
        BaseMode::Dark => 8.0,
        BaseMode::Amoled => 4.0,
        BaseMode::Light => 94.0,
    }
}

/// Most chroma of the pane tint: enough to read as the song's colour, low enough that the tint
/// stays a surface (the text contrast tests run at this value).
const PANE_TINT_CHROMA: f64 = 0.05;

/// The colour of the panes for a song seed: its hue at the pane tone of `base`, with half the
/// seed's chroma (at most [`PANE_TINT_CHROMA`]). A grey cover gives a neutral grey tint.
#[must_use]
pub fn pane_tint(seed: Seed, base: BaseMode) -> Rgba {
    oklch_to_rgb(Oklch {
        l: pane_tone(base) / 100.0,
        c: (seed.chroma * 0.5).min(PANE_TINT_CHROMA),
        h: seed.hue,
    })
}

/// The neutrals of the window. With a song seed the panes take [`pane_tint`] in place of the
/// neutral `surface-container`; without one they are the base mode's own neutrals. Text contrast
/// over the tinted pane is tested.
#[must_use]
pub fn window_neutrals(base: BaseMode, song: Option<Seed>) -> Neutrals {
    let mut n = base.neutrals();
    if let Some(seed) = song {
        n.container = pane_tint(seed, base);
    }
    n
}

/// Opacity of the page glow at its top (collection-page-ytm spec).
pub const GLOW_TOP_ALPHA: f32 = 0.92;
/// Opacity of the page glow at [`GLOW_MID_AT`] of its height; it is transparent at the bottom.
pub const GLOW_MID_ALPHA: f32 = 0.55;
/// Where the middle stop of the page glow sits, as a fraction of its height.
pub const GLOW_MID_AT: f32 = 0.45;

/// The two colours of the tall gradient behind an album or playlist page, from the page cover's
/// seed: the top colour, then a darker middle one (dark modes) that fades into the pane. Light
/// mode uses pale tints instead. Without a seed the glow is a quiet neutral.
#[must_use]
pub fn page_glow(seed: Option<Seed>, base: BaseMode) -> [Rgba; 2] {
    let s = seed.unwrap_or(Seed {
        hue: 262.0,
        chroma: 0.0,
    });
    let (l, c_max): ([f64; 2], f64) = match base {
        BaseMode::Dark => ([0.36, 0.27], 0.10),
        BaseMode::Amoled => ([0.31, 0.21], 0.10),
        BaseMode::Light => ([0.88, 0.92], 0.05),
    };
    [0, 1].map(|i| {
        oklch_to_rgb(Oklch {
            l: l[i],
            c: (s.chroma * [1.0, 0.85][i]).min(c_max),
            h: s.hue,
        })
    })
}

/// Alpha of the panes over the background field: they stay mostly opaque (text contrast is
/// checked against this exact mix, and it stays above the 0.60 scrim of `design.md` section 9)
/// and let the colour and the drift of the song show through (0.74 before the living-backdrop
/// spec).
pub const PANE_ALPHA: f32 = 0.66;

/// The pane alpha views draw with now, as `f32` bits: [`PANE_ALPHA`], or at least the glass
/// floor while the window is see-through (appearance-glass spec). A process-wide slot like the
/// active neutrals, set on the UI thread at start and after every settings change.
static PANE_ALPHA_NOW: AtomicU32 = AtomicU32::new(PANE_ALPHA.to_bits());

/// Alpha of the panes over the window backdrop now.
#[must_use]
pub fn pane_alpha() -> f32 {
    f32::from_bits(PANE_ALPHA_NOW.load(Ordering::Relaxed))
}

/// Make `alpha` (0 to 1) the alpha every pane draws with.
pub fn set_pane_alpha(alpha: f32) {
    PANE_ALPHA_NOW.store(alpha.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
}

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
