//! The chosen output device and the output options as stored in `settings.json`
//! (output-devices task 5, criteria 6 and 7).
//!
//! The device is saved by ID and name; the name is only the label shown while the device is
//! absent. A file from before IDs holds a plain name: it loads with an empty ID, is matched by
//! name once against the device list ([`StoredDevice::upgrade`]), and is then saved with the
//! ID. `nemo-core` has no `serde`, so the options are a plain mirror of `OutputOptions`.

use nemo_core::output::{DeviceId, OutputDevice, OutputOptions, SavedDevice};
use serde::{Deserialize, Serialize};

/// The device the user chose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "DeviceField")]
pub struct StoredDevice {
    /// The audio backend's device ID; empty for a name from an older file, until it is
    /// matched.
    pub id: String,
    /// The name the device had when chosen.
    pub name: String,
}

/// What the file may hold: a plain name (older files) or the ID and name.
#[derive(Deserialize)]
#[serde(untagged)]
enum DeviceField {
    Name(String),
    Saved {
        #[serde(default)]
        id: String,
        #[serde(default)]
        name: String,
    },
}

impl From<DeviceField> for StoredDevice {
    fn from(field: DeviceField) -> Self {
        match field {
            DeviceField::Name(name) => Self {
                id: String::new(),
                name,
            },
            DeviceField::Saved { id, name } => Self { id, name },
        }
    }
}

impl From<SavedDevice> for StoredDevice {
    fn from(saved: SavedDevice) -> Self {
        Self {
            id: saved.id.0,
            name: saved.name,
        }
    }
}

impl StoredDevice {
    /// The device for the player; `None` while a name from an older file has no ID yet.
    #[must_use]
    pub fn saved(&self) -> Option<SavedDevice> {
        (!self.id.is_empty()).then(|| SavedDevice {
            id: DeviceId(self.id.clone()),
            name: self.name.clone(),
        })
    }

    /// Match a name from an older file against `devices` once: when exactly one device has
    /// that name, take its ID. `true` when the ID was filled in, so the caller saves the file.
    /// A device that already has an ID, or a name that matches none or several, is left as is.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "called when the player reports its device list (output-devices tasks 3 and 6)"
        )
    )]
    pub fn upgrade(&mut self, devices: &[OutputDevice]) -> bool {
        if !self.id.is_empty() {
            return false;
        }
        let mut named = devices.iter().filter(|d| d.name == self.name);
        match (named.next(), named.next()) {
            (Some(device), None) => {
                self.id.clone_from(&device.id.0);
                true
            }
            _ => false,
        }
    }
}

/// The stored output options. Missing fields take the `nemo-core` defaults (shared mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StoredOutputOptions {
    /// Open the device in exclusive mode (Windows WASAPI).
    pub exclusive: bool,
    /// Seconds idle before the device is released (1 to 60).
    pub idle_release_secs: u8,
    /// The exclusive-mode buffer in ms (5 to 100).
    pub exclusive_buffer_ms: u8,
    /// Resume playback when the chosen device returns.
    pub auto_resume: bool,
}

impl From<OutputOptions> for StoredOutputOptions {
    fn from(o: OutputOptions) -> Self {
        Self {
            exclusive: o.exclusive,
            idle_release_secs: o.idle_release_secs,
            exclusive_buffer_ms: o.exclusive_buffer_ms,
            auto_resume: o.auto_resume,
        }
    }
}

impl Default for StoredOutputOptions {
    fn default() -> Self {
        OutputOptions::default().into()
    }
}

impl StoredOutputOptions {
    /// The options for the player.
    #[must_use]
    pub fn to_core(self) -> OutputOptions {
        OutputOptions {
            exclusive: self.exclusive,
            idle_release_secs: self.idle_release_secs,
            exclusive_buffer_ms: self.exclusive_buffer_ms,
            auto_resume: self.auto_resume,
        }
    }

    /// Clamp the numbers into their ranges (`OutputOptions::clamped`).
    #[must_use]
    pub fn sanitized(self) -> Self {
        self.to_core().clamped().into()
    }
}

#[cfg(test)]
mod tests;
