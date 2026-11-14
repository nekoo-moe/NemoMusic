//! Effects and song-transition settings, and the gapless-mark policy.
//!
//! Pure data, no I/O. Values use integer units (tenths of a dB, hundredths of Q, percent) so
//! `PlayerCommand` stays `Eq`. Every effect and the transition are off by default, and an
//! effect that is off is bypassed with no arithmetic (`code.md`, Bit-perfect). The control
//! thread in `nemo-audio` clamps out-of-range values with the limits below; the audio callback
//! never does.

use crate::track::Track;

/// Most filters the equalizer holds: the ten bands plus room for parametric profiles.
pub const EQ_MAX_FILTERS: usize = 12;

/// The shape of one equalizer filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FilterKind {
    /// A bell around the centre frequency.
    #[default]
    Peak,
    /// Boost or cut below the corner frequency.
    LowShelf,
    /// Boost or cut above the corner frequency.
    HighShelf,
}

/// One equalizer filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EqFilter {
    /// The filter shape.
    pub kind: FilterKind,
    /// Centre or corner frequency in Hz.
    pub freq_hz: u16,
    /// Gain in tenths of a dB, from [`GAIN_MIN_TENTH_DB`](Self::GAIN_MIN_TENTH_DB) to
    /// [`GAIN_MAX_TENTH_DB`](Self::GAIN_MAX_TENTH_DB).
    pub gain_tenth_db: i16,
    /// Quality factor in hundredths (141 is Q 1.41).
    pub q_hundredths: u16,
}

impl EqFilter {
    /// -12 dB: the lowest gain of a filter.
    pub const GAIN_MIN_TENTH_DB: i16 = -120;
    /// +12 dB: the highest gain of a filter.
    pub const GAIN_MAX_TENTH_DB: i16 = 120;
}

/// The equalizer: a preamp and up to [`EQ_MAX_FILTERS`] filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct EqSettings {
    /// Whether the equalizer runs. Off by default.
    pub enabled: bool,
    /// Preamp in tenths of a dB.
    pub preamp_tenth_db: i16,
    /// The filters, in order; `None` slots are unused.
    pub filters: [Option<EqFilter>; EQ_MAX_FILTERS],
}

/// The delay (echo) effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DelaySettings {
    /// Whether the delay runs. Off by default.
    pub enabled: bool,
    /// Delay time in ms, from [`TIME_MIN_MS`](Self::TIME_MIN_MS) to
    /// [`TIME_MAX_MS`](Self::TIME_MAX_MS).
    pub time_ms: u16,
    /// Feedback in percent, at most [`FEEDBACK_MAX_PCT`](Self::FEEDBACK_MAX_PCT), so the echo
    /// always decays.
    pub feedback_pct: u8,
    /// Wet share of the output in percent (0 to 100).
    pub mix_pct: u8,
}

impl DelaySettings {
    /// 1 ms: the shortest delay time.
    pub const TIME_MIN_MS: u16 = 1;
    /// 2 s: the longest delay time, which sizes the delay line.
    pub const TIME_MAX_MS: u16 = 2000;
    /// 85%: the most feedback, so the echo cannot grow without bound.
    pub const FEEDBACK_MAX_PCT: u8 = 85;
}

impl Default for DelaySettings {
    /// Off, with a 250 ms echo at 30% feedback and 30% mix ready for when it is turned on.
    fn default() -> Self {
        Self {
            enabled: false,
            time_ms: 250,
            feedback_pct: 30,
            mix_pct: 30,
        }
    }
}

/// The reverb effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReverbSettings {
    /// Whether the reverb runs. Off by default.
    pub enabled: bool,
    /// Room size in percent (0 to 100); bounded so the tail cannot grow.
    pub size_pct: u8,
    /// High-frequency damping in percent (0 to 100).
    pub damping_pct: u8,
    /// Wet share of the output in percent (0 to 100).
    pub mix_pct: u8,
}

impl Default for ReverbSettings {
    /// Off, with a medium room (50% size and damping) and 25% mix ready for when it is turned on.
    fn default() -> Self {
        Self {
            enabled: false,
            size_pct: 50,
            damping_pct: 50,
            mix_pct: 25,
        }
    }
}

/// Every effect of the chain. `Default` has every effect off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct EffectsSettings {
    /// The equalizer.
    pub equalizer: EqSettings,
    /// The delay.
    pub delay: DelaySettings,
    /// The reverb.
    pub reverb: ReverbSettings,
}

/// How the preamp was set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PreampMode {
    /// The user's preamp is used as set.
    #[default]
    Manual,
    /// The preamp was lowered to keep the combined boost under full scale.
    Auto {
        /// How far it was lowered, in tenths of a dB.
        lowered_by_tenth_db: u16,
    },
}

/// The effects as the player applies them, reported in `EffectsChanged`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct EffectsStatus {
    /// The settings after clamping.
    pub settings: EffectsSettings,
    /// How the preamp was set.
    pub preamp: PreampMode,
    /// Samples clamped at full scale since the stream started.
    pub clipped_samples: u64,
}

