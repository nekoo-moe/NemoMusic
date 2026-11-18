//! User settings, stored as JSON in the data folder. Never holds secrets (`coc.md`).
//!
//! Defaults follow the owner decisions: ad blocking and SponsorBlock are off until the user
//! turns them on (no pre-checked consent), the glass tier starts at Low (Intel HD 530 class GPUs).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub mod effects;
pub mod output;

pub use effects::{StoredEffects, StoredTransition};
pub use output::{StoredDevice, StoredOutputOptions};

/// File name inside the data root.
pub const CONFIG_FILE: &str = "settings.json";

/// Largest settings file read, in bytes. A bigger file is treated as broken.
pub const MAX_CONFIG_BYTES: u64 = 256 * 1024;

/// How much glass effect the UI draws (`design.md` section 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GlassTier {
    /// Live backdrop blur, lens and specular.
    High,
    /// Half-resolution blur, no lens.
    Medium,
    /// Pre-blurred cover texture. Default.
    #[default]
    Low,
    /// Opaque tonal surfaces.
    Off,
}

/// Every user setting. Unknown fields in the file are ignored; missing ones take defaults.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent user switches, stored as JSON"
)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Folders scanned for local music.
    pub music_folders: Vec<PathBuf>,
    /// Volume in percent (0 to 100).
    pub volume_percent: u8,
    /// Skip non-music segments with SponsorBlock.
    pub sponsorblock: bool,
    /// Never request or play ads.
    pub block_ads: bool,
    /// The user has seen and acknowledged the Terms of Service notice.
    pub tos_acknowledged: bool,
    /// The first-run setup flow was finished or skipped. A file without it loads as `false`;
    /// a user who acknowledged the notice is never sent through the flow (see
    /// `UiSettings::needs_setup`).
    pub setup_completed: bool,
    /// Glass effect level.
    pub glass_tier: GlassTier,
    /// Reduce transparency regardless of the OS setting.
    pub reduce_transparency: bool,
    /// Reduce motion regardless of the OS setting.
    pub reduce_motion: bool,
    /// The chosen output device by ID and name; `None` uses the system default. An older
    /// file's plain name loads with an empty ID until it is matched (output-devices).
    pub output_device: Option<StoredDevice>,
    /// Exclusive mode, idle release and the exclusive buffer (output-devices).
    pub output_options: StoredOutputOptions,
    /// Path to `yt-dlp`; `None` searches the usual places.
    pub yt_dlp_path: Option<PathBuf>,
    /// Install and update the app's own `yt-dlp` copy when none is found (on by default,
    /// owner decision 2026-11-15). Never touches a `yt-dlp` the user installed.
    pub yt_dlp_auto_install: bool,
    /// Theme accent source, stored as its menu label (empty = default).
    pub theme: String,
    /// Surface mode label (Dark, AMOLED, Light; empty = default).
    pub base_mode: String,
    /// Accent strength label (empty = default).
    pub accent_intensity: String,
    /// Corner style label (empty = default).
    pub corner_style: String,
    /// Now Playing style label (empty or unknown = Material 3 Expressive).
    pub now_playing_style: String,
    /// Animated-cover switches, all off by default.
    pub animated_covers: AnimatedCovers,
    /// The window takes its colours from the playing song (on by default; song-colour-theme).
    pub song_colour_theme: bool,
    /// A gentle heartbeat of the background (on by default; song-colour-theme).
    pub song_heartbeat: bool,
    /// The opt-in rows, by `OptInInfo.id`. A missing id means the user never touched the row,
    /// so the feature's default holds (see `opt_in_gate`).
    pub opt_ins: BTreeMap<String, OptInChoice>,
    /// Equalizer, delay and reverb; every effect off when missing (audio-effects).
    pub effects: StoredEffects,
    /// Crossfade and smart transitions; off when missing (audio-effects).
    pub transition: StoredTransition,
    /// The order picked on album and playlist pages: page key (`album:<id>`, `playlist:<id>`)
    /// to the order's label; a page in its default order has no entry (collection-sort-meta).
    pub collection_sorts: BTreeMap<String, String>,
}

