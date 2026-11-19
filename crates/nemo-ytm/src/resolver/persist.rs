//! Resolved streams that survive a restart (cold-start spec).
//!
//! A song played in the last hours starts at once after the app is opened again: its resolved
//! stream, with the time it expires, is kept in one private file (`cache/streams.json` in the
//! app folder; mode 0600 on Unix, the per-user data folder on Windows). The idea is `SimpMusic`'s
//! persisted format cache (read 2026-11-19, no code taken).
//!
//! - Only the automatic request is kept (with the tracks the source listed), a plain file, not a
//!   preview.
//! - An entry is used only while it is valid for at least [`EXPIRY_MARGIN`](super::EXPIRY_MARGIN)
//!   more, and only after [`UrlCheck`] says the address still answers (one range request of one
//!   byte). A refused address, or a stream that could not be opened, drops the entry.
//! - Stream addresses can carry the listener's IP and a signature: they are never logged, and
//!   the file is deleted at sign-out ([`SignOutWipe`]).
//! - A damaged or unknown file is the same as no file.

use std::collections::HashMap;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nemo_core::account::{SignOutWipe, WipeError, WipeScope};
use nemo_core::audio::{AudioFormat, Codec, Origin, StreamInfo};
use nemo_core::secret::Secret;
use nemo_core::stream::{AudioTrackInfo, ResolvedStream, Transport};
use nemo_core::track::VideoId;
use serde::{Deserialize, Serialize};

use super::{EXPIRY_MARGIN, MAX_CACHED};

/// Format version of the file. A file with another number is ignored.
const VERSION: u32 = 1;
/// Name of the sign-out step, shown when it fails.
pub const SIGN_OUT_STEP: &str = "streams";

/// One kept stream. Plain numbers and text, so the file does not depend on the engine's types.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Stored {
    url: String,
    /// Seconds since the Unix epoch.
    expires: u64,
    codec: String,
    kbps: Option<u32>,
    rate: Option<u32>,
    channels: Option<u16>,
    depth: Option<u8>,
    /// The language of the track that plays, when the source says.
    #[serde(default)]
    language: Option<String>,
    /// The audio tracks of a video that has several.
    #[serde(default)]
    tracks: Vec<StoredTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredTrack {
    language: Option<String>,
    label: String,
    default: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct File {
    version: u32,
    streams: HashMap<String, Stored>,
}

fn codec_name(codec: Codec) -> &'static str {
    match codec {
        Codec::Flac => "flac",
        Codec::Alac => "alac",
        Codec::Pcm => "pcm",
        Codec::Aac => "aac",
        Codec::Opus => "opus",
        Codec::Mp3 => "mp3",
        Codec::Vorbis => "vorbis",
        Codec::Unknown => "unknown",
    }
}

fn codec_of(name: &str) -> Codec {
    [
        Codec::Flac,
        Codec::Alac,
        Codec::Pcm,
        Codec::Aac,
        Codec::Opus,
        Codec::Mp3,
        Codec::Vorbis,
    ]
    .into_iter()
    .find(|c| codec_name(*c) == name)
    .unwrap_or(Codec::Unknown)
}

