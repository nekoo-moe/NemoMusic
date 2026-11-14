//! Text for the signal path, built only from the real [`SignalPath`] of the player snapshot
//! (`design.md` section 13). Pure. A lossy source never reads "Lossless"; "Bit-perfect" appears
//! only when the path really is.

use nemo_core::audio::{
    AudioFormat, Origin, OutputMode, ProcessingStage, QualityClass, SignalPath,
};

use std::fmt::Write as _;

use crate::format::THIN_SPACE;

/// One labelled line of the signal-path view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathLine {
    /// `Source`, `Decode`, `Process` or `Output`.
    pub label: &'static str,
    /// What the engine reports.
    pub value: String,
}

/// Sample rate in kHz without a trailing `.0`: `44.1`, `48`.
#[must_use]
pub fn khz(hz: u32) -> String {
    let (whole, tenth) = (hz / 1000, (hz % 1000) / 100);
    if tenth == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{tenth}")
    }
}

fn format_text(f: AudioFormat) -> String {
    let channels = match f.channels {
        1 => "mono".to_owned(),
        2 => "stereo".to_owned(),
        n => format!("{n}{THIN_SPACE}channels"),
    };
    let mut s = format!("{}{THIN_SPACE}kHz \u{b7} {channels}", khz(f.sample_rate));
    if let Some(bits) = f.bit_depth {
        let _ = write!(s, " \u{b7} {bits}-bit");
    }
    s
}

fn stage_text(stage: &ProcessingStage) -> String {
    match stage {
        ProcessingStage::Resample { from_hz, to_hz } => format!(
            "Resample {}\u{2009}\u{2192}\u{2009}{}{THIN_SPACE}kHz",
            khz(*from_hz),
            khz(*to_hz)
        ),
        ProcessingStage::Gain { db } => format!("Gain {db:+.1}{THIN_SPACE}dB"),
        ProcessingStage::Equalizer { .. } => "Equalizer".to_owned(),
        ProcessingStage::Delay { .. } => "Delay".to_owned(),
        ProcessingStage::Reverb => "Reverb".to_owned(),
        ProcessingStage::Crossfade { .. } => "Crossfade".to_owned(),
        ProcessingStage::Clipped { .. } => "Clipping".to_owned(),
        ProcessingStage::ReplayGain { .. } => "ReplayGain".to_owned(),
        ProcessingStage::Normalize => "Normalize".to_owned(),
        ProcessingStage::Dither => "Dither".to_owned(),
        ProcessingStage::ChannelMix { from, to } => {
            format!("Channel mix {from}\u{2009}\u{2192}\u{2009}{to}")
        }
    }
}

/// `Lossless`, `Lossy` or `Unknown`, by the honest-label rule of `nemo-core`.
#[must_use]
pub fn quality_word(path: &SignalPath) -> &'static str {
    match path.source.quality_class() {
        QualityClass::Lossless => "Lossless",
        QualityClass::Lossy => "Lossy",
        QualityClass::Unknown => "Unknown",
    }
}

/// Why the path is not bit-perfect (empty when it is).
#[must_use]
pub fn not_bit_perfect_because(path: &SignalPath) -> Vec<String> {
    let mut why = Vec::new();
    match path.output.mode {
        OutputMode::Exclusive => {}
        OutputMode::Shared => why.push("shared mode".to_owned()),
        OutputMode::Unknown => why.push("unknown output mode".to_owned()),
    }
    why.extend(path.processing.iter().map(|s| {
        // Lower-case the first letter only: units such as kHz keep their case.
        let text = stage_text(s);
        let mut chars = text.chars();
        chars
            .next()
            .map_or_else(String::new, |c| c.to_lowercase().chain(chars).collect())
    }));
    if path.source.format != Some(path.output.format) && path.processing.is_empty() {
        why.push("device format differs from the source".to_owned());
    }
    if path.is_bit_perfect() {
        Vec::new()
    } else {
        why
    }
}

