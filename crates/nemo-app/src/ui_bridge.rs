//! Translate between the stored settings and the UI's view of them, and save changes off the UI
//! thread.

use std::path::PathBuf;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};

use nemo_core::output::OutputStatus;
use nemo_core::player::PlayerEvent;
use nemo_core::services::PlayerControl;
use nemo_ui::collection_sort::{CollectionSort, MAX_REMEMBERED};
use nemo_ui::scheme::{AccentIntensity, BaseMode, CornerStyle, ThemeChoice};
use nemo_ui::{
    AnimatedCoverSettings, GlassTier as UiGlass, NowPlayingStyle, UiOptions, UiSettings,
};

use crate::config::{GlassTier, Settings, StoredDevice};

/// Find the menu entry whose label matches; unknown or empty text gives the default.
fn by_label<T: Copy + Default>(all: &[T], label: fn(T) -> &'static str, stored: &str) -> T {
    all.iter()
        .copied()
        .find(|v| label(*v) == stored)
        .unwrap_or_default()
}

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
        theme: by_label(&ThemeChoice::ALL, ThemeChoice::label, &s.theme),
        base: by_label(&BaseMode::ALL, BaseMode::label, &s.base_mode),
        accent_intensity: by_label(
            &AccentIntensity::ALL,
            AccentIntensity::label,
            &s.accent_intensity,
        ),
        corner_style: by_label(&CornerStyle::ALL, CornerStyle::label, &s.corner_style),
        now_playing_style: by_label(
            &NowPlayingStyle::ALL,
            NowPlayingStyle::label,
            &s.now_playing_style,
        ),
        animated_covers: AnimatedCoverSettings {
            spotify_canvas: s.animated_covers.spotify_canvas,
        },
        // Unknown labels (an older or newer file) and the default order are skipped.
        collection_sorts: s
            .collection_sorts
            .iter()
            .filter_map(|(k, v)| Some((k.clone(), CollectionSort::from_label(v)?)))
            .filter(|(_, v)| *v != CollectionSort::Default)
            .take(MAX_REMEMBERED)
            .collect(),
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
    ui.theme.label().clone_into(&mut s.theme);
    ui.base.label().clone_into(&mut s.base_mode);
    ui.accent_intensity
        .label()
        .clone_into(&mut s.accent_intensity);
    ui.corner_style.label().clone_into(&mut s.corner_style);
    ui.now_playing_style
        .label()
        .clone_into(&mut s.now_playing_style);
    s.animated_covers.spotify_canvas = ui.animated_covers.spotify_canvas;
    s.collection_sorts = ui
        .collection_sorts
        .iter()
        .map(|(k, v)| (k.clone(), v.label().to_owned()))
        .collect();
}

/// An update to be saved to settings.
pub enum SettingsMsg {
    /// Settings updated from the UI.
    Ui(UiSettings),
    /// Output status updated from the player.
    Output(OutputStatus),
}

/// Update `settings.output_device` from the player's output status.
///
/// Returns `true` if `settings.output_device` changed.
pub fn apply_output_choice(settings: &mut Settings, status: &OutputStatus) -> bool {
    let new_choice = status.chosen.clone().map(StoredDevice::from);
    if settings.output_device == new_choice {
        false
    } else {
        settings.output_device = new_choice;
        true
    }
}

/// Start the saver thread and build the UI options. The callback only sends on a channel, so it
/// never blocks the UI thread. The returned sender also feeds the output watcher, which starts
/// with the player (see `backend::Startup`).
pub fn options(settings: Settings, root: PathBuf) -> (UiOptions, Sender<SettingsMsg>) {
    let initial = to_ui(&settings);
    let folders = settings
        .music_folders
        .iter()
        .map(|p| p.display().to_string())
        .collect();
    let saver = spawn_saver(settings, root);
    let tx = Mutex::new(saver.clone());
    let on_settings: Arc<dyn Fn(UiSettings) + Send + Sync> = Arc::new(move |ui| {
        if let Ok(tx) = tx.lock() {
            // The saver only stops when the app exits; a failed send then is harmless.
            let _ = tx.send(SettingsMsg::Ui(ui));
        }
    });
    let options = UiOptions {
        settings: initial,
        on_settings: Some(on_settings),
        music_folders: folders,
        // No folder picker yet: folders are edited in settings.json (see README).
        on_add_music_folder: None,
        maximized: true,
        // One switch: the OS preference here, OR the user's Settings toggle (the UI joins them).
        reduced_motion: crate::os_motion::reduce_motion(),
        ..UiOptions::default()
    };
    (options, saver)
}