fn secs(at: SystemTime) -> Option<u64> {
    at.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

impl Stored {
    /// What to keep of `stream`; `None` for anything the file does not hold.
    fn of(stream: &ResolvedStream) -> Option<Self> {
        let plain = stream.transport == Transport::File
            && !stream.preview
            && stream.info.origin == Origin::YoutubeMusic;
        let format = stream.info.format;
        plain.then_some(())?;
        Some(Self {
            url: stream.url.expose().clone(),
            expires: secs(stream.expires_at?)?,
            codec: codec_name(stream.info.codec).to_owned(),
            kbps: stream.info.bitrate_kbps,
            rate: format.map(|f| f.sample_rate),
            channels: format.map(|f| f.channels),
            depth: format.and_then(|f| f.bit_depth),
            language: stream.audio_language.clone(),
            tracks: stream
                .audio_tracks
                .iter()
                .map(|t| StoredTrack {
                    language: t.language.clone(),
                    label: t.label.clone(),
                    default: t.is_default,
                })
                .collect(),
        })
    }

    fn usable(&self, now: SystemTime) -> bool {
        UNIX_EPOCH
            .checked_add(Duration::from_secs(self.expires))
            .and_then(|at| at.duration_since(now).ok())
            .is_some_and(|left| left > EXPIRY_MARGIN)
    }

    fn stream(&self) -> ResolvedStream {
        let format = self
            .rate
            .zip(self.channels)
            .map(|(sample_rate, channels)| AudioFormat {
                sample_rate,
                channels,
                bit_depth: self.depth,
            });
        ResolvedStream {
            url: Secret::new(self.url.clone()),
            info: StreamInfo {
                origin: Origin::YoutubeMusic,
                codec: codec_of(&self.codec),
                bitrate_kbps: self.kbps,
                format,
            },
            expires_at: UNIX_EPOCH.checked_add(Duration::from_secs(self.expires)),
            transport: Transport::File,
            preview: false,
            audio_language: self.language.clone(),
            audio_tracks: self
                .tracks
                .iter()
                .map(|t| AudioTrackInfo {
                    language: t.language.clone(),
                    label: t.label.clone(),
                    is_default: t.default,
                })
                .collect(),
        }
    }
}

/// The file of kept streams.
#[derive(Debug)]
pub struct StreamStore {
    path: PathBuf,
    entries: Mutex<HashMap<String, Stored>>,
    /// Held while the file is written, so two writes never mix.
    writing: Mutex<()>,
}

impl StreamStore {
    /// Open the store at `path`: the entries that are still valid, or none when the file is
    /// missing, damaged or of another version. Never fails.
    #[must_use]
    pub fn open(path: PathBuf) -> Self {
        let now = SystemTime::now();
        let mut entries = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<File>(&text).ok())
            .filter(|file| file.version == VERSION)
            .map(|file| file.streams)
            .unwrap_or_default();
        entries.retain(|id, stored| VideoId::parse(id).is_ok() && stored.usable(now));
        Self {
            path,
            entries: Mutex::new(entries),
            writing: Mutex::new(()),
        }
    }

    /// The kept stream of `video` while it is valid for at least the expiry margin.
    #[must_use]
    pub fn get(&self, video: &VideoId) -> Option<ResolvedStream> {
        let entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        let stored = entries.get(video.as_str())?;
        stored.usable(SystemTime::now()).then(|| stored.stream())
    }

    /// Keep `stream` for `video`. `false` when the file does not hold such a stream (another
    /// language, a preview, no expiry); nothing changes then. Call [`flush`](Self::flush) to
    /// write the file.
    pub fn put(&self, video: &VideoId, stream: &ResolvedStream) -> bool {
        let Some(stored) = Stored::of(stream).filter(|s| s.usable(SystemTime::now())) else {
            return false;
        };
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        if entries.len() >= MAX_CACHED && !entries.contains_key(video.as_str()) {
            // Make room: drop the entry that expires first.
            let oldest = entries
                .iter()
                .min_by_key(|(_, s)| s.expires)
                .map(|(id, _)| id.clone());
            if let Some(id) = oldest {
                entries.remove(&id);
            }
        }
        entries.insert(video.as_str().to_owned(), stored);
        true
    }

    /// Drop the entry of `video` (its address was refused or could not be opened).
    pub fn forget(&self, video: &VideoId) {
        let removed = self
            .entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(video.as_str())
            .is_some();
        if removed {
            self.flush();
        }
    }

    /// Number of kept entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// `true` when nothing is kept.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Write the file (temporary file, then rename). A failure is logged without a path and
    /// the next write tries again.
    pub fn flush(&self) {
        let _writing = self.writing.lock().unwrap_or_else(PoisonError::into_inner);
        let streams = self
            .entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let file = File {
            version: VERSION,
            streams,
        };
        if write_private(&self.path, &file).is_err() {
            tracing::warn!("the stream cache file could not be written");
        }
    }

    /// Forget everything and delete the file. Returns how many entries were dropped.
    ///
    /// # Errors
    /// The error of deleting the file, unless it was not there.
    pub fn wipe(&self) -> std::io::Result<usize> {
        let _writing = self.writing.lock().unwrap_or_else(PoisonError::into_inner);
        let dropped = {
            let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
            let n = entries.len();
            entries.clear();
            n
        };
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(dropped.max(1)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(dropped),
            Err(e) => Err(e),
        }
    }
}

/// Write `file` to `path` through a temporary file in the same folder, readable by the owner
/// only (mode 0600 on Unix).
fn write_private(path: &std::path::Path, file: &File) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut out = options.open(&tmp)?;
    let text = serde_json::to_vec(file).map_err(std::io::Error::other)?;
    out.write_all(&text)?;
    out.sync_data()?;
    drop(out);
    std::fs::rename(&tmp, path)
}

/// Sign-out deletes the kept streams, whichever account signs out.
impl SignOutWipe for StreamStore {
    fn name(&self) -> &'static str {
        SIGN_OUT_STEP
    }

    fn wipe(&self, _scope: WipeScope) -> Result<usize, WipeError> {
        Self::wipe(self).map_err(|_| WipeError("the stream cache could not be deleted"))
    }
}

#[cfg(test)]
mod tests;
