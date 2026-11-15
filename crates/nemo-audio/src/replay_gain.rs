//! `ReplayGain`: one gain per song from its stored values, applied by the feeder. Off by default.
//!
//! The control thread turns the song's [`ReplayGainValues`] and the user's
//! [`ReplayGainSettings`] into a [`ReplayGain`] stage when it hands the song to the feeder, so
//! a change applies from the next song (the ring already holds up to 1 s of the current one).
//! The feeder multiplies each converted block by the one scalar, after the song's own `Chain`
//! and before the ring (audio-effects design, Data Flow 9). Off, no stored value, or a gain of
//! exactly 0 dB builds no stage: no arithmetic, and the bit-perfect path stays.

use std::sync::{Arc, Mutex, PoisonError};

use nemo_core::audio::ProcessingStage;
use nemo_core::library::{ReplayGainMode, ReplayGainValues};

/// The largest pre-gain, in dB either way. A larger value is limited to it.
pub const PRE_GAIN_MAX_DB: f32 = 15.0;

/// The user's `ReplayGain` setting. Off by default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReplayGainSettings {
    /// Whether `ReplayGain` is applied. Off by default.
    pub enabled: bool,
    /// Track or album gain.
    pub mode: ReplayGainMode,
    /// Added to the stored gain, in dB, from -15 to +15.
    pub pre_gain_db: f32,
}

impl Default for ReplayGainSettings {
    /// Off, in track mode, with no pre-gain.
    fn default() -> Self {
        Self {
            enabled: false,
            mode: ReplayGainMode::Track,
            pre_gain_db: 0.0,
        }
    }
}

/// The setting, shared by the app, which changes it, and the player, which reads it each time
/// it hands a song to the feeder. Clones share one value.
#[derive(Debug, Clone, Default)]
pub struct ReplayGainSwitch(Arc<Mutex<ReplayGainSettings>>);

impl ReplayGainSwitch {
    /// Use `settings` from the next song on.
    pub fn set(&self, settings: ReplayGainSettings) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = settings;
    }

    /// The setting now.
    #[must_use]
    pub fn get(&self) -> ReplayGainSettings {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The gain of one song: a stage in its signal path while it plays.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReplayGain {
    db: f32,
    scalar: f32,
}

impl ReplayGain {
    /// The stage for a song with `values` under `settings`, or `None` when the setting is off,
    /// the song has no usable value, or the gain comes to exactly 0 dB.
    ///
    /// Track mode takes the track gain and falls back to the album gain; album mode the other
    /// way round. The pre-gain is added, then the scalar is lowered so that the matching peak
    /// times the scalar stays at or below 1.0 when the song states that peak.
    #[must_use]
    pub fn plan(settings: &ReplayGainSettings, values: &ReplayGainValues) -> Option<Self> {
        if !settings.enabled {
            return None;
        }
        let track = values.track_gain_db.map(|gain| (gain, values.track_peak));
        let album = values.album_gain_db.map(|gain| (gain, values.album_peak));
        let (gain, peak) = match settings.mode {
            ReplayGainMode::Track => track.or(album),
            ReplayGainMode::Album => album.or(track),
        }?;
        let pre_gain = if settings.pre_gain_db.is_finite() {
            settings
                .pre_gain_db
                .clamp(-PRE_GAIN_MAX_DB, PRE_GAIN_MAX_DB)
        } else {
            0.0
        };
        let mut scalar = db_to_scalar(gain + pre_gain);
        if let Some(peak) = peak.filter(|p| p.is_finite() && *p > 0.0)
            && scalar * peak > 1.0
        {
            scalar = 1.0 / peak;
        }
        let usable = scalar.is_finite() && scalar > 0.0 && scalar.to_bits() != 1.0_f32.to_bits();
        usable.then(|| Self {
            db: scalar_to_db(scalar),
            scalar,
        })
    }

    /// The gain in dB, after the peak limit.
    #[must_use]
    pub const fn db(&self) -> f32 {
        self.db
    }

    /// The linear factor every sample is multiplied by.
    #[must_use]
    pub const fn scalar(&self) -> f32 {
        self.scalar
    }

    /// The signal-path stage, [`ProcessingStage::ReplayGain`].
    #[must_use]
    pub const fn stage(&self) -> ProcessingStage {
        ProcessingStage::ReplayGain { db: self.db }
    }

    /// Scale `samples` in place. Runs on the feeder thread; allocates nothing.
    pub fn apply(&self, samples: &mut [f32]) {
        for sample in samples {
            *sample *= self.scalar;
        }
    }
}

/// A gain in dB as a linear factor: `10^(db / 20)`.
#[must_use]
pub fn db_to_scalar(db: f32) -> f32 {
    10_f32.powf(db / 20.0)
}

/// A linear factor as a gain in dB: `20 log10(scalar)`.
#[must_use]
pub fn scalar_to_db(scalar: f32) -> f32 {
    20.0 * scalar.log10()
}

#[cfg(test)]
mod tests;
