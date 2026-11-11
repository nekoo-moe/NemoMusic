//! Themes: which surfaces (dark, AMOLED, light), which accent source (the cover or a curated
//! seed), how strong the accent is and how round the corners are. Pure data and maths; the GUI
//! applies it. Every palette keeps text contrast at 4.5:1 or better on its surfaces (tests below).

use crate::motion::emphasized_decelerate;
use crate::palette::{
    MAX_CHROMA, Oklch, Palette, Seed, Tones, build_palette_on, mix_rgb, oklch_to_rgb,
};
use crate::theme::{Neutrals, Rgba, motion::ACCENT_CROSSFADE_MS};

/// Where the accent colours come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ThemeChoice {
    /// From the cover of the playing song (the original behaviour).
    #[default]
    Dynamic,
    /// Blue-teal seed.
    Ocean,
    /// Pink seed.
    Sakura,
    /// Green seed.
    Forest,
    /// Orange seed.
    Sunset,
    /// Violet seed.
    Lavender,
    /// No hue: grey accents.
    Mono,
}

impl ThemeChoice {
    /// All choices in menu order.
    pub const ALL: [Self; 7] = [
        Self::Dynamic,
        Self::Ocean,
        Self::Sakura,
        Self::Forest,
        Self::Sunset,
        Self::Lavender,
        Self::Mono,
    ];

    /// Menu label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Dynamic => "Dynamic",
            Self::Ocean => "Ocean",
            Self::Sakura => "Sakura",
            Self::Forest => "Forest",
            Self::Sunset => "Sunset",
            Self::Lavender => "Lavender",
            Self::Mono => "Mono",
        }
    }

    /// One line under the swatch.
    #[must_use]
    pub const fn note(self) -> &'static str {
        match self {
            Self::Dynamic => "From the cover",
            Self::Ocean => "Cool blue",
            Self::Sakura => "Soft pink",
            Self::Forest => "Calm green",
            Self::Sunset => "Warm orange",
            Self::Lavender => "Quiet violet",
            Self::Mono => "No colour",
        }
    }

    /// The curated seed, or `None` for [`Self::Dynamic`].
    #[must_use]
    pub const fn seed(self) -> Option<Seed> {
        let (hue, chroma) = match self {
            Self::Dynamic => return None,
            Self::Ocean => (232.0, 0.11),
            Self::Sakura => (355.0, 0.11),
            Self::Forest => (152.0, 0.10),
            Self::Sunset => (48.0, 0.13),
            Self::Lavender => (295.0, 0.11),
            Self::Mono => (0.0, 0.0),
        };
        Some(Seed { hue, chroma })
    }
}

/// The surface mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BaseMode {
    /// Dark grey surfaces (the default).
    #[default]
    Dark,
    /// True black surfaces.
    Amoled,
    /// Light surfaces.
    Light,
}

impl BaseMode {
    /// All modes in menu order.
    pub const ALL: [Self; 3] = [Self::Dark, Self::Amoled, Self::Light];

    /// Menu label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Amoled => "AMOLED black",
            Self::Light => "Light",
        }
    }

    /// Surface and text colours of the mode.
    #[must_use]
    pub const fn neutrals(self) -> Neutrals {
        match self {
            Self::Dark => Neutrals::DARK,
            Self::Amoled => Neutrals::AMOLED,
            Self::Light => Neutrals::LIGHT,
        }
    }

    /// Tone rules of the accent roles on these surfaces.
    #[must_use]
    pub const fn tones(self) -> Tones {
        match self {
            Self::Dark => Tones::DARK,
            Self::Amoled => Tones::AMOLED,
            Self::Light => Tones::LIGHT,
        }
    }

    /// `true` for light surfaces.
    #[must_use]
    pub const fn is_light(self) -> bool {
        matches!(self, Self::Light)
    }
}

/// How saturated the accent is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AccentIntensity {
    /// Muted.
    Soft,
    /// The designed strength.
    #[default]
    Standard,
    /// Saturated, still inside the contrast rules.
    Vivid,
}

impl AccentIntensity {
    /// All steps in order.
    pub const ALL: [Self; 3] = [Self::Soft, Self::Standard, Self::Vivid];

    /// Label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Soft => "Soft",
            Self::Standard => "Standard",
            Self::Vivid => "Vivid",
        }
    }

    /// Chroma factor.
    #[must_use]
    pub const fn factor(self) -> f64 {
        match self {
            Self::Soft => 0.55,
            Self::Standard => 1.0,
            Self::Vivid => 1.35,
        }
    }
}

/// How round the corners are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CornerStyle {
    /// Tight corners.
    Sharp,
    /// The Material 3 scale.
    #[default]
    Soft,
    /// Extra round.
    Round,
}

impl CornerStyle {
    /// All styles in order.
    pub const ALL: [Self; 3] = [Self::Sharp, Self::Soft, Self::Round];

    /// Label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Sharp => "Sharp",
            Self::Soft => "Soft",
            Self::Round => "Round",
        }
    }

    /// Radius scale in percent of the Material 3 scale.
    #[must_use]
    pub const fn percent(self) -> u32 {
        match self {
            Self::Sharp => 40,
            Self::Soft => 100,
            Self::Round => 140,
        }
    }
}

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

