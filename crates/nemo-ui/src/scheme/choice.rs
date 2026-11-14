//! The theme choices: accent source, surface mode, accent intensity and corner style.

use crate::palette::{Seed, Tones};
use crate::theme::Neutrals;

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
