//! Audio format, codec, quality class and signal-path types.
//!
//! The honest-label rules from `product.md` live here as code, so no UI can break them:
//! a YouTube Music stream is never [`QualityClass::Lossless`], and a path is bit-perfect only
//! when nothing alters the samples.

use crate::effects::TransitionKind;

/// Thin space used between a number and its unit (`design.md`, Typography).
const THIN_SPACE: char = '\u{2009}';

/// Audio codec of a source stream or file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Codec {
    /// Free Lossless Audio Codec.
    Flac,
    /// Apple Lossless.
    Alac,
    /// Uncompressed PCM (WAV, AIFF).
    Pcm,
    /// Advanced Audio Coding (lossy).
    Aac,
    /// Opus (lossy).
    Opus,
    /// MPEG Layer 3 (lossy).
    Mp3,
    /// Ogg Vorbis (lossy).
    Vorbis,
    /// The codec could not be determined.
    Unknown,
}

impl Codec {
    /// `true` only for codecs that keep every source sample.
    #[must_use]
    pub const fn is_lossless(self) -> bool {
        matches!(self, Self::Flac | Self::Alac | Self::Pcm)
    }

    /// `true` only for codecs known to discard audio data.
    #[must_use]
    pub const fn is_lossy(self) -> bool {
        matches!(self, Self::Aac | Self::Opus | Self::Mp3 | Self::Vorbis)
    }

    /// Short name for quality badges.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Flac => "FLAC",
            Self::Alac => "ALAC",
            Self::Pcm => "PCM",
            Self::Aac => "AAC",
            Self::Opus => "Opus",
            Self::Mp3 => "MP3",
            Self::Vorbis => "Vorbis",
            Self::Unknown => "Unknown",
        }
    }
}

/// Where a stream comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Origin {
    /// A file on this computer.
    Local,
    /// A YouTube Music stream. Always lossy.
    YoutubeMusic,
    /// A public SoundCloud stream, found through the `yt-dlp` URL resolver. Always lossy.
    SoundCloud,
}

/// Sample rate, channel count and (when known) bit depth of PCM audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AudioFormat {
    /// Samples per second per channel, in hertz.
    pub sample_rate: u32,
    /// Number of channels.
    pub channels: u16,
    /// Bits per sample, if the source states it. Lossy codecs usually do not.
    pub bit_depth: Option<u8>,
}

/// Facts measured from a stream. Never guessed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamInfo {
    /// Where the stream comes from.
    pub origin: Origin,
    /// The codec.
    pub codec: Codec,
    /// Average bitrate in kilobits per second, if known.
    pub bitrate_kbps: Option<u32>,
    /// PCM format after decoding, if known.
    pub format: Option<AudioFormat>,
}

/// How the quality of a stream may be described to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityClass {
    /// Every source sample is kept. Only for local lossless files.
    Lossless,
    /// The source discarded audio data.
    Lossy,
    /// Not enough facts. Show "Unknown", never a guess.
    Unknown,
}

impl StreamInfo {
    /// Quality class under the honest-label rule.
    ///
    /// A YouTube Music or SoundCloud stream is always [`QualityClass::Lossy`], whatever the
    /// codec says. [`QualityClass::Lossless`] needs a local file with a lossless codec.
    #[must_use]
    pub const fn quality_class(&self) -> QualityClass {
        match self.origin {
            Origin::YoutubeMusic | Origin::SoundCloud => QualityClass::Lossy,
            Origin::Local => {
                if self.codec.is_lossless() {
                    QualityClass::Lossless
                } else if self.codec.is_lossy() {
                    QualityClass::Lossy
                } else {
                    QualityClass::Unknown
                }
            }
        }
    }

    /// Badge text built from measured facts, for example `Opus 160 kbps` or
    /// `FLAC 96 kHz / 24-bit`. Unknown values print `Unknown`.
    #[must_use]
    pub fn badge(&self) -> String {
        let name = self.codec.name();
        if self.quality_class() == QualityClass::Lossless {
            return match self.format {
                Some(AudioFormat {
                    sample_rate,
                    bit_depth: Some(bits),
                    ..
                }) => {
                    format!("{name} {}{THIN_SPACE}kHz / {bits}-bit", khz(sample_rate))
                }
                Some(AudioFormat {
                    sample_rate,
                    bit_depth: None,
                    ..
                }) => {
                    format!("{name} {}{THIN_SPACE}kHz", khz(sample_rate))
                }
                None => format!("{name} Unknown"),
            };
        }
        match self.bitrate_kbps {
            Some(kbps) => format!("{name} {kbps}{THIN_SPACE}kbps"),
            None => format!("{name} Unknown"),
        }
    }
}

/// Sample rate in kHz without a trailing `.0` (`44.1`, `96`).
fn khz(sample_rate: u32) -> String {
    let whole = sample_rate / 1000;
    let tenth = (sample_rate % 1000) / 100;
    if tenth == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{tenth}")
    }
}

/// How the output device is opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// The app owns the device. Needed for bit-perfect output.
    Exclusive,
    /// The system mixer sits in between and may change samples.
    Shared,
    /// The mode could not be determined.
    Unknown,
}