/// A cross-fade between two neutral sets (the animated theme switch), as long as the accent
/// cross-fade so both finish together.
#[derive(Debug, Clone, PartialEq)]
pub struct SchemeFade {
    from: Neutrals,
    to: Neutrals,
    elapsed_ms: f32,
}

impl SchemeFade {
    /// Duration in ms.
    #[allow(clippy::cast_precision_loss)] // 400 is exact in f32
    const DURATION_MS: f32 = ACCENT_CROSSFADE_MS as f32;

    /// A settled fade that shows `n`.
    #[must_use]
    pub fn settled(n: Neutrals) -> Self {
        Self {
            from: n,
            to: n,
            elapsed_ms: Self::DURATION_MS,
        }
    }

    /// Fade to `target` from what shows now; instant with reduced motion.
    pub fn retarget(&mut self, target: Neutrals, reduced_motion: bool) {
        if target == self.to {
            return;
        }
        self.from = self.current();
        self.to = target;
        self.elapsed_ms = if reduced_motion {
            Self::DURATION_MS
        } else {
            0.0
        };
    }

    /// Advance by `dt_ms`.
    pub fn advance(&mut self, dt_ms: f32) {
        self.elapsed_ms = (self.elapsed_ms + dt_ms.max(0.0)).min(Self::DURATION_MS);
    }

    /// `true` when the target is reached.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.elapsed_ms >= Self::DURATION_MS
    }

    /// The neutrals to draw now.
    #[must_use]
    pub fn current(&self) -> Neutrals {
        if self.is_done() {
            return self.to;
        }
        self.from.mix(
            &self.to,
            emphasized_decelerate(self.elapsed_ms / Self::DURATION_MS),
        )
    }
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

/// The cross-fade of the background field: a critically damped spring of about 0.7 s (a snap
/// with reduced motion).
#[derive(Debug, Clone)]
pub struct BgFade {
    from: [Rgba; 3],
    to: [Rgba; 3],
    spring: crate::motion::Spring,
}

impl BgFade {
    /// A settled field showing `colors`.
    #[must_use]
    pub fn settled(colors: [Rgba; 3]) -> Self {
        Self {
            from: colors,
            to: colors,
            spring: crate::motion::Spring::tuned(1.0, 150.0, 1.0),
        }
    }

    /// Fade to `target` from what shows now.
    pub fn retarget(&mut self, target: [Rgba; 3], reduced_motion: bool) {
        if target == self.to {
            return;
        }
        self.from = self.current();
        self.to = target;
        self.spring.snap(0.0);
        self.spring.target = 1.0;
        if reduced_motion {
            self.spring.snap(1.0);
        }
    }

    /// Advance by `dt_ms`.
    pub fn advance(&mut self, dt_ms: f32) {
        self.spring.step(dt_ms.clamp(0.0, 100.0) / 1000.0);
    }