/// Watch the player's event stream and forward `OutputChanged` to the settings saver.
pub fn spawn_output_watcher(
    player: &Arc<dyn PlayerControl>,
    tx: Sender<SettingsMsg>,
) -> Option<std::thread::JoinHandle<()>> {
    let player = Arc::clone(player);
    let spawned = std::thread::Builder::new()
        .name("output-watch".into())
        .spawn(move || {
            let mut events = player.subscribe();
            loop {
                while let Some(event) = events.try_next() {
                    if let PlayerEvent::OutputChanged(status) = event
                        && tx.send(SettingsMsg::Output(status)).is_err()
                    {
                        return;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
                if Arc::strong_count(&player) <= 1 {
                    break;
                }
            }
        });
    if let Err(err) = spawned.as_ref() {
        tracing::warn!(error = %err, "output choices will not be saved this session");
    }
    spawned.ok()
}

fn spawn_saver(mut settings: Settings, root: PathBuf) -> Sender<SettingsMsg> {
    let (tx, rx) = channel::<SettingsMsg>();
    let spawned = std::thread::Builder::new()
        .name("settings-save".into())
        .spawn(move || {
            while let Ok(msg) = rx.recv() {
                let mut changed = false;
                match msg {
                    SettingsMsg::Ui(mut ui) => {
                        // Keep only the newest change when the user drags a slider.
                        while let Ok(newer) = rx.try_recv() {
                            match newer {
                                SettingsMsg::Ui(u) => ui = u,
                                SettingsMsg::Output(status) => {
                                    apply_output_choice(&mut settings, &status);
                                }
                            }
                        }
                        apply_ui(&mut settings, &ui);
                        changed = true;
                    }
                    SettingsMsg::Output(status) => {
                        changed |= apply_output_choice(&mut settings, &status);
                    }
                }
                if changed && let Err(err) = settings.save(&root) {
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
    use crate::config::StoredDevice;

    #[test]
    fn round_trip_keeps_ui_fields_and_leaves_the_rest() {
        let mut s = Settings {
            music_folders: vec![PathBuf::from("C:/Music")],
            output_device: Some(StoredDevice {
                id: "dac-id".into(),
                name: "DAC".into(),
            }),
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
        assert_eq!(
            s.output_device.as_ref().map(|d| d.name.as_str()),
            Some("DAC")
        );
        assert_eq!(s.music_folders, [PathBuf::from("C:/Music")]);
        assert_eq!(to_ui(&s).glass_tier, UiGlass::Off);
    }

    #[test]
    fn now_playing_style_and_canvas_switch_round_trip() {
        let mut s = Settings::default();
        assert_eq!(to_ui(&s).now_playing_style, NowPlayingStyle::M3Expressive);
        assert!(!to_ui(&s).animated_covers.spotify_canvas);
        for style in NowPlayingStyle::ALL {
            let ui = UiSettings {
                now_playing_style: style,
                animated_covers: AnimatedCoverSettings {
                    spotify_canvas: true,
                },
                ..to_ui(&s)
            };
            apply_ui(&mut s, &ui);
            assert_eq!(to_ui(&s), ui);
        }
    }

    #[test]
    fn page_sorts_round_trip_and_unknown_labels_are_dropped() {
        let mut s = Settings::default();
        let mut ui = to_ui(&s);
        ui.collection_sorts
            .insert("playlist:PLa".to_owned(), CollectionSort::Title);
        ui.collection_sorts
            .insert("album:MPREb".to_owned(), CollectionSort::Duration);
        apply_ui(&mut s, &ui);
        assert_eq!(
            s.collection_sorts.get("playlist:PLa").map(String::as_str),
            Some("Title")
        );
        assert_eq!(to_ui(&s), ui);
        // A label from another version, and the default order, give no entry.
        s.collection_sorts
            .insert("playlist:old".to_owned(), "Newest first".to_owned());
        s.collection_sorts
            .insert("playlist:def".to_owned(), "Default order".to_owned());
        assert_eq!(to_ui(&s).collection_sorts.len(), 2);
        // The stored file keeps the field through JSON.
        let json = serde_json::to_string(&s).expect("serialises");
        let back: Settings = serde_json::from_str(&json).expect("parses");
        assert_eq!(back.collection_sorts, s.collection_sorts);
    }

    #[test]
    fn unknown_style_label_falls_back_to_m3() {
        let s = Settings {
            now_playing_style: "Winamp".into(),
            ..Settings::default()
        };
        assert_eq!(to_ui(&s).now_playing_style, NowPlayingStyle::M3Expressive);
    }

    #[test]
    fn higher_stored_tiers_show_as_low_in_the_ui() {
        let s = Settings {
            glass_tier: GlassTier::High,
            ..Settings::default()
        };
        assert_eq!(to_ui(&s).glass_tier, UiGlass::Low);
    }

    #[test]
    fn apply_output_choice_updates_and_ignores_same() {
        use nemo_core::output::{DeviceId, SavedDevice};

        let mut s = Settings::default();
        let mut status = OutputStatus::default();
        assert!(!apply_output_choice(&mut s, &status));

        status.chosen = Some(SavedDevice {
            id: DeviceId("dev-1".into()),
            name: "Headphones".into(),
        });
        assert!(apply_output_choice(&mut s, &status));
        assert_eq!(
            s.output_device,
            Some(StoredDevice {
                id: "dev-1".into(),
                name: "Headphones".into(),
            })
        );

        // Same status returns false
        assert!(!apply_output_choice(&mut s, &status));

        // Clearing back to None
        status.chosen = None;
        assert!(apply_output_choice(&mut s, &status));
        assert_eq!(s.output_device, None);
    }
}
