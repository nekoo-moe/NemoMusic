//! Wording of the Discord presence row in Settings, Privacy (discord-presence spec). The notice
//! itself (what is sent, risk) is the `OptInInfo` of `nemo-integrations`.

/// Under the switch while it is off.
pub const STAYS: &str = "Nothing is shown on Discord while this is off.";
/// Under the switch while it is on.
pub const STATUS_ON: &str = "Shown while the Discord app runs on this computer. When Discord is not running, nothing is shown and nothing is reported.";
/// Title of the switch that hides the song names.
pub const HIDE_TITLE: &str = "Hide song names on Discord";
/// Helper of the switch that hides the song names.
pub const HIDE_HELPER: &str =
    "Discord then shows only that you listen to NemoMusic: no title, artist, cover or time.";
