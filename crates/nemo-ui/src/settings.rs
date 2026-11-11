//! User settings the UI shows and changes. Plain data: the app loads and stores them, the UI only
//! reports changes through the `on_settings` callback of `UiOptions`.

use crate::scheme::{AccentIntensity, BaseMode, CornerStyle, ThemeChoice};

/// How much glass the floating bar draws (`design.md` section 7). The live-blur tiers need the
/// owner's GPU and are not built yet, so only the two cheapest tiers exist here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlassTier {
    /// Pre-blurred cover texture, tint and edge (the shipped glass).
    #[default]
    Low,
    /// Tonal surface, no glass at all.
    Off,
}

impl GlassTier {
    /// Label for the settings control.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Off => "Off",
        }
    }
}

/// Settings the UI owns a view of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiSettings {
    /// Skip non-music parts of music videos (SponsorBlock).
    pub sponsorblock: bool,
    /// Never request or play ads.
    pub block_ads: bool,
    /// Volume in percent (0 to 100).
    pub volume: u8,
    /// Glass tier of the player bar.
    pub glass_tier: GlassTier,
    /// Reduce motion: no cross-fades, no lyric glide.
    pub reduce_motion: bool,
    /// Reduce transparency: all glass becomes opaque.
    pub reduce_transparency: bool,
    /// The first-run notice about YouTube's Terms of Service was accepted.
    pub tos_acknowledged: bool,
    /// Accent source: the cover or a curated seed.
    pub theme: ThemeChoice,
    /// Surface mode: dark, AMOLED black or light.
    pub base: BaseMode,
    /// Accent strength.
    pub accent_intensity: AccentIntensity,
    /// Corner roundness.
    pub corner_style: CornerStyle,
}

impl Default for UiSettings {
    /// Both switches start off (README); the notice is not accepted yet.
    fn default() -> Self {
        Self {
            sponsorblock: false,
            block_ads: false,
            volume: 100,
            glass_tier: GlassTier::Low,
            reduce_motion: false,
            reduce_transparency: false,
            tos_acknowledged: false,
            theme: ThemeChoice::Dynamic,
            base: BaseMode::Dark,
            accent_intensity: AccentIntensity::Standard,
            corner_style: CornerStyle::Soft,
        }
    }
}

/// One change made on the Settings screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingChange {
    /// SponsorBlock switch.
    SponsorBlock(bool),
    /// Ad blocking switch.
    BlockAds(bool),
    /// Volume in percent.
    Volume(u8),
    /// Glass tier.
    GlassTier(GlassTier),
    /// Reduce motion switch.
    ReduceMotion(bool),
    /// Reduce transparency switch.
    ReduceTransparency(bool),
    /// Accent source.
    Theme(ThemeChoice),
    /// Surface mode.
    Base(BaseMode),
    /// Accent strength.
    AccentIntensity(AccentIntensity),
    /// Corner roundness.
    CornerStyle(CornerStyle),
}

impl UiSettings {
    /// Apply one change.
    pub fn apply(&mut self, change: SettingChange) {
        match change {
            SettingChange::SponsorBlock(v) => self.sponsorblock = v,
            SettingChange::BlockAds(v) => self.block_ads = v,
            SettingChange::Volume(v) => self.volume = v.min(100),
            SettingChange::GlassTier(v) => self.glass_tier = v,
            SettingChange::ReduceMotion(v) => self.reduce_motion = v,
            SettingChange::ReduceTransparency(v) => self.reduce_transparency = v,
            SettingChange::Theme(v) => self.theme = v,
            SettingChange::Base(v) => self.base = v,
            SettingChange::AccentIntensity(v) => self.accent_intensity = v,
            SettingChange::CornerStyle(v) => self.corner_style = v,
        }
    }

    /// `true` when the player bar must use the opaque tonal surface instead of glass.
    #[must_use]
    pub fn opaque_chrome(&self) -> bool {
        // The glass tint maths is tuned for dark surfaces, so light mode uses the tonal bar.
        self.reduce_transparency || self.glass_tier == GlassTier::Off || self.base.is_light()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_calm_and_unacknowledged() {
        let s = UiSettings::default();
        assert!(!s.sponsorblock && !s.block_ads && !s.tos_acknowledged);
        assert_eq!(s.volume, 100);
        assert!(!s.opaque_chrome());
    }

    #[test]
    fn theme_settings_default_to_the_original_look_and_apply() {
        let mut s = UiSettings::default();
        assert_eq!(s.theme, ThemeChoice::Dynamic);
        assert_eq!(s.base, BaseMode::Dark);
        assert_eq!(s.accent_intensity, AccentIntensity::Standard);
        assert_eq!(s.corner_style, CornerStyle::Soft);
        s.apply(SettingChange::Theme(ThemeChoice::Sakura));
        s.apply(SettingChange::Base(BaseMode::Light));
        s.apply(SettingChange::AccentIntensity(AccentIntensity::Vivid));
        s.apply(SettingChange::CornerStyle(CornerStyle::Round));
        assert_eq!(s.theme, ThemeChoice::Sakura);
        assert!(s.opaque_chrome(), "light mode uses the tonal bar");
        assert_eq!(s.corner_style, CornerStyle::Round);
    }

    #[test]
    fn changes_apply_and_volume_is_clamped() {
        let mut s = UiSettings::default();
        s.apply(SettingChange::SponsorBlock(true));
        s.apply(SettingChange::Volume(250));
        s.apply(SettingChange::ReduceTransparency(true));
        assert!(s.sponsorblock && s.volume == 100 && s.opaque_chrome());
        s.apply(SettingChange::ReduceTransparency(false));
        s.apply(SettingChange::GlassTier(GlassTier::Off));
        assert!(s.opaque_chrome());
    }
}
