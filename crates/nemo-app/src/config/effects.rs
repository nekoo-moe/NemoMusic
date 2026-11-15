//! The effects and song-transition settings as stored in `settings.json` (audio-effects
//! task 4, criteria 1 and 27).
//!
//! `nemo-core` has no `serde`, so these are plain mirrors of its `effects` types with
//! conversions both ways. Every missing field takes the `nemo-core` default, which has every
//! effect and the transition off, so a file from an older app (or a newer field this app does
//! not know) never turns an effect on. [`StoredEffects::sanitized`] and
//! [`StoredTransition::sanitized`] clamp the documented ranges of a hand-edited file.

use nemo_core::effects::{
    DelaySettings, EQ_MAX_FILTERS, EffectsSettings, EqFilter, EqSettings, FilterKind,
    ReverbSettings, SkipFade, TransitionMode, TransitionSettings,
};
use serde::{Deserialize, Serialize};

/// Highest percentage of a mix, size or damping value.
const PCT_MAX: u8 = 100;

/// The shape of one stored equalizer filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StoredFilterKind {
    /// A bell around the centre frequency.
    #[default]
    Peak,
    /// Boost or cut below the corner frequency.
    LowShelf,
    /// Boost or cut above the corner frequency.
    HighShelf,
}

/// One stored equalizer filter, in the integer units of `nemo-core`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct StoredFilter {
    /// The filter shape.
    pub kind: StoredFilterKind,
    /// Centre or corner frequency in Hz.
    pub freq_hz: u16,
    /// Gain in tenths of a dB (-120 to 120).
    pub gain_tenth_db: i16,
    /// Quality factor in hundredths.
    pub q_hundredths: u16,
}

/// The stored equalizer. Off when missing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct StoredEq {
    /// Whether the equalizer runs.
    pub enabled: bool,
    /// Preamp in tenths of a dB.
    pub preamp_tenth_db: i16,
    /// The filters in order, at most [`EQ_MAX_FILTERS`] (extra ones are dropped).
    pub filters: Vec<StoredFilter>,
}

/// The stored delay. Off when missing, with the `nemo-core` default values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StoredDelay {
    /// Whether the delay runs.
    pub enabled: bool,
    /// Delay time in ms (1 to 2000).
    pub time_ms: u16,
    /// Feedback in percent (at most 85).
    pub feedback_pct: u8,
    /// Wet share in percent (0 to 100).
    pub mix_pct: u8,
}

/// The stored reverb. Off when missing, with the `nemo-core` default values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StoredReverb {
    /// Whether the reverb runs.
    pub enabled: bool,
    /// Room size in percent (0 to 100).
    pub size_pct: u8,
    /// High-frequency damping in percent (0 to 100).
    pub damping_pct: u8,
    /// Wet share in percent (0 to 100).
    pub mix_pct: u8,
}

/// Every stored effect. `Default` has every effect off.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StoredEffects {
    /// The equalizer.
    pub equalizer: StoredEq,
    /// The delay.
    pub delay: StoredDelay,
    /// The reverb.
    pub reverb: StoredReverb,
}

/// The stored transition mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StoredTransitionMode {
    /// Songs join gaplessly, untouched.
    #[default]
    Off,
    /// Every pair crossfades.
    Crossfade,
    /// A crossfade, a silence trim or a gapless join per pair.
    Smart,
}

/// The stored fade on a manual skip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StoredSkipFade {
    /// No fade.
    #[default]
    Off,
    /// A 200 ms fade.
    Short,
}

/// The stored song-transition settings. Off when missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StoredTransition {
    /// The transition mode.
    pub mode: StoredTransitionMode,
    /// Crossfade length in seconds (1 to 12).
    pub seconds: u8,
    /// The fade on a manual skip.
    pub skip_fade: StoredSkipFade,
    /// Whether songs marked gapless skip the crossfade.
    pub honour_gapless_marks: bool,
}

impl From<FilterKind> for StoredFilterKind {
    fn from(kind: FilterKind) -> Self {
        match kind {
            FilterKind::Peak => Self::Peak,
            FilterKind::LowShelf => Self::LowShelf,
            FilterKind::HighShelf => Self::HighShelf,
        }
    }
}

impl From<StoredFilterKind> for FilterKind {
    fn from(kind: StoredFilterKind) -> Self {
        match kind {
            StoredFilterKind::Peak => Self::Peak,
            StoredFilterKind::LowShelf => Self::LowShelf,
            StoredFilterKind::HighShelf => Self::HighShelf,
        }
    }
}

impl From<EqFilter> for StoredFilter {
    fn from(f: EqFilter) -> Self {
        Self {
            kind: f.kind.into(),
            freq_hz: f.freq_hz,
            gain_tenth_db: f.gain_tenth_db,
            q_hundredths: f.q_hundredths,
        }
    }
}

impl From<StoredFilter> for EqFilter {
    fn from(f: StoredFilter) -> Self {
        Self {
            kind: f.kind.into(),
            freq_hz: f.freq_hz,
            gain_tenth_db: f.gain_tenth_db,
            q_hundredths: f.q_hundredths,
        }
    }
}

impl From<DelaySettings> for StoredDelay {
    fn from(d: DelaySettings) -> Self {
        Self {
            enabled: d.enabled,
            time_ms: d.time_ms,
            feedback_pct: d.feedback_pct,
            mix_pct: d.mix_pct,
        }
    }
}