/// Stored animated-cover switches. A missing field is off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AnimatedCovers {
    /// Spotify Canvas clips in place of the still cover.
    pub spotify_canvas: bool,
}

/// The stored answer of one opt-in row (ADR 0004, "Opt-in settings pattern").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct OptInChoice {
    /// The switch.
    pub enabled: bool,
    /// The `OptInInfo.version` the user confirmed in the panel; 0 when never confirmed.
    pub confirmed_version: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            music_folders: default_music_folder().into_iter().collect(),
            volume_percent: 100,
            // On by default (owner decision, 2026-11-15); each has its own switch.
            sponsorblock: true,
            block_ads: true,
            tos_acknowledged: false,
            setup_completed: false,
            glass_tier: GlassTier::Low,
            reduce_transparency: false,
            reduce_motion: false,
            output_device: None,
            output_options: StoredOutputOptions::default(),
            yt_dlp_path: None,
            yt_dlp_auto_install: true,
            theme: String::new(),
            base_mode: String::new(),
            accent_intensity: String::new(),
            corner_style: String::new(),
            now_playing_style: String::new(),
            animated_covers: AnimatedCovers::default(),
            song_colour_theme: true,
            song_heartbeat: true,
            opt_ins: BTreeMap::new(),
            effects: StoredEffects::default(),
            transition: StoredTransition::default(),
            collection_sorts: BTreeMap::new(),
        }
    }
}

impl Settings {
    /// Read settings from `root`. A missing, oversized or broken file gives defaults; a broken
    /// file is kept as `settings.broken.json` so nothing the user wrote is lost.
    #[must_use]
    pub fn load(root: &Path) -> Self {
        let path = root.join(CONFIG_FILE);
        let Ok(meta) = std::fs::metadata(&path) else {
            return Self::default();
        };
        let parsed = (meta.len() <= MAX_CONFIG_BYTES)
            .then(|| std::fs::read_to_string(&path).ok())
            .flatten()
            .and_then(|text| serde_json::from_str::<Self>(&text).ok());
        if let Some(settings) = parsed {
            settings.sanitized()
        } else {
            // Keeping the broken file is best effort; defaults still load if this fails.
            let _ = std::fs::rename(&path, root.join("settings.broken.json"));
            Self::default()
        }
    }

    /// Write settings to `root` atomically (temp file, then rename).
    ///
    /// # Errors
    /// Returns the I/O error when the file cannot be written.
    pub fn save(&self, root: &Path) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = root.join("settings.json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(tmp, root.join(CONFIG_FILE))
    }

    /// Clamp values that a hand-edited file could break.
    #[must_use]
    pub fn sanitized(mut self) -> Self {
        self.volume_percent = self.volume_percent.min(100);
        self.music_folders.retain(|p| p.is_absolute());
        self.music_folders.dedup();
        self.effects = self.effects.sanitized();
        self.transition = self.transition.sanitized();
        self.output_options = self.output_options.sanitized();
        self
    }
}