/// How one song leads into the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TransitionMode {
    /// Songs join gaplessly, untouched.
    #[default]
    Off,
    /// Every pair crossfades for the set length.
    Crossfade,
    /// Each pair gets a crossfade, a silence trim or a gapless join, chosen by a scan.
    Smart,
}

/// The fade on a manual skip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SkipFade {
    /// No fade: the skip cuts, with no arithmetic.
    #[default]
    Off,
    /// A 200 ms fade.
    Short,
}

impl SkipFade {
    /// The fade length in ms: 0 for [`Off`](Self::Off), 200 for [`Short`](Self::Short).
    #[must_use]
    pub const fn millis(self) -> u16 {
        match self {
            Self::Off => 0,
            Self::Short => 200,
        }
    }
}

/// Song-transition settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransitionSettings {
    /// The transition mode. Off by default.
    pub mode: TransitionMode,
    /// Crossfade length in seconds, from [`SECONDS_MIN`](Self::SECONDS_MIN) to
    /// [`SECONDS_MAX`](Self::SECONDS_MAX).
    pub seconds: u8,
    /// The fade on a manual skip.
    pub skip_fade: SkipFade,
    /// Whether songs marked gapless (a live album, a mix) skip the crossfade. On by default.
    pub honour_gapless_marks: bool,
}

impl TransitionSettings {
    /// 1 s: the shortest crossfade.
    pub const SECONDS_MIN: u8 = 1;
    /// 12 s: the longest crossfade.
    pub const SECONDS_MAX: u8 = 12;
}

impl Default for TransitionSettings {
    /// Off, with a 6 s length ready for when it is turned on, no skip fade, and gapless marks
    /// honoured.
    fn default() -> Self {
        Self {
            mode: TransitionMode::Off,
            seconds: 6,
            skip_fade: SkipFade::Off,
            honour_gapless_marks: true,
        }
    }
}

/// The transition used between two songs, reported in `TransitionStarted`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransitionKind {
    /// An equal-power overlap of the set length.
    Crossfade,
    /// The silences at the end and start were dropped, then a short overlap.
    TrimSilence,
    /// A plain gapless join.
    Gapless,
}

/// Why a transition did not run; the songs joined gaplessly instead. Not a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransitionSkipReason {
    /// The next song was not ready in time.
    NextNotReady,
    /// An exclusive-mode device had to reopen for the next song's format.
    FormatChangeInExclusive,
    /// One of the songs is too short for the set length.
    TooShort,
    /// The songs could not be scanned (for example a stream with no lead time).
    Analysis,
}

/// Decides whether two consecutive songs must join without a fade (gapless marks: one CUE
/// file, an album marked gapless). `nemo-library` implements it; `nemo-audio` calls it.
pub trait GaplessPolicy: Send + Sync {
    /// Whether `from` must lead into `to` gaplessly, with no crossfade.
    fn join_without_fade(&self, from: &Track, to: &Track) -> bool;
}

#[cfg(test)]
mod tests {
    use super::{
        DelaySettings, EQ_MAX_FILTERS, EffectsSettings, EffectsStatus, EqFilter, PreampMode,
        SkipFade, TransitionMode, TransitionSettings,
    };

    #[test]
    fn default_turns_every_effect_and_the_transition_off() {
        let e = EffectsSettings::default();
        assert!(!e.equalizer.enabled && !e.delay.enabled && !e.reverb.enabled);
        assert_eq!(e.equalizer.preamp_tenth_db, 0);
        assert!(e.equalizer.filters.iter().all(Option::is_none));
        let t = TransitionSettings::default();
        assert_eq!(t.mode, TransitionMode::Off);
        assert_eq!(t.skip_fade, SkipFade::Off);
        assert!(t.honour_gapless_marks);
        let s = EffectsStatus::default();
        assert_eq!(s.preamp, PreampMode::Manual);
        assert_eq!(s.clipped_samples, 0);
    }

    #[test]
    fn documented_ranges_are_the_clamp_constants() {
        assert_eq!(EQ_MAX_FILTERS, 12);
        assert_eq!(EqFilter::GAIN_MIN_TENTH_DB, -120);
        assert_eq!(EqFilter::GAIN_MAX_TENTH_DB, 120);
        assert_eq!(DelaySettings::TIME_MIN_MS, 1);
        assert_eq!(DelaySettings::TIME_MAX_MS, 2000);
        assert_eq!(DelaySettings::FEEDBACK_MAX_PCT, 85);
        assert_eq!(TransitionSettings::SECONDS_MIN, 1);
        assert_eq!(TransitionSettings::SECONDS_MAX, 12);
        assert_eq!(SkipFade::Off.millis(), 0);
        assert_eq!(SkipFade::Short.millis(), 200);
    }

    #[test]
    fn defaults_sit_inside_the_ranges() {
        let d = DelaySettings::default();
        assert!((DelaySettings::TIME_MIN_MS..=DelaySettings::TIME_MAX_MS).contains(&d.time_ms));
        assert!(d.feedback_pct <= DelaySettings::FEEDBACK_MAX_PCT);
        let t = TransitionSettings::default();
        let range = TransitionSettings::SECONDS_MIN..=TransitionSettings::SECONDS_MAX;
        assert!(range.contains(&t.seconds));
    }
}
