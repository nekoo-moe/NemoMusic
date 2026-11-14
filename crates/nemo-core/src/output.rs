//! Output devices: which device plays, the exclusive-mode options and what the player reports.
//!
//! No I/O here. `nemo-audio` lists and opens devices and fills [`OutputStatus`]; the UI picks a
//! device and options through player commands. Device ids and names are never logged at info
//! level or above, and the `Debug` form of [`DeviceId`] prints a hash, never the id.

use std::fmt;
use std::hash::{DefaultHasher, Hash, Hasher};

/// An output device id: the audio backend's device id in its text form.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct DeviceId(pub String);

impl fmt::Debug for DeviceId {
    /// Prints a hash of the id, so logs and panics never carry it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut h = DefaultHasher::new();
        self.0.hash(&mut h);
        write!(f, "DeviceId(#{:016x})", h.finish())
    }
}

/// One output device the system offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputDevice {
    /// The device id.
    pub id: DeviceId,
    /// The name the system shows.
    pub name: String,
    /// The system's default output device.
    pub is_default: bool,
}

/// The device the user chose, as stored in the settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedDevice {
    /// The device id.
    pub id: DeviceId,
    /// The name it had when chosen, shown while it is missing.
    pub name: String,
}

/// How the output device is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutputOptions {
    /// Open the device in exclusive mode (Windows WASAPI). Off by default.
    pub exclusive: bool,
    /// Seconds of pause or stop before the device is released, from
    /// [`IDLE_RELEASE_MIN`](Self::IDLE_RELEASE_MIN) to [`IDLE_RELEASE_MAX`](Self::IDLE_RELEASE_MAX).
    pub idle_release_secs: u8,
    /// The exclusive-mode buffer in ms, from [`BUFFER_MIN_MS`](Self::BUFFER_MIN_MS) to
    /// [`BUFFER_MAX_MS`](Self::BUFFER_MAX_MS).
    pub exclusive_buffer_ms: u8,
    /// Resume playback when the chosen device returns. Off by default.
    pub auto_resume: bool,
}

impl OutputOptions {
    /// 1 s: the shortest idle time before the device is released.
    pub const IDLE_RELEASE_MIN: u8 = 1;
    /// 60 s: the longest idle time before the device is released.
    pub const IDLE_RELEASE_MAX: u8 = 60;
    /// 5 ms: the smallest exclusive-mode buffer.
    pub const BUFFER_MIN_MS: u8 = 5;
    /// 100 ms: the largest exclusive-mode buffer.
    pub const BUFFER_MAX_MS: u8 = 100;

    /// These options with every number clamped into its range.
    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            idle_release_secs: self
                .idle_release_secs
                .clamp(Self::IDLE_RELEASE_MIN, Self::IDLE_RELEASE_MAX),
            exclusive_buffer_ms: self
                .exclusive_buffer_ms
                .clamp(Self::BUFFER_MIN_MS, Self::BUFFER_MAX_MS),
            ..self
        }
    }
}

impl Default for OutputOptions {
    /// Shared mode, the device released after 5 s idle, a 20 ms exclusive buffer ready for when
    /// exclusive mode is turned on, and no automatic resume.
    fn default() -> Self {
        Self {
            exclusive: false,
            idle_release_secs: 5,
            exclusive_buffer_ms: 20,
            auto_resume: false,
        }
    }
}

/// Why exclusive mode was not used and the device opened shared instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FallbackReason {
    /// Another app holds the device.
    DeviceBusy,
    /// The driver refused exclusive mode.
    DriverRefused,
    /// "Allow applications to take exclusive control" is off for this device.
    ExclusiveControlDisabled,
    /// The device accepts no format close to the source in exclusive mode.
    FormatNotAccepted,
    /// Exclusive mode does not exist on this system.
    NotSupportedHere,
}

/// The output as the player has it, reported in `OutputChanged` and the snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutputStatus {
    /// The devices the system offers.
    pub devices: Vec<OutputDevice>,
    /// The device the user chose; `None` follows the system default.
    pub chosen: Option<SavedDevice>,
    /// The chosen device is not present; the system default plays.
    pub saved_missing: bool,
    /// The device that plays now.
    pub active: Option<OutputDevice>,
    /// Exclusive mode can be offered on this system.
    pub exclusive_available: bool,
    /// The options in use.
    pub options: OutputOptions,
    /// Why exclusive mode fell back to shared, if it did.
    pub fallback: Option<FallbackReason>,
    /// The name of a device that was lost while playing.
    pub lost: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{DeviceId, OutputOptions, OutputStatus};

    #[test]
    fn default_options_are_shared_with_documented_values() {
        let o = OutputOptions::default();
        assert!(!o.exclusive && !o.auto_resume);
        assert_eq!(o.idle_release_secs, 5);
        assert_eq!(o.exclusive_buffer_ms, 20);
        assert_eq!(o.clamped(), o);
        let s = OutputStatus::default();
        assert!(s.chosen.is_none() && s.active.is_none() && s.fallback.is_none());
        assert_eq!(s.options, o);
    }

    #[test]
    fn options_clamp_into_their_ranges() {
        let low = OutputOptions {
            idle_release_secs: 0,
            exclusive_buffer_ms: 1,
            ..OutputOptions::default()
        }
        .clamped();
        assert_eq!((low.idle_release_secs, low.exclusive_buffer_ms), (1, 5));
        let high = OutputOptions {
            exclusive: true,
            idle_release_secs: 200,
            exclusive_buffer_ms: 250,
            auto_resume: true,
        }
        .clamped();
        assert_eq!(
            (high.idle_release_secs, high.exclusive_buffer_ms),
            (60, 100)
        );
        assert!(high.exclusive && high.auto_resume);
    }

    #[test]
    fn debug_of_a_device_id_hides_the_id() {
        let id = DeviceId("{0.0.0.00000000}.{8d1f-speakers}".to_owned());
        let text = format!("{id:?}");
        assert!(
            !text.contains("speakers") && !text.contains("0.0.0"),
            "{text}"
        );
        assert!(text.starts_with("DeviceId(#"));
        assert_eq!(
            text,
            format!("{:?}", id.clone()),
            "the same id prints the same"
        );
        assert_ne!(text, format!("{:?}", DeviceId("other".to_owned())));
    }
}