/// A step between decoder and device that changes samples.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProcessingStage {
    /// Sample-rate conversion.
    Resample {
        /// Source rate in hertz.
        from_hz: u32,
        /// Target rate in hertz.
        to_hz: u32,
    },
    /// Software gain (the volume stage).
    Gain {
        /// Gain in decibels.
        db: f32,
    },
    /// Equalizer.
    Equalizer {
        /// Preamp as applied, in tenths of a dB.
        preamp_tenth_db: i16,
        /// Whether the preamp was lowered automatically.
        auto_preamp: bool,
    },
    /// Delay (echo).
    Delay {
        /// Delay time in ms.
        time_ms: u16,
    },
    /// Reverb.
    Reverb,
    /// Song transition. Listed while the transition mode is on.
    Crossfade {
        /// The set crossfade length in seconds.
        seconds: u8,
        /// The transition running now, if one is.
        running: Option<TransitionKind>,
    },
    /// Samples were clamped at full scale. Listed only when `samples` is above 0.
    Clipped {
        /// Clamped samples since the stream started.
        samples: u64,
    },
    /// `ReplayGain` of the playing song, from its stored values.
    ReplayGain {
        /// Gain in decibels.
        db: f32,
    },
    /// Loudness normalization.
    Normalize,
    /// Dither.
    Dither,
    /// Channel up-mix or down-mix because the device has another channel count.
    ChannelMix {
        /// Source channel count.
        from: u16,
        /// Device channel count.
        to: u16,
    },
}

/// The device end of the signal path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputInfo {
    /// Human-readable device name.
    pub device_name: String,
    /// How the device is opened.
    pub mode: OutputMode,
    /// Format negotiated with the device.
    pub format: AudioFormat,
}

/// Source -> processing -> output, reported from the real engine state only.
#[derive(Debug, Clone, PartialEq)]
pub struct SignalPath {
    /// Facts about the source stream.
    pub source: StreamInfo,
    /// Every stage that alters samples. Empty means untouched.
    pub processing: Vec<ProcessingStage>,
    /// The output device.
    pub output: OutputInfo,
}

impl SignalPath {
    /// `true` only when no stage alters samples, the device is opened exclusively and its
    /// format equals the decoded source format. Conservative on purpose: unknown means `false`.
    #[must_use]
    pub fn is_bit_perfect(&self) -> bool {
        self.processing.is_empty()
            && self.output.mode == OutputMode::Exclusive
            && self.source.format == Some(self.output.format)
    }

