//! Wording of the Appearance page (appearance-glass spec). One plain sentence for each helper.

/// Title of the Background card.
pub const BACKGROUND_TITLE: &str = "Background";
/// Helper of the Background card.
pub const BACKGROUND_HELPER: &str = "What lives behind the panes while a song plays.";
/// Added to the heartbeat helper while the switch is greyed out.
pub const HEARTBEAT_OFF_NOTE: &str = "Off while song colours are off or motion is reduced.";
/// Helper of the Surface card.
pub const SURFACE_HELPER: &str =
    "Dark is the default. AMOLED black saves power on OLED screens. Light is for bright rooms.";
/// Helper of the Colour theme card.
pub const THEME_HELPER: &str = "Dynamic follows the cover of the song that plays. The others keep one palette when the song colours are off.";
/// Helper of the Accent intensity card.
pub const ACCENT_HELPER: &str =
    "How strong the accent colour is. Text stays at 4.5:1 contrast or better.";
/// Helper of the Corners card.
pub const CORNERS_HELPER: &str = "Sharp is the Material 3 default. Round is extra soft.";
/// Helper of the Now Playing style card.
pub const NOW_PLAYING_HELPER: &str =
    "How the Now Playing pane and the full-screen view are arranged. Playback does not change.";
/// Title of the player bar card.
pub const BAR_TITLE: &str = "Player bar";
/// Helper of the player bar card. It says what the bar is and what makes it opaque.
pub const BAR_HELPER: &str = "A Material surface with a soft shadow. The background shows through a little. Reduce transparency and Window glass make it fully opaque.";
/// Title of the Window glass card.
pub const WINDOW_GLASS_TITLE: &str = "Window glass";
/// Helper of the Window glass switch.
pub const WINDOW_GLASS_HELPER: &str = "Makes the window itself see-through, so other windows and the desktop show through the gaps and edges. Panes stay readable.";
/// The pill that restarts the app once the Window glass switch needs it.
pub const RESTART_PILL: &str = "Restart now";
/// The short line that goes with the pill. It says what the restart costs.
pub const RESTART_HELPER: &str = "Needs a restart. The music stops for a moment.";
/// The one sentence shown when the new copy of the app could not start.
pub const RESTART_FAILED: &str = "NemoMusic could not restart. Close it and start it again.";
/// Title of the opacity row.
pub const OPACITY_TITLE: &str = "Opacity";
/// Helper of the opacity row.
pub const OPACITY_HELPER: &str =
    "How much of the window background shows. Lower lets more through.";
/// What blur needs: built in on Windows, the compositor elsewhere. NemoMusic draws none.
pub const WINDOW_GLASS_BLUR: &str = "On Windows the blur is built in. On KDE it needs the Blur effect of the compositor, with Force blur for this window. Other desktops show clear glass without blur.";
/// Title of the Motion and accessibility card.
pub const MOTION_TITLE: &str = "Motion and accessibility";
/// Helper of the Reduce motion switch.
pub const REDUCE_MOTION_HELPER: &str =
    "No cross-fades and no lyric glide. The progress bar keeps moving.";
/// Helper of the Reduce transparency switch.
pub const REDUCE_TRANSPARENCY_HELPER: &str =
    "The player bar becomes fully opaque, and the window stays solid.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_glass_text_is_honest_and_the_restart_note_is_exact() {
        assert!(BAR_HELPER.contains("Material") && !BAR_HELPER.contains("Low"));
        assert_eq!(RESTART_PILL, "Restart now");
        assert!(RESTART_HELPER.len() < 60 && RESTART_FAILED.contains("could not restart"));
        assert!(WINDOW_GLASS_BLUR.contains("compositor"));
        assert!(WINDOW_GLASS_BLUR.contains("Force blur"));
        assert!(WINDOW_GLASS_BLUR.contains("Windows"));
    }
}
