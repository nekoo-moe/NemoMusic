//! User settings the UI shows and changes. Plain data: the app loads and stores them, the UI only
//! reports changes through the `on_settings` callback of `UiOptions`.

use std::collections::BTreeMap;

use crate::collection_sort::CollectionSort;
use crate::scheme::{AccentIntensity, BaseMode, CornerStyle, ThemeChoice};
use crate::window_glass::{OPACITY_DEFAULT, WindowGlass, clamp_opacity};
use crate::workspace::Source;

/// The old glass tier of the player bar. The bar is a Material surface now (owner, 2026-11-19;
/// bar-material), so both values draw the same thing. The enum and its config key stay so old
/// config files load.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlassTier {
    /// Written by older versions as the default. Draws the Material surface.
    #[default]
    Low,
    /// Draws the Material surface.
    Off,
}

/// Which look draws the Now Playing pane and full-screen view (now-playing-styles spec). Only the
/// arrangement changes; playback, the badge and the signal path are the same in every style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum NowPlayingStyle {
    /// A calm column: still cover, titles, seek bar, controls; lyrics on a tab.
    Classic,
    /// Today's Material 3 Expressive look. Default, so existing users see no change.
    #[default]
    M3Expressive,
    /// Large lyrics that fill word by word over the cover.
    AppleMusic,
}

impl NowPlayingStyle {
    /// All styles in settings order.
    pub const ALL: [Self; 3] = [Self::Classic, Self::M3Expressive, Self::AppleMusic];

    /// Label for the settings control (requirements, criterion 1).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Classic => "Classic",
            Self::M3Expressive => "Material 3 Expressive",
            Self::AppleMusic => "Apple Music",
        }
    }
}

/// User interface language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Language {
    /// English.
    #[default]
    English,
    /// Vietnamese (Tiếng Việt).
    Vietnamese,
}

impl Language {
    /// All available languages.
    pub const ALL: [Self; 2] = [Self::English, Self::Vietnamese];

    /// Display label for UI choice controls.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Vietnamese => "Tiếng Việt",
        }
    }

    /// True if Vietnamese.
    #[must_use]
    pub const fn is_vietnamese(self) -> bool {
        matches!(self, Self::Vietnamese)
    }
}

/// Animated-cover sources the user turned on. Every switch is off by default; nothing is
/// requested while a switch is off.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AnimatedCoverSettings {
    /// Show Spotify Canvas clips in place of the still cover.
    pub spotify_canvas: bool,
}

/// The stored answer of one opt-in row: the switch and the version of its notice the user
/// confirmed (ADR 0004, "Opt-in settings pattern"). Off and never confirmed by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OptInState {
    /// The switch.
    pub enabled: bool,
    /// The `OptInInfo.version` the user confirmed in the panel; 0 when never confirmed.
    pub confirmed_version: u32,
}