    /// `true` only when the true-lossless badge may show: a local lossless file (FLAC, ALAC,
    /// WAV) on a bit-perfect path. Shared mode (every `cpal` report on Linux), resampling or any
    /// effect turns it off, and a lossy or unknown codec never has it.
    #[must_use]
    pub fn is_true_lossless(&self) -> bool {
        self.source.quality_class() == QualityClass::Lossless && self.is_bit_perfect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flac_96_24() -> StreamInfo {
        StreamInfo {
            origin: Origin::Local,
            codec: Codec::Flac,
            bitrate_kbps: None,
            format: Some(AudioFormat {
                sample_rate: 96_000,
                channels: 2,
                bit_depth: Some(24),
            }),
        }
    }

    #[test]
    fn youtube_stream_is_never_lossless_even_with_a_lossless_codec() {
        for codec in [
            Codec::Flac,
            Codec::Alac,
            Codec::Pcm,
            Codec::Opus,
            Codec::Unknown,
        ] {
            let info = StreamInfo {
                origin: Origin::YoutubeMusic,
                codec,
                bitrate_kbps: Some(256),
                format: None,
            };
            assert_eq!(info.quality_class(), QualityClass::Lossy, "{codec:?}");
        }
    }

    #[test]
    fn soundcloud_stream_is_never_lossless_even_with_a_lossless_codec() {
        for codec in [
            Codec::Flac,
            Codec::Alac,
            Codec::Pcm,
            Codec::Opus,
            Codec::Unknown,
        ] {
            let info = StreamInfo {
                origin: Origin::SoundCloud,
                codec,
                bitrate_kbps: None,
                format: Some(AudioFormat {
                    sample_rate: 44_100,
                    channels: 2,
                    bit_depth: Some(16),
                }),
            };
            assert_eq!(info.quality_class(), QualityClass::Lossy, "{codec:?}");
        }
    }

    #[test]
    fn local_quality_follows_the_codec() {
        let mut info = flac_96_24();
        assert_eq!(info.quality_class(), QualityClass::Lossless);
        info.codec = Codec::Mp3;
        assert_eq!(info.quality_class(), QualityClass::Lossy);
        info.codec = Codec::Unknown;
        assert_eq!(info.quality_class(), QualityClass::Unknown);
    }

    #[test]
    fn badge_text_uses_measured_facts() {
        assert_eq!(flac_96_24().badge(), "FLAC 96\u{2009}kHz / 24-bit");
        let opus = StreamInfo {
            origin: Origin::YoutubeMusic,
            codec: Codec::Opus,
            bitrate_kbps: Some(160),
            format: None,
        };
        assert_eq!(opus.badge(), "Opus 160\u{2009}kbps");
        let mut cd = flac_96_24();
        cd.format = Some(AudioFormat {
            sample_rate: 44_100,
            channels: 2,
            bit_depth: Some(16),
        });
        assert_eq!(cd.badge(), "FLAC 44.1\u{2009}kHz / 16-bit");
    }

    #[test]
    fn badge_says_unknown_instead_of_guessing() {
        let info = StreamInfo {
            origin: Origin::YoutubeMusic,
            codec: Codec::Aac,
            bitrate_kbps: None,
            format: None,
        };
        assert_eq!(info.badge(), "AAC Unknown");
    }

    fn path(mode: OutputMode, processing: Vec<ProcessingStage>) -> SignalPath {
        let source = flac_96_24();
        let format = source.format.expect("invariant: fixture sets a format");
        SignalPath {
            source,
            processing,
            output: OutputInfo {
                device_name: "Test DAC".into(),
                mode,
                format,
            },
        }
    }

    #[test]
    fn bit_perfect_needs_exclusive_mode_and_no_processing() {
        assert!(path(OutputMode::Exclusive, vec![]).is_bit_perfect());
        assert!(!path(OutputMode::Shared, vec![]).is_bit_perfect());
        assert!(!path(OutputMode::Unknown, vec![]).is_bit_perfect());
        assert!(
            !path(
                OutputMode::Exclusive,
                vec![ProcessingStage::Gain { db: -3.0 }]
            )
            .is_bit_perfect()
        );
    }

    #[test]
    fn bit_perfect_is_false_with_each_effects_stage() {
        let stages = [
            ProcessingStage::Equalizer {
                preamp_tenth_db: 0,
                auto_preamp: false,
            },
            ProcessingStage::Delay { time_ms: 250 },
            ProcessingStage::Reverb,
            ProcessingStage::Crossfade {
                seconds: 6,
                running: None,
            },
            ProcessingStage::Clipped { samples: 1 },
            ProcessingStage::ReplayGain { db: -6.5 },
        ];
        for stage in stages {
            assert!(
                !path(OutputMode::Exclusive, vec![stage]).is_bit_perfect(),
                "{stage:?}"
            );
        }
    }

    #[test]
    fn bit_perfect_is_false_when_device_format_differs() {
        let mut p = path(OutputMode::Exclusive, vec![]);
        p.output.format.sample_rate = 48_000;
        assert!(!p.is_bit_perfect());
    }

    #[test]
    fn shared_mode_is_never_bit_perfect_even_with_an_equal_format() {
        // `cpal` reports every Linux output as shared, so the badge stays off there.
        let p = path(OutputMode::Shared, vec![]);
        assert_eq!(p.source.format, Some(p.output.format));
        assert!(!p.is_bit_perfect());
        assert!(!p.is_true_lossless());
    }

    /// The true-lossless badge per codec and origin, on a bit-perfect and a shared path.
    #[test]
    fn true_lossless_only_for_local_lossless_files_on_a_bit_perfect_path() {
        let table = [
            (Origin::Local, Codec::Flac, true),
            (Origin::Local, Codec::Alac, true),
            (Origin::Local, Codec::Pcm, true),
            (Origin::Local, Codec::Opus, false),
            (Origin::Local, Codec::Vorbis, false),
            (Origin::Local, Codec::Mp3, false),
            (Origin::Local, Codec::Aac, false),
            (Origin::Local, Codec::Unknown, false),
            (Origin::YoutubeMusic, Codec::Flac, false),
            (Origin::YoutubeMusic, Codec::Opus, false),
            (Origin::SoundCloud, Codec::Pcm, false),
        ];
        for (origin, codec, lossless) in table {
            let mut exclusive = path(OutputMode::Exclusive, vec![]);
            exclusive.source.origin = origin;
            exclusive.source.codec = codec;
            assert_eq!(
                exclusive.is_true_lossless(),
                lossless,
                "{origin:?} {codec:?}"
            );
            let mut shared = exclusive.clone();
            shared.output.mode = OutputMode::Shared;
            assert!(!shared.is_true_lossless(), "shared {origin:?} {codec:?}");
            let mut resampled = exclusive.clone();
            resampled.processing.push(ProcessingStage::Resample {
                from_hz: 96_000,
                to_hz: 48_000,
            });
            assert!(!resampled.is_true_lossless(), "resampled {codec:?}");
        }
    }

    #[test]
    fn lossy_badges_never_say_lossless() {
        for codec in [Codec::Opus, Codec::Vorbis, Codec::Mp3, Codec::Aac] {
            let mut info = flac_96_24();
            info.codec = codec;
            info.bitrate_kbps = None;
            assert_eq!(info.badge(), format!("{} Unknown", codec.name()));
            assert!(!info.badge().contains("ossless"));
        }
        let mut unknown = flac_96_24();
        unknown.format = None;
        assert_eq!(unknown.badge(), "FLAC Unknown");
    }
}
