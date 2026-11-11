//! User settings, stored as JSON in the data folder. Never holds secrets (`coc.md`).
//!
//! Defaults follow the owner decisions: ad blocking and SponsorBlock are off until the user
//! turns them on (no pre-checked consent), the glass tier starts at Low (Intel HD 530 class GPUs).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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
    /// Glass effect level.
    pub glass_tier: GlassTier,
    /// Reduce transparency regardless of the OS setting.
    pub reduce_transparency: bool,
    /// Reduce motion regardless of the OS setting.
    pub reduce_motion: bool,
    /// Preferred output device name; `None` uses the system default.
    pub output_device: Option<String>,
    /// Path to `yt-dlp`; `None` searches the usual places.
    pub yt_dlp_path: Option<PathBuf>,
    /// Theme accent source, stored as its menu label (empty = default).
    pub theme: String,
    /// Surface mode label (Dark, AMOLED, Light; empty = default).
    pub base_mode: String,
    /// Accent strength label (empty = default).
    pub accent_intensity: String,
    /// Corner style label (empty = default).
    pub corner_style: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            music_folders: default_music_folder().into_iter().collect(),
            volume_percent: 100,
            sponsorblock: false,
            block_ads: false,
            tos_acknowledged: false,
            glass_tier: GlassTier::Low,
            reduce_transparency: false,
            reduce_motion: false,
            output_device: None,
            yt_dlp_path: None,
            theme: String::new(),
            base_mode: String::new(),
            accent_intensity: String::new(),
            corner_style: String::new(),
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
    fn defaults_have_no_pre_checked_consent() {
        let s = Settings::default();
        assert!(!s.sponsorblock && !s.block_ads && !s.tos_acknowledged);
        assert_eq!(s.volume_percent, 100);
        assert_eq!(s.glass_tier, GlassTier::Low);
    }

    #[test]
    fn save_then_load_round_trips() {
        let root = temp_root("roundtrip");
        let s = Settings {
            volume_percent: 40,
            sponsorblock: true,
            glass_tier: GlassTier::Off,
            ..Settings::default()
        };
        s.save(&root).expect("save");
        assert_eq!(Settings::load(&root), s);
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
        assert!(!s.block_ads);
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
