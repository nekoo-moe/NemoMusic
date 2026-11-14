//! Local library types: CUE spans, `ReplayGain` values, the loudness meter, and the library
//! service the UI uses (folders, scan, track details, local playlists, M3U, tag edits).
//!
//! `nemo-library` implements [`LocalTrackLookup`] and [`LocalLibraryService`]; `nemo-audio`
//! calls the lookup while preparing a song and implements [`LoudnessMeter`]. No I/O happens in
//! this module.

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use crate::track::Track;

mod service;

pub use service::{
    LibraryError, LocalLibraryService, LocalPlaylist, LocalPlaylistId, LocalTrackDetails,
    M3uImport, PlaylistEntry, ScanEvent, ScanReceiver, ScanSummary, TagDiff, TagEdit, TrackStatus,
    UnplayableReason,
};

/// A time span of one audio file, such as one track of a CUE sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TrackSpan {
    /// Where the track starts, from the start of the file.
    pub start: Duration,
    /// Where the track ends, from the start of the file. `None` plays to the end of the file.
    pub end: Option<Duration>,
}

/// Which `ReplayGain` value the player applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReplayGainMode {
    /// Each song at its own loudness.
    Track,
    /// Every song of an album at the album's loudness.
    Album,
}

/// `ReplayGain` or R128 values read from a file's tags or measured by the loudness scan.
///
/// The default has no values, which means unity gain.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ReplayGainValues {
    /// Track gain in decibels.
    pub track_gain_db: Option<f32>,
    /// Album gain in decibels.
    pub album_gain_db: Option<f32>,
    /// Track sample peak, where 1.0 is full scale.
    pub track_peak: Option<f32>,
    /// Album sample peak, where 1.0 is full scale.
    pub album_peak: Option<f32>,
}

/// What the player needs to know about a local file before it plays.
///
/// The default has no span (the whole file) and no gain (unity).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LocalPlayback {
    /// The part of the file to play. `None` plays the whole file.
    pub span: Option<TrackSpan>,
    /// Stored gain values.
    pub gain: ReplayGainValues,
}

/// Looks up the playback facts of a local track.
///
/// Prepare thread only, never the audio callback: an implementation may block on its
/// database.
pub trait LocalTrackLookup: Send + Sync {
    /// The span and gain of `track`, or `None` when the library does not know it.
    fn playback(&self, track: &Track) -> Option<LocalPlayback>;
}

/// The measured loudness of one file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Loudness {
    /// Integrated loudness in LUFS.
    pub integrated_lufs: f32,
    /// True peak, where 1.0 is full scale.
    pub true_peak: f32,
}

/// Measuring a file's loudness failed. Messages hold no path or tag.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LoudnessError {
    /// The cancel flag was set.
    #[error("the loudness scan was stopped")]
    Cancelled,
    /// The file could not be opened or read.
    #[error("the file could not be read")]
    Unreadable,
    /// The file could not be decoded.
    #[error("the file could not be decoded")]
    Decode,
    /// The meter rejected the audio, for example an unsupported channel layout.
    #[error("the loudness could not be measured")]
    Measure,
}

/// Measures the loudness of a file, for the opt-in scan of untagged files.
///
/// Runs on a low-priority worker thread, never the audio callback.
pub trait LoudnessMeter: Send + Sync {
    /// Decode the file at `path` and measure it. Checks `cancel` between blocks.
    ///
    /// # Errors
    /// Returns [`LoudnessError::Cancelled`] when `cancel` is set, and another
    /// [`LoudnessError`] when the file cannot be read, decoded or measured.
    fn measure(&self, path: &Path, cancel: &AtomicBool) -> Result<Loudness, LoudnessError>;
}

#[cfg(test)]
mod tests {
    use super::{
        LibraryError, LocalPlayback, LoudnessError, ReplayGainValues, ScanSummary, TagEdit,
        TrackSpan, TrackStatus,
    };

    #[test]
    fn default_playback_has_no_span_and_no_gain() {
        let playback = LocalPlayback::default();
        assert_eq!(playback.span, None);
        assert_eq!(
            playback.gain,
            ReplayGainValues {
                track_gain_db: None,
                album_gain_db: None,
                track_peak: None,
                album_peak: None,
            }
        );
    }

    #[test]
    fn default_span_is_the_whole_file() {
        let span = TrackSpan::default();
        assert_eq!(span.start, std::time::Duration::ZERO);
        assert_eq!(span.end, None);
    }

    #[test]
    fn loudness_errors_hold_no_path() {
        for err in [
            LoudnessError::Cancelled,
            LoudnessError::Unreadable,
            LoudnessError::Decode,
            LoudnessError::Measure,
        ] {
            let text = err.to_string();
            assert!(!text.contains('/') && !text.contains('\\'), "{text}");
        }
    }

    #[test]
    fn library_errors_hold_no_path() {
        for err in [
            LibraryError::Db,
            LibraryError::Io(std::io::ErrorKind::NotFound),
            LibraryError::Tag("the file is read-only"),
            LibraryError::Cue,
            LibraryError::M3u,
            LibraryError::Corrupt,
            LibraryError::Busy,
        ] {
            let text = err.to_string();
            assert!(!text.contains('/') && !text.contains('\\'), "{text}");
        }
    }

    #[test]
    fn defaults_are_playable_and_empty() {
        assert_eq!(TrackStatus::default(), TrackStatus::Playable);
        assert_eq!(TagEdit::default().title, None);
        assert_eq!(ScanSummary::default().added, 0);
    }
}