    /// `true` when the target is reached.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.spring.is_settled() && (self.spring.value - 1.0).abs() < 0.004
    }

    /// Jump to the end.
    pub fn finish(&mut self) {
        self.spring.snap(1.0);
    }

    /// The colours to draw now.
    #[must_use]
    pub fn current(&self) -> [Rgba; 3] {
        let t = f64::from(self.spring.value.clamp(0.0, 1.0));
        if t >= 0.999 {
            return self.to;
        }
        [0, 1, 2].map(|i| mix_rgb(self.from[i], self.to[i], t))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::contrast_ratio;

    fn all_palettes() -> Vec<(String, BaseMode, Palette)> {
        let mut out = Vec::new();
        for base in BaseMode::ALL {
            for choice in ThemeChoice::ALL {
                for intensity in AccentIntensity::ALL {
                    // A few covers for Dynamic: saturated, dull, grey.
                    let covers = if choice == ThemeChoice::Dynamic {
                        vec![
                            None,
                            Some(Seed {
                                hue: 25.0,
                                chroma: 0.14,
                            }),
                            Some(Seed {
                                hue: 145.0,
                                chroma: 0.06,
                            }),
                            Some(Seed {
                                hue: 265.0,
                                chroma: 0.0,
                            }),
                            Some(Seed {
                                hue: 95.0,
                                chroma: 0.14,
                            }),
                        ]
                    } else {
                        vec![None]
                    };
                    for cover in covers {
                        out.push((
                            format!("{choice:?}/{base:?}/{intensity:?}/{cover:?}"),
                            base,
                            accent_palette(choice, base, intensity, cover),
                        ));
                    }
                }
            }
        }
        out
    }

    #[test]
    fn every_palette_keeps_text_contrast_on_its_surfaces() {
        for (name, base, p) in all_palettes() {
            let n = base.neutrals();
            for (pair, fg, bg) in p.text_pairs_on(&n) {
                let ratio = contrast_ratio(fg, bg);
                assert!(ratio >= 4.5, "{name}: {pair} is {ratio:.2}:1");
            }
            assert!(
                contrast_ratio(p.primary, n.surface) >= 3.0,
                "{name}: primary is not a 3:1 non-text colour"
            );
        }
    }

    #[test]
    fn neutral_text_reaches_4_5_on_every_surface_of_every_mode() {
        for base in BaseMode::ALL {
            let n = base.neutrals();
            for (name, fg, bg) in n.text_pairs() {
                let ratio = contrast_ratio(fg, bg);
                assert!(ratio >= 4.5, "{base:?}: {name} is {ratio:.2}:1");
            }
        }
    }

    #[test]
    fn text_on_accent_containers_is_legible() {
        for (name, _, p) in all_palettes() {
            assert!(
                contrast_ratio(p.on_primary, p.primary) >= 4.5,
                "{name}: on-primary"
            );
            assert!(
                contrast_ratio(p.on_secondary_container, p.secondary_container) >= 4.5,
                "{name}: on-secondary-container"
            );
        }
    }

    #[test]
    fn curated_seeds_give_distinct_accents() {
        let primaries: Vec<Rgba> = ThemeChoice::ALL
            .iter()
            .skip(1)
            .map(|c| accent_palette(*c, BaseMode::Dark, AccentIntensity::Standard, None).primary)
            .collect();
        for (i, a) in primaries.iter().enumerate() {
            for b in primaries.iter().skip(i + 1) {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn dynamic_without_a_cover_is_the_fallback() {
        assert_eq!(
            accent_palette(
                ThemeChoice::Dynamic,
                BaseMode::Dark,
                AccentIntensity::Standard,
                None
            ),
            Palette::fallback()
        );
    }

    #[test]
    fn vivid_is_more_saturated_than_soft() {
        let chroma = |i| {
            let p = accent_palette(ThemeChoice::Sunset, BaseMode::Dark, i, None);
            let (_, a, b) = crate::palette::rgb_to_oklab(p.primary_container);
            a.hypot(b)
        };
        assert!(chroma(AccentIntensity::Vivid) > chroma(AccentIntensity::Soft));
    }

    #[test]
    fn scheme_fade_runs_400_ms_between_modes() {
        let mut f = SchemeFade::settled(Neutrals::DARK);
        assert!(f.is_done());
        f.retarget(Neutrals::LIGHT, false);
        assert!(!f.is_done());
        f.advance(100.0);
        let mid = f.current();
        assert!(mid.surface != Neutrals::DARK.surface && mid.surface != Neutrals::LIGHT.surface);
        f.advance(400.0);
        assert!(f.is_done());
        assert_eq!(f.current(), Neutrals::LIGHT);
        // Reduced motion swaps at once.
        f.retarget(Neutrals::AMOLED, true);
        assert!(f.is_done() && f.current() == Neutrals::AMOLED);
    }

    #[test]
    fn background_field_keeps_text_legible_for_any_seed_and_mode() {
        let seeds: Vec<Option<Seed>> = std::iter::once(None)
            .chain((0..36).flat_map(|h| {
                [0.0, 0.06, 0.14].into_iter().map(move |c| {
                    Some(Seed {
                        hue: f64::from(h) * 10.0,
                        chroma: c,
                    })
                })
            }))
            .collect();
        for base in BaseMode::ALL {
            let n = base.neutrals();
            for seed in &seeds {
                for (k, bg) in bg_colors(*seed, base).into_iter().enumerate() {
                    // Text straight on the field, and on a pane over the field.
                    let pane = n.container.with_alpha(PANE_ALPHA).over(bg);
                    for (name, fg, under) in [
                        ("on-surface/field", n.on_surface, bg),
                        ("variant/field", n.on_surface_variant, bg),
                        ("on-surface/pane", n.on_surface, pane),
                        ("variant/pane", n.on_surface_variant, pane),
                    ] {
                        let ratio = contrast_ratio(fg, under);
                        assert!(
                            ratio >= 4.5,
                            "{base:?} {seed:?} stop {k} {name}: {ratio:.2}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn background_fade_takes_under_a_second_and_snaps_when_reduced() {
        let a = bg_colors(None, BaseMode::Dark);
        let b = bg_colors(
            Some(Seed {
                hue: 20.0,
                chroma: 0.12,
            }),
            BaseMode::Dark,
        );
        let mut f = BgFade::settled(a);
        f.retarget(b, false);
        assert!(!f.is_done());
        let mut ms = 0.0;
        while !f.is_done() && ms < 3000.0 {
            f.advance(16.0);
            ms += 16.0;
        }
        assert!((300.0..1100.0).contains(&ms), "settled after {ms} ms");
        assert_eq!(f.current(), b);
        f.retarget(a, true);
        assert!(f.is_done() && f.current() == a);
    }

    #[test]
    fn corner_scales_are_ordered() {
        assert!(CornerStyle::Sharp.percent() < CornerStyle::Soft.percent());
        assert!(CornerStyle::Soft.percent() < CornerStyle::Round.percent());
        assert_eq!(CornerStyle::Soft.percent(), 100);
    }
}
