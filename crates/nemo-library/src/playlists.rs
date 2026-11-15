//! Local playlists as the library service edits them (criteria 18 and 21).
//!
//! Every change goes through the one write connection ([`Store`]) and is saved at once, in
//! one transaction: create, rename, move, remove and delete are [`Store`] methods, and this
//! module turns the service's tracks and files into entries. A local track is kept as a file
//! entry, by path, so it survives rescans and shows as missing while its file is gone; a
//! streamed song (playlist-import, a stats recap) is kept whole. No file is ever touched.

use std::path::PathBuf;

use nemo_core::audio::Origin;
use nemo_core::library::{LibraryError, LocalPlaylistId};
use nemo_core::track::Track;

use crate::store::{NewEntry, Store};

/// The entry a track becomes: a file entry for a local track with a path, otherwise the
/// streamed song itself.
#[must_use]
pub fn entry_for(track: &Track) -> NewEntry {
    match (track.origin, &track.local_path) {
        (Origin::Local, Some(path)) => NewEntry::File {
            path: path.clone(),
            cue_index: 0,
        },
        _ => NewEntry::Stream(track.clone()),
    }
}

/// Add tracks to the end of a playlist in one transaction (playlist-import,
/// listening-stats recaps). Returns how many were added.
///
/// # Errors
/// [`LibraryError::Db`] when the playlist does not exist or the write fails (nothing is
/// added), or [`LibraryError::Io`] for a path that is not valid Unicode.
pub fn add_tracks(
    store: &mut Store,
    id: LocalPlaylistId,
    tracks: &[Track],
) -> Result<usize, LibraryError> {
    let entries: Vec<NewEntry> = tracks.iter().map(entry_for).collect();
    store.add_entries(id, &entries)
}

/// Add local files (whole files, not CUE tracks) to the end of a playlist in one
/// transaction. A file outside the watched folders is kept too. Returns how many were added.
///
/// # Errors
/// [`LibraryError::Db`] when the playlist does not exist or the write fails (nothing is
/// added), or [`LibraryError::Io`] for a path that is not valid Unicode.
pub fn add_files(
    store: &mut Store,
    id: LocalPlaylistId,
    paths: &[PathBuf],
) -> Result<usize, LibraryError> {
    let entries: Vec<NewEntry> = paths
        .iter()
        .map(|path| NewEntry::File {
            path: path.clone(),
            cue_index: 0,
        })
        .collect();
    store.add_entries(id, &entries)
}

#[cfg(test)]
mod tests;