impl OptInState {
    /// `true` when the switch is on and the notice of `version` was confirmed: the only state in
    /// which the app may build the service.
    #[must_use]
    pub const fn is_open(self, version: u32) -> bool {
        self.enabled && self.confirmed_version == version
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
    /// Old glass tier of the player bar: kept for old config files, no effect on the look.
    pub glass_tier: GlassTier,
    /// Reduce motion: no cross-fades, no lyric glide.
    pub reduce_motion: bool,
    /// Reduce transparency: all glass becomes opaque.
    pub reduce_transparency: bool,
    /// The first-run notice about YouTube's Terms of Service was accepted.
    pub tos_acknowledged: bool,
    /// The first-run setup flow was finished or skipped (startup-flow spec).
    pub setup_completed: bool,
    /// Accent source: the cover or a curated seed.
    pub theme: ThemeChoice,
    /// Surface mode: dark, AMOLED black or light.
    pub base: BaseMode,
    /// Accent strength.
    pub accent_intensity: AccentIntensity,
    /// Corner roundness.
    pub corner_style: CornerStyle,
    /// Look of the Now Playing surfaces.
    pub now_playing_style: NowPlayingStyle,
    /// Animated-cover switches.
    pub animated_covers: AnimatedCoverSettings,
    /// The whole window takes its colours from the cover of the song that plays (owner,
    /// 2026-11-18; song-colour-theme spec). On by default.
    pub song_colour_theme: bool,
    /// A slow, soft pulse in the background while a song plays. On by default; it needs
    /// [`Self::song_colour_theme`] and stops under reduced motion.
    pub song_heartbeat: bool,
    /// The window itself is see-through (owner, 2026-11-19; appearance-glass spec). Off by
    /// default. The window is created once, so a change from off to on shows after a restart.
    pub window_glass: bool,
    /// Opacity of the window backdrop with [`Self::window_glass`] on, in percent (40 to 95).
    pub window_glass_opacity: u8,
    /// The order the user picked per album or playlist page, by [`crate::collection_sort`] page
    /// key (`album:<id>`, `playlist:<id>`). Pages in their default order have no entry.
    pub collection_sorts: BTreeMap<String, CollectionSort>,
    /// The SoundCloud opt-in row (owner, 2026-11-19; soundcloud-account). Off by default.
    pub soundcloud: OptInState,
    /// The listening area the window follows (owner, 2026-11-19; source-workspaces spec).
    /// YouTube Music by default; the app falls back to it when SoundCloud is off or signed out.
    pub listening_area: Source,
    /// UI display language (English or Tiếng Việt).
    pub language: Language,
}

impl Default for UiSettings {
    /// Both switches start on (owner, 2026-11-15; README); the notice is not accepted yet.
    fn default() -> Self {
        Self {
            sponsorblock: true,
            block_ads: true,
            volume: 100,
            glass_tier: GlassTier::Low,
            reduce_motion: false,
            reduce_transparency: false,
            tos_acknowledged: false,
            setup_completed: false,
            theme: ThemeChoice::Dynamic,
            base: BaseMode::Dark,
            accent_intensity: AccentIntensity::Standard,
            corner_style: CornerStyle::Soft,
            now_playing_style: NowPlayingStyle::M3Expressive,
            animated_covers: AnimatedCoverSettings::default(),
            song_colour_theme: true,
            song_heartbeat: true,
            window_glass: false,
            window_glass_opacity: OPACITY_DEFAULT,
            collection_sorts: BTreeMap::new(),
            soundcloud: OptInState::default(),
            listening_area: Source::default(),
            language: Language::default(),
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
    /// Look of the Now Playing surfaces.
    NowPlayingStyle(NowPlayingStyle),
    /// Match the window colours to the playing song.
    SongColourTheme(bool),
    /// Gentle breathing of the background (stored as the heartbeat).
    SongHeartbeat(bool),
    /// See-through window switch.
    WindowGlass(bool),
    /// Opacity of the see-through window in percent (limited to 40 to 95).
    WindowGlassOpacity(u8),
    /// The SoundCloud opt-in row: the switch and the confirmed notice version.
    SoundCloud(OptInState),
    /// The listening area: YouTube Music or SoundCloud.
    ListeningArea(Source),
    /// Display language.
    Language(Language),
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
            SettingChange::NowPlayingStyle(v) => self.now_playing_style = v,
            SettingChange::SongColourTheme(v) => self.song_colour_theme = v,
            SettingChange::SongHeartbeat(v) => self.song_heartbeat = v,
            SettingChange::WindowGlass(v) => self.window_glass = v,
            SettingChange::WindowGlassOpacity(v) => self.window_glass_opacity = clamp_opacity(v),
            SettingChange::SoundCloud(v) => self.soundcloud = v,
            SettingChange::ListeningArea(v) => self.listening_area = v,
            SettingChange::Language(v) => self.language = v,
        }
    }

    /// `true` when the first-run setup flow should open at start. A user who already accepted
    /// the notice is never sent through it (startup-flow, criterion 8).
    #[must_use]
    pub fn needs_setup(&self) -> bool {
        !self.setup_completed && !self.tos_acknowledged
    }

    /// `true` when the heartbeat switch can be used: song colours are on and motion is not
    /// reduced (`reduced_motion` is the effective value, the setting or the OS preference).
    #[must_use]
    pub const fn heartbeat_available(&self, reduced_motion: bool) -> bool {
        self.song_colour_theme && !reduced_motion
    }