/// The four lines `Source`, `Decode`, `Process`, `Output`, then `Result`.
#[must_use]
pub fn lines(path: &SignalPath) -> Vec<PathLine> {
    let origin = match path.source.origin {
        Origin::Local => "local file",
        Origin::YoutubeMusic => "YouTube Music",
        Origin::SoundCloud => "SoundCloud",
    };
    let process = if path.processing.is_empty() {
        "None (samples untouched)".to_owned()
    } else {
        path.processing
            .iter()
            .map(stage_text)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mode = match path.output.mode {
        OutputMode::Exclusive => "Exclusive mode",
        OutputMode::Shared => "Shared mode",
        OutputMode::Unknown => "Unknown mode",
    };
    let result = if path.is_bit_perfect() {
        format!("{} \u{b7} Bit-perfect", quality_word(path))
    } else {
        format!(
            "{} \u{b7} Not bit-perfect ({})",
            quality_word(path),
            not_bit_perfect_because(path).join(", ")
        )
    };
    vec![
        PathLine {
            label: "Source",
            value: format!("{} \u{b7} {origin}", path.source.badge()),
        },
        PathLine {
            label: "Decode",
            value: path
                .source
                .format
                .as_ref()
                .map_or_else(|| "Unknown".to_owned(), |f| format_text(*f)),
        },
        PathLine {
            label: "Process",
            value: process,
        },
        PathLine {
            label: "Output",
            value: format!(
                "{} \u{b7} {mode} \u{b7} {}",
                path.output.device_name,
                format_text(path.output.format)
            ),
        },
        PathLine {
            label: "Result",
            value: result,
        },
    ]
}

/// Short text for the badge in the player bar: `AAC 129 kbps · Lossy · Resample`.
#[must_use]
pub fn badge_label(path: Option<&SignalPath>) -> String {
    let Some(path) = path else {
        return "Unknown".to_owned();
    };
    let mut s = format!("{} \u{b7} {}", path.source.badge(), quality_word(path));
    // A bit-perfect path says so in the popover (Result line); the badge stays short.
    if !path.is_bit_perfect()
        && path
            .processing
            .iter()
            .any(|p| matches!(p, ProcessingStage::Resample { .. }))
    {
        s.push_str(" \u{b7} Resample");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use nemo_core::audio::{Codec, OutputInfo, StreamInfo};

    /// The real case on the dev machine: AAC 129 kbps from YouTube through a 48 kHz shared device.
    fn youtube_aac() -> SignalPath {
        SignalPath {
            source: StreamInfo {
                origin: Origin::YoutubeMusic,
                codec: Codec::Aac,
                bitrate_kbps: Some(129),
                format: Some(AudioFormat {
                    sample_rate: 44_100,
                    channels: 2,
                    bit_depth: None,
                }),
            },
            processing: vec![ProcessingStage::Resample {
                from_hz: 44_100,
                to_hz: 48_000,
            }],
            output: OutputInfo {
                device_name: "Speakers (Realtek)".to_owned(),
                mode: OutputMode::Shared,
                format: AudioFormat {
                    sample_rate: 48_000,
                    channels: 2,
                    bit_depth: Some(32),
                },
            },
        }
    }

    #[test]
    fn youtube_aac_shows_resample_lossy_and_not_bit_perfect() {
        let p = youtube_aac();
        assert_eq!(
            badge_label(Some(&p)),
            "AAC 129\u{2009}kbps \u{b7} Lossy \u{b7} Resample"
        );
        let lines = lines(&p);
        assert_eq!(lines[0].value, "AAC 129\u{2009}kbps \u{b7} YouTube Music");
        assert_eq!(
            lines[2].value,
            "Resample 44.1\u{2009}\u{2192}\u{2009}48\u{2009}kHz"
        );
        assert!(lines[3].value.contains("Shared mode"));
        assert!(lines[4].value.starts_with("Lossy \u{b7} Not bit-perfect"));
        assert!(!lines.iter().any(|l| l.value.contains("Lossless")));
    }

    #[test]
    fn local_flac_in_exclusive_mode_is_lossless_and_bit_perfect() {
        let format = AudioFormat {
            sample_rate: 96_000,
            channels: 2,
            bit_depth: Some(24),
        };
        let p = SignalPath {
            source: StreamInfo {
                origin: Origin::Local,
                codec: Codec::Flac,
                bitrate_kbps: None,
                format: Some(format),
            },
            processing: vec![],
            output: OutputInfo {
                device_name: "DAC".to_owned(),
                mode: OutputMode::Exclusive,
                format,
            },
        };
        assert_eq!(
            badge_label(Some(&p)),
            "FLAC 96\u{2009}kHz / 24-bit \u{b7} Lossless"
        );
        assert_eq!(lines(&p)[4].value, "Lossless \u{b7} Bit-perfect");
    }

    #[test]
    fn local_flac_through_the_shared_mixer_is_lossless_but_not_bit_perfect() {
        let mut p = youtube_aac();
        p.source.origin = Origin::Local;
        p.source.codec = Codec::Flac;
        p.source.bitrate_kbps = None;
        assert!(badge_label(Some(&p)).contains("Lossless"));
        assert!(badge_label(Some(&p)).ends_with("Resample"));
        assert!(lines(&p)[4].value.contains("Not bit-perfect"));
    }

    #[test]
    fn no_path_is_unknown() {
        assert_eq!(badge_label(None), "Unknown");
        assert_eq!(khz(44_100), "44.1");
        assert_eq!(khz(48_000), "48");
    }
}