/// `%USERPROFILE%\Music` (or `$HOME/Music`) when it exists.
fn default_music_folder() -> Option<PathBuf> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?;
    let music = PathBuf::from(home).join("Music");
    music.is_dir().then_some(music)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nemo-cfg-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir); // A leftover from an earlier run is fine to drop.
        std::fs::create_dir_all(&dir).expect("dir");
        dir
    }

    #[test]
    fn defaults_block_ads_and_skip_sponsors_but_leave_the_notice_unread() {
        let s = Settings::default();
        assert!(s.sponsorblock && s.block_ads && !s.tos_acknowledged);
        assert_eq!(s.volume_percent, 100);
        assert_eq!(s.glass_tier, GlassTier::Low);
    }

    #[test]
    fn save_then_load_round_trips() {
        let root = temp_root("roundtrip");
        let s = Settings {
            volume_percent: 40,
            sponsorblock: false,
            glass_tier: GlassTier::Off,
            ..Settings::default()
        };
        s.save(&root).expect("save");
        assert_eq!(Settings::load(&root), s);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn setup_completed_is_additive_and_round_trips() {
        let root = temp_root("setup-completed");
        // A file from before the setup flow: loads, setup not completed, nothing else lost.
        std::fs::write(
            root.join(CONFIG_FILE),
            r#"{"volume_percent": 40, "tos_acknowledged": true}"#,
        )
        .expect("write");
        let old = Settings::load(&root);
        assert!(!old.setup_completed && old.tos_acknowledged);
        assert_eq!(old.volume_percent, 40);
        let s = Settings {
            setup_completed: true,
            ..old
        };
        s.save(&root).expect("save");
        assert!(Settings::load(&root).setup_completed);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn now_playing_settings_round_trip_and_an_old_file_has_none() {
        let root = temp_root("now-playing");
        let s = Settings {
            now_playing_style: "Apple Music".into(),
            animated_covers: AnimatedCovers {
                spotify_canvas: true,
            },
            ..Settings::default()
        };
        s.save(&root).expect("save");
        assert_eq!(Settings::load(&root), s);
        // A file written before the style existed.
        std::fs::write(root.join(CONFIG_FILE), r#"{"volume_percent": 40}"#).expect("write");
        let old = Settings::load(&root);
        assert_eq!(old.now_playing_style, "");
        assert!(!old.animated_covers.spotify_canvas);
        // Values of the wrong type: the broken file is kept and defaults load.
        std::fs::write(
            root.join(CONFIG_FILE),
            r#"{"now_playing_style": 5, "animated_covers": {"spotify_canvas": "yes"}}"#,
        )
        .expect("write");
        assert_eq!(Settings::load(&root), Settings::default());
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn song_colour_switches_are_on_by_default_and_round_trip() {
        let root = temp_root("song-colour");
        // A file written before the switches existed loads with both on.
        std::fs::write(root.join(CONFIG_FILE), r#"{"volume_percent": 40}"#).expect("write");
        let old = Settings::load(&root);
        assert!(old.song_colour_theme && old.song_heartbeat);
        assert!(Settings::default().song_colour_theme && Settings::default().song_heartbeat);
        let off = Settings {
            song_colour_theme: false,
            song_heartbeat: false,
            ..Settings::default()
        };
        off.save(&root).expect("save");
        assert_eq!(Settings::load(&root), off);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn opt_in_choices_round_trip_and_an_old_file_has_none() {
        let root = temp_root("opt-ins");
        let s = Settings {
            opt_ins: BTreeMap::from([(
                "lastfm".to_owned(),
                OptInChoice {
                    enabled: true,
                    confirmed_version: 2,
                },
            )]),
            ..Settings::default()
        };
        s.save(&root).expect("save");
        assert_eq!(Settings::load(&root), s);
        std::fs::write(root.join(CONFIG_FILE), r#"{"volume_percent": 40}"#).expect("write");
        assert!(Settings::load(&root).opt_ins.is_empty());
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn yt_dlp_auto_install_is_on_by_default_and_round_trips() {
        assert!(Settings::default().yt_dlp_auto_install);
        let root = temp_root("ytdlp-auto");
        // A file written before the switch existed: on.
        std::fs::write(root.join(CONFIG_FILE), r#"{"volume_percent": 40}"#).expect("write");
        assert!(Settings::load(&root).yt_dlp_auto_install);
        let off = Settings {
            yt_dlp_auto_install: false,
            ..Settings::default()
        };
        off.save(&root).expect("save");
        assert!(!Settings::load(&root).yt_dlp_auto_install);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn broken_file_gives_defaults_and_is_kept() {
        let root = temp_root("broken");
        std::fs::write(root.join(CONFIG_FILE), "{ not json").expect("write");
        assert_eq!(Settings::load(&root), Settings::default());
        assert!(root.join("settings.broken.json").is_file());
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn missing_fields_take_defaults_and_bad_values_are_clamped() {
        let root = temp_root("partial");
        std::fs::write(
            root.join(CONFIG_FILE),
            r#"{"volume_percent": 250, "music_folders": ["relative/path"], "extra": 1}"#,
        )
        .expect("write");
        let s = Settings::load(&root);
        assert_eq!(s.volume_percent, 100);
        assert_eq!(s.music_folders, Vec::<PathBuf>::new());
        assert!(s.block_ads && s.sponsorblock);
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