    /// The window glass inputs. `created_transparent` is whether the running window was created
    /// see-through (the switch was on when the app started).
    #[must_use]
    pub fn window_glass(&self, created_transparent: bool) -> WindowGlass {
        WindowGlass {
            enabled: self.window_glass,
            opacity: self.window_glass_opacity,
            reduce_transparency: self.reduce_transparency,
            created_transparent,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_calm_and_unacknowledged() {
        let s = UiSettings::default();
        assert!(s.sponsorblock && s.block_ads && !s.tos_acknowledged);
        assert_eq!(s.volume, 100);
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
        assert_eq!(s.corner_style, CornerStyle::Round);
    }

    #[test]
    fn now_playing_defaults_to_the_m3_style_with_no_animated_cover() {
        let s = UiSettings::default();
        assert_eq!(s.now_playing_style, NowPlayingStyle::M3Expressive);
        assert!(!s.animated_covers.spotify_canvas);
        let labels = NowPlayingStyle::ALL.map(NowPlayingStyle::label);
        assert_eq!(labels, ["Classic", "Material 3 Expressive", "Apple Music"]);
    }

    #[test]
    fn the_now_playing_style_changes_through_a_setting_change() {
        let mut s = UiSettings::default();
        s.apply(SettingChange::NowPlayingStyle(NowPlayingStyle::Classic));
        assert_eq!(s.now_playing_style, NowPlayingStyle::Classic);
    }

    #[test]
    fn song_colours_and_heartbeat_start_on_and_the_heartbeat_needs_both() {
        let mut s = UiSettings::default();
        assert!(s.song_colour_theme && s.song_heartbeat);
        assert!(s.heartbeat_available(false));
        assert!(!s.heartbeat_available(true), "reduced motion greys it out");
        s.apply(SettingChange::SongColourTheme(false));
        assert!(!s.song_colour_theme && !s.heartbeat_available(false));
        s.apply(SettingChange::SongHeartbeat(false));
        assert!(!s.song_heartbeat);
    }

    #[test]
    fn window_glass_starts_off_at_80_and_the_opacity_is_clamped() {
        let mut s = UiSettings::default();
        assert!(!s.window_glass);
        assert_eq!(s.window_glass_opacity, 80);
        s.apply(SettingChange::WindowGlass(true));
        s.apply(SettingChange::WindowGlassOpacity(10));
        assert!(s.window_glass && s.window_glass_opacity == 40);
        s.apply(SettingChange::WindowGlassOpacity(99));
        assert_eq!(s.window_glass_opacity, 95);
        s.apply(SettingChange::WindowGlassOpacity(65));
        assert_eq!(s.window_glass_opacity, 65);
        assert!(s.window_glass(true).is_clear());
        assert!(s.window_glass(false).needs_restart());
        s.apply(SettingChange::ReduceTransparency(true));
        assert!(!s.window_glass(true).is_clear(), "reduce transparency wins");
    }

    #[test]
    fn soundcloud_starts_off_and_is_open_only_with_the_confirmed_version() {
        let mut s = UiSettings::default();
        assert_eq!(s.soundcloud, OptInState::default());
        assert!(!s.soundcloud.is_open(2));
        s.apply(SettingChange::SoundCloud(OptInState {
            enabled: true,
            confirmed_version: 1,
        }));
        assert!(!s.soundcloud.is_open(2), "an old notice is not enough");
        s.apply(SettingChange::SoundCloud(OptInState {
            enabled: true,
            confirmed_version: 2,
        }));
        assert!(s.soundcloud.is_open(2));
        s.apply(SettingChange::SoundCloud(OptInState {
            enabled: false,
            confirmed_version: 2,
        }));
        assert!(!s.soundcloud.is_open(2), "off is off");
    }

    #[test]
    fn the_listening_area_starts_on_youtube_music_and_changes_through_a_setting() {
        let mut s = UiSettings::default();
        assert_eq!(s.listening_area, Source::YouTube);
        s.apply(SettingChange::ListeningArea(Source::SoundCloud));
        assert_eq!(s.listening_area, Source::SoundCloud);
        s.apply(SettingChange::ListeningArea(Source::YouTube));
        assert_eq!(s.listening_area, Source::YouTube);
    }

    #[test]
    fn changes_apply_and_volume_is_clamped() {
        let mut s = UiSettings::default();
        s.apply(SettingChange::SponsorBlock(true));
        s.apply(SettingChange::Volume(250));
        s.apply(SettingChange::ReduceTransparency(true));
        assert!(s.sponsorblock && s.volume == 100 && s.reduce_transparency);
        s.apply(SettingChange::GlassTier(GlassTier::Off));
        assert_eq!(s.glass_tier, GlassTier::Off, "old config values still load");
    }
}
