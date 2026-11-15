//! Equalizer presets: the built-in ones and the user's own, kept as `AutoEq` text files.
//!
//! The built-in presets are constants. The user's presets and imported `AutoEq` profiles are
//! plain text files in the format of [`write_autoeq`], one file `<name>.txt` per preset, in a
//! folder that `nemo-app` passes in. Everything here runs off the real-time path, does file
//! I/O only in that folder or on a path the user chose, and contacts no network host.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

use nemo_core::effects::{EQ_MAX_FILTERS, EqFilter, EqSettings};

use super::biquad::ten_band;
use super::profile::{EqProfile, ProfileError, parse_autoeq, write_autoeq};

/// The most bytes read from one preset or profile file: 64 KiB.
pub const PRESET_FILE_MAX_BYTES: u64 = 64 * 1024;

/// The longest preset name, in characters.
const NAME_MAX_CHARS: usize = 100;
/// The file extension of a preset file.
const EXTENSION: &str = "txt";
/// Characters a preset name may not hold, because it becomes a file name on every OS.
const NAME_FORBIDDEN: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
/// Windows device names that cannot be file names, in any letter case.
const NAME_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// A built-in ten-band preset: a name and one gain per band of
/// [`TEN_BAND_HZ`](super::TEN_BAND_HZ). Its preamp is 0 dB; the automatic preamp lowers it
/// when the boost could clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BuiltinPreset {
    /// The name shown to the user.
    pub name: &'static str,
    /// The gain of each band, 31 Hz to 16 kHz, in tenths of a dB.
    pub gains_tenth_db: [i16; 10],
}

impl BuiltinPreset {
    /// Every band and the preamp at 0 dB (criterion 9).
    pub const FLAT: Self = Self {
        name: "Flat",
        gains_tenth_db: [0; 10],
    };
    /// More low end: +6 dB at 31 Hz, falling to 0 dB at 500 Hz.
    pub const BASS_BOOST: Self = Self {
        name: "Bass boost",
        gains_tenth_db: [60, 50, 40, 20, 0, 0, 0, 0, 0, 0],
    };
    /// More air: 0 dB up to 1 kHz, rising to +6 dB at 16 kHz.
    pub const TREBLE_BOOST: Self = Self {
        name: "Treble boost",
        gains_tenth_db: [0, 0, 0, 0, 0, 0, 20, 40, 50, 60],
    };
    /// Voices forward: the lowest bands cut, +3 to +4 dB from 500 Hz to 2 kHz.
    pub const VOCAL: Self = Self {
        name: "Vocal",
        gains_tenth_db: [-20, -10, 0, 10, 30, 40, 40, 20, 0, -10],
    };

    /// The equalizer filters of this preset: ten peaks in the first ten slots.
    #[must_use]
    pub fn filters(self) -> [Option<EqFilter>; EQ_MAX_FILTERS] {
        ten_band(self.gains_tenth_db)
    }

    /// `settings` with this preset's filters and a 0 dB preamp. Whether the equalizer is on
    /// stays as it was.
    #[must_use]
    pub fn apply(self, settings: EqSettings) -> EqSettings {
        EqSettings {
            preamp_tenth_db: 0,
            filters: self.filters(),
            ..settings
        }
    }

    /// This preset as a profile, for example to export it as a file.
    #[must_use]
    pub fn profile(self) -> EqProfile {
        EqProfile {
            name: self.name.to_owned(),
            preamp_tenth_db: 0,
            filters: self.filters().into_iter().flatten().collect(),
        }
    }
}

/// The built-in presets in the order the UI lists them.
pub const BUILTIN_PRESETS: [BuiltinPreset; 4] = [
    BuiltinPreset::FLAT,
    BuiltinPreset::BASS_BOOST,
    BuiltinPreset::TREBLE_BOOST,
    BuiltinPreset::VOCAL,
];

/// Why a preset action failed. Nothing is changed when it fails.
#[derive(Debug, thiserror::Error)]
pub enum PresetError {
    /// A file could not be read or written, is larger than [`PRESET_FILE_MAX_BYTES`], is not
    /// UTF-8 text, or the name cannot be a file name (`InvalidInput`).
    #[error("preset file error: {0}")]
    Io(#[from] io::Error),
    /// A preset with this name exists already.
    #[error("a preset with this name exists already")]
    NameTaken,
    /// The file is not a profile the equalizer can use.
    #[error(transparent)]
    Profile(#[from] ProfileError),
}

/// The user's presets: one `AutoEq` text file per preset in one folder. Off the real-time path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetStore {
    folder: PathBuf,
}

impl PresetStore {
    /// A store in `folder`. The folder is created on the first save or import.
    #[must_use]
    pub fn new(folder: PathBuf) -> Self {
        Self { folder }
    }

    /// The folder that holds the preset files.
    #[must_use]
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// The names of the saved presets, sorted without regard to letter case. A missing folder
    /// is an empty list; files that are not `.txt` or whose name is not a valid preset name are
    /// left out.
    ///
    /// # Errors
    /// [`PresetError::Io`] when the folder cannot be read.
    pub fn list(&self) -> Result<Vec<String>, PresetError> {
        let entries = match fs::read_dir(&self.folder) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut names = Vec::new();
        for entry in entries {
            let path = entry?.path();
            if !path.is_file() || !has_preset_extension(&path) {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str());
            if let Some(name) = stem.filter(|n| check_name(n).is_ok()) {
                names.push(name.to_owned());
            }
        }
        names.sort_by_key(|n| n.to_lowercase());
        Ok(names)
    }