impl From<ReverbSettings> for StoredReverb {
    fn from(r: ReverbSettings) -> Self {
        Self {
            enabled: r.enabled,
            size_pct: r.size_pct,
            damping_pct: r.damping_pct,
            mix_pct: r.mix_pct,
        }
    }
}

impl Default for StoredDelay {
    fn default() -> Self {
        DelaySettings::default().into()
    }
}

impl Default for StoredReverb {
    fn default() -> Self {
        ReverbSettings::default().into()
    }
}

impl From<EffectsSettings> for StoredEffects {
    fn from(e: EffectsSettings) -> Self {
        Self {
            equalizer: StoredEq {
                enabled: e.equalizer.enabled,
                preamp_tenth_db: e.equalizer.preamp_tenth_db,
                filters: e
                    .equalizer
                    .filters
                    .iter()
                    .flatten()
                    .map(|&f| f.into())
                    .collect(),
            },
            delay: e.delay.into(),
            reverb: e.reverb.into(),
        }
    }
}

impl Default for StoredEffects {
    fn default() -> Self {
        EffectsSettings::default().into()
    }
}

impl StoredEffects {
    /// The settings the player gets. Filters past [`EQ_MAX_FILTERS`] are dropped.
    #[must_use]
    pub fn to_core(&self) -> EffectsSettings {
        let mut filters = [None; EQ_MAX_FILTERS];
        for (slot, f) in filters.iter_mut().zip(&self.equalizer.filters) {
            *slot = Some(EqFilter::from(*f));
        }
        let (d, r) = (self.delay, self.reverb);
        EffectsSettings {
            equalizer: EqSettings {
                enabled: self.equalizer.enabled,
                preamp_tenth_db: self.equalizer.preamp_tenth_db,
                filters,
            },
            delay: DelaySettings {
                enabled: d.enabled,
                time_ms: d.time_ms,
                feedback_pct: d.feedback_pct,
                mix_pct: d.mix_pct,
            },
            reverb: ReverbSettings {
                enabled: r.enabled,
                size_pct: r.size_pct,
                damping_pct: r.damping_pct,
                mix_pct: r.mix_pct,
            },
        }
    }

    /// Clamp the documented ranges: filter gain -12 to +12 dB, at most [`EQ_MAX_FILTERS`]
    /// filters, delay 1 to 2000 ms, feedback at most 85%, every percentage at most 100.
    #[must_use]
    pub fn sanitized(mut self) -> Self {
        self.equalizer.filters.truncate(EQ_MAX_FILTERS);
        for f in &mut self.equalizer.filters {
            f.gain_tenth_db = f
                .gain_tenth_db
                .clamp(EqFilter::GAIN_MIN_TENTH_DB, EqFilter::GAIN_MAX_TENTH_DB);
        }
        let d = &mut self.delay;
        d.time_ms = d
            .time_ms
            .clamp(DelaySettings::TIME_MIN_MS, DelaySettings::TIME_MAX_MS);
        d.feedback_pct = d.feedback_pct.min(DelaySettings::FEEDBACK_MAX_PCT);
        d.mix_pct = d.mix_pct.min(PCT_MAX);
        let r = &mut self.reverb;
        r.size_pct = r.size_pct.min(PCT_MAX);
        r.damping_pct = r.damping_pct.min(PCT_MAX);
        r.mix_pct = r.mix_pct.min(PCT_MAX);
        self
    }
}

impl From<TransitionSettings> for StoredTransition {
    fn from(t: TransitionSettings) -> Self {
        Self {
            mode: match t.mode {
                TransitionMode::Off => StoredTransitionMode::Off,
                TransitionMode::Crossfade => StoredTransitionMode::Crossfade,
                TransitionMode::Smart => StoredTransitionMode::Smart,
            },
            seconds: t.seconds,
            skip_fade: match t.skip_fade {
                SkipFade::Off => StoredSkipFade::Off,
                SkipFade::Short => StoredSkipFade::Short,
            },
            honour_gapless_marks: t.honour_gapless_marks,
        }
    }
}

impl Default for StoredTransition {
    fn default() -> Self {
        TransitionSettings::default().into()
    }
}

impl StoredTransition {
    /// The settings the player gets.
    #[must_use]
    pub fn to_core(self) -> TransitionSettings {
        TransitionSettings {
            mode: match self.mode {
                StoredTransitionMode::Off => TransitionMode::Off,
                StoredTransitionMode::Crossfade => TransitionMode::Crossfade,
                StoredTransitionMode::Smart => TransitionMode::Smart,
            },
            seconds: self.seconds,
            skip_fade: match self.skip_fade {
                StoredSkipFade::Off => SkipFade::Off,
                StoredSkipFade::Short => SkipFade::Short,
            },
            honour_gapless_marks: self.honour_gapless_marks,
        }
    }

    /// Clamp the crossfade length to 1 to 12 s.
    #[must_use]
    pub fn sanitized(mut self) -> Self {
        self.seconds = self.seconds.clamp(
            TransitionSettings::SECONDS_MIN,
            TransitionSettings::SECONDS_MAX,
        );
        self
    }
}

#[cfg(test)]
mod tests;
