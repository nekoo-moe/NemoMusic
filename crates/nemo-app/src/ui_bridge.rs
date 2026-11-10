//! Translate between the stored settings and the UI's view of them, and save changes off the UI
//! thread.

use std::path::PathBuf;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};

use nemo_ui::{GlassTier as UiGlass, UiOptions, UiSettings};

use crate::config::{GlassTier, Settings};

/// The UI view of the stored settings.
pub fn to_ui(s: &Settings) -> UiSettings {
    UiSettings {
        sponsorblock: s.sponsorblock,
        block_ads: s.block_ads,
        volume: s.volume_percent,
        glass_tier: match s.glass_tier {
            GlassTier::Off => UiGlass::Off,
            GlassTier::High | GlassTier::Medium | GlassTier::Low => UiGlass::Low,
        },
        reduce_motion: s.reduce_motion,
        reduce_transparency: s.reduce_transparency,
        tos_acknowledged: s.tos_acknowledged,
    }
}

/// Fold a UI change into the stored settings. Fields the UI does not own stay as they are.
pub fn apply_ui(s: &mut Settings, ui: &UiSettings) {
    s.sponsorblock = ui.sponsorblock;
    s.block_ads = ui.block_ads;
    s.volume_percent = ui.volume.min(100);
    s.glass_tier = match ui.glass_tier {
        UiGlass::Off => GlassTier::Off,
        UiGlass::Low => GlassTier::Low,
    };
    s.reduce_motion = ui.reduce_motion;
    s.reduce_transparency = ui.reduce_transparency;
    s.tos_acknowledged = ui.tos_acknowledged;
}

/// Start the saver thread and build the UI options. The callback only sends on a channel, so
/// it never blocks the UI thread.
pub fn options(settings: Settings, root: PathBuf) -> UiOptions {
    let initial = to_ui(&settings);
    let folders = settings
        .music_folders
        .iter()
        .map(|p| p.display().to_string())
        .collect();
    let tx = spawn_saver(settings, root);
    let tx = Mutex::new(tx);
    let on_settings: Arc<dyn Fn(UiSettings) + Send + Sync> = Arc::new(move |ui| {
        if let Ok(tx) = tx.lock() {
            // The saver only stops when the app exits; a failed send then is harmless.
            let _ = tx.send(ui);
        }
    });
    UiOptions {
        settings: initial,
        on_settings: Some(on_settings),
        music_folders: folders,
        // No folder picker yet: folders are edited in settings.json (see README).
        on_add_music_folder: None,
        ..UiOptions::default()
    }
}

fn spawn_saver(mut settings: Settings, root: PathBuf) -> Sender<UiSettings> {
    let (tx, rx) = channel::<UiSettings>();
    let spawned = std::thread::Builder::new()
        .name("settings-save".into())
        .spawn(move || {
            while let Ok(mut ui) = rx.recv() {
                // Keep only the newest change when the user drags a slider.
                while let Ok(newer) = rx.try_recv() {
                    ui = newer;
                }
                apply_ui(&mut settings, &ui);
                if let Err(err) = settings.save(&root) {
                    tracing::warn!(error = %err, "cannot save settings");
                }
            }
        });
    if let Err(err) = spawned {
        tracing::warn!(error = %err, "settings will not be saved this session");
    }
    tx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_ui_fields_and_leaves_the_rest() {
        let mut s = Settings {
            music_folders: vec![PathBuf::from("C:/Music")],
            output_device: Some("DAC".into()),
            ..Settings::default()
        };
        let ui = UiSettings {
            sponsorblock: true,
            volume: 250,
            glass_tier: UiGlass::Off,
            tos_acknowledged: true,
            ..to_ui(&s)
        };
        apply_ui(&mut s, &ui);
        assert!(s.sponsorblock && s.tos_acknowledged);
        assert_eq!(s.volume_percent, 100);
        assert_eq!(s.glass_tier, GlassTier::Off);
        assert_eq!(s.output_device.as_deref(), Some("DAC"));
        assert_eq!(s.music_folders, [PathBuf::from("C:/Music")]);
        assert_eq!(to_ui(&s).glass_tier, UiGlass::Off);
    }

    #[test]
    fn higher_stored_tiers_show_as_low_in_the_ui() {
        let s = Settings {
            glass_tier: GlassTier::High,
            ..Settings::default()
        };
        assert_eq!(to_ui(&s).glass_tier, UiGlass::Low);
    }
}