    /// Read the preset `name`.
    ///
    /// # Errors
    /// [`PresetError::Io`] when the name is not valid or the file cannot be read, and
    /// [`PresetError::Profile`] when its text is refused.
    pub fn load(&self, name: &str) -> Result<EqProfile, PresetError> {
        let text = read_capped(&self.path_of(name)?)?;
        Ok(parse_autoeq(name, &text)?)
    }

    /// Save `profile` as a new preset named after `profile.name`.
    ///
    /// # Errors
    /// [`PresetError::NameTaken`] when that preset exists, and [`PresetError::Io`] when the
    /// name is not valid or the file cannot be written.
    pub fn save(&self, profile: &EqProfile) -> Result<(), PresetError> {
        let path = self.path_of(&profile.name)?;
        fs::create_dir_all(&self.folder)?;
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                return Err(PresetError::NameTaken);
            }
            Err(e) => return Err(e.into()),
        };
        file.write_all(write_autoeq(profile).as_bytes())?;
        Ok(file.sync_all()?)
    }

    /// Rename the preset `from` to `to`.
    ///
    /// # Errors
    /// [`PresetError::NameTaken`] when a preset `to` exists, and [`PresetError::Io`] when a
    /// name is not valid or `from` cannot be renamed (for example because it does not exist).
    pub fn rename(&self, from: &str, to: &str) -> Result<(), PresetError> {
        let (old, new) = (self.path_of(from)?, self.path_of(to)?);
        if new.exists() {
            return Err(PresetError::NameTaken);
        }
        Ok(fs::rename(old, new)?)
    }

    /// Delete the preset `name`.
    ///
    /// # Errors
    /// [`PresetError::Io`] when the name is not valid or the file cannot be removed.
    pub fn delete(&self, name: &str) -> Result<(), PresetError> {
        Ok(fs::remove_file(self.path_of(name)?)?)
    }

    /// Import the `AutoEq` profile file at `path`, which the user chose, as a new preset named
    /// after the file name without its extension. Returns the profile that was saved.
    ///
    /// # Errors
    /// [`PresetError::Profile`] when the text is refused (nothing is saved),
    /// [`PresetError::NameTaken`] when a preset of that name exists, and [`PresetError::Io`]
    /// when the file cannot be read, is too large, or its name cannot be a preset name.
    pub fn import(&self, path: &Path) -> Result<EqProfile, PresetError> {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| invalid_name("the file name is not valid text"))?;
        check_name(name)?;
        let profile = parse_autoeq(name, &read_capped(path)?)?;
        self.save(&profile)?;
        Ok(profile)
    }

    /// Write the preset `name` to `path`, which the user chose, in the `AutoEq` text format.
    /// An existing file at `path` is replaced.
    ///
    /// # Errors
    /// [`PresetError::Io`] or [`PresetError::Profile`] as [`load`](Self::load) gives them, and
    /// [`PresetError::Io`] when `path` cannot be written.
    pub fn export(&self, name: &str, path: &Path) -> Result<(), PresetError> {
        let profile = self.load(name)?;
        Ok(fs::write(path, write_autoeq(&profile))?)
    }

    /// The file of the preset `name`, after the name is checked.
    fn path_of(&self, name: &str) -> Result<PathBuf, PresetError> {
        check_name(name)?;
        Ok(self.folder.join(format!("{name}.{EXTENSION}")))
    }
}

/// `true` when `path` ends in `.txt`, in any letter case.
fn has_preset_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION))
}

/// Refuse a name that cannot be one file name in the store folder on Windows and Linux.
fn check_name(name: &str) -> Result<(), PresetError> {
    let chars = name.chars().count();
    if chars == 0 || chars > NAME_MAX_CHARS {
        return Err(invalid_name("a preset name has 1 to 100 characters"));
    }
    if name.trim() != name || name.starts_with('.') || name.ends_with('.') {
        return Err(invalid_name(
            "a preset name cannot start or end with a space or a dot",
        ));
    }
    if name
        .chars()
        .any(|c| c.is_control() || NAME_FORBIDDEN.contains(&c))
    {
        return Err(invalid_name(
            "a preset name cannot hold / \\ : * ? \" < > | or control characters",
        ));
    }
    let base = name.split('.').next().unwrap_or(name);
    if NAME_RESERVED.iter().any(|r| r.eq_ignore_ascii_case(base)) {
        return Err(invalid_name("this preset name is reserved by Windows"));
    }
    Ok(())
}

fn invalid_name(why: &'static str) -> PresetError {
    PresetError::Io(io::Error::new(io::ErrorKind::InvalidInput, why))
}

/// The text of `path`, refused when it is larger than [`PRESET_FILE_MAX_BYTES`] or not UTF-8.
fn read_capped(path: &Path) -> Result<String, PresetError> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(PRESET_FILE_MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > PRESET_FILE_MAX_BYTES {
        return Err(PresetError::Io(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "a preset file holds at most 64 KiB",
        )));
    }
    String::from_utf8(bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "the file is not UTF-8 text"))
        .map_err(PresetError::Io)
}

#[cfg(test)]
mod tests;
