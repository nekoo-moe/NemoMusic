//! The SoundCloud account's writes as the app sees them (soundcloud-account spec, task 7): like
//! and unlike, unfollow, create a playlist, add or remove a track, rename.
//!
//! One write runs at a time. Nothing on screen changes before the server answered: a heart does
//! not fill, a playlist does not appear, until the answer says it happened. A change that failed
//! says so in one sentence, and a write is never sent again by itself.

use nemo_core::audio::Origin;
use nemo_core::catalog::BrowseId;
use nemo_core::source_writes::WriteError;
use nemo_core::track::Track;

use super::message::Job;
use super::model::ScLibData;
use super::{App, Message};
use crate::copy::soundcloud as text;
use crate::route::Route;
use crate::state::Event;

/// One change to send.
#[derive(Debug, Clone)]
pub enum ScWriteJob {
    /// Like or unlike a track.
    Like {
        /// The track.
        track: Track,
        /// `true` to like it.
        liked: bool,
    },
    /// Add a track to one of the user's playlists.
    Add {
        /// The track.
        track: Track,
        /// The playlist.
        playlist: BrowseId,
    },
    /// Remove a track from one of the user's playlists.
    Remove {
        /// The track.
        track: Track,
        /// The playlist.
        playlist: BrowseId,
    },
    /// Create a private playlist, with `first` in it when given.
    Create {
        /// The title as typed.
        title: String,
        /// The track that starts it.
        first: Option<Track>,
    },
    /// Give a playlist a new title.
    Rename {
        /// The playlist.
        playlist: BrowseId,
        /// The title as typed.
        title: String,
    },
    /// Stop following a user.
    Unfollow {
        /// The user's page id.
        user: BrowseId,
    },
}

/// A change that was sent and what came back.
#[derive(Debug, Clone)]
pub struct ScWriteDone {
    /// What was sent.
    pub job: ScWriteJob,
    /// What came back.
    pub result: Result<(), WriteError>,
}

/// Messages of the writes.
#[derive(Debug, Clone)]
pub enum ScWriteMsg {
    /// The track menu shows the playlists to add its track to.
    PickPlaylist(bool),
    /// Open the sheet that names a new playlist, which starts with `first` when given.
    NewPlaylist(Option<Track>),
    /// Add `track` to `playlist`.
    AddTo {
        /// The track.
        track: Track,
        /// The playlist.
        playlist: BrowseId,
    },
    /// Remove `track` from `playlist`.
    RemoveFrom {
        /// The track.
        track: Track,
        /// The playlist.
        playlist: BrowseId,
    },
    /// Open the sheet that renames `playlist`.
    Rename(BrowseId),
    /// Stop following `user`.
    Unfollow(BrowseId),
    /// A change finished.
    Done(Box<ScWriteDone>),
}

/// What the sheet of the Create button does when confirmed.
#[derive(Debug, Clone, Default)]
pub enum CreateTarget {
    /// A playlist of the Google or local account (the sheet as before).
    #[default]
    Account,
    /// A new SoundCloud playlist.
    SoundCloud(Option<Track>),
    /// A new title for a SoundCloud playlist.
    Rename(BrowseId),
}

/// What the writes remember.
#[derive(Debug, Clone, Default)]
pub struct ScWriteUi {
    /// A change is on its way: the next one waits for its answer.
    pub busy: bool,
    /// The track menu lists the playlists instead of the track's actions.
    pub pick: bool,
}

impl App {
    /// Changes can be sent: SoundCloud is on and running, someone is signed in and the app built
    /// the writes.
    #[must_use]
    pub fn sc_can_write(&self) -> bool {
        self.sc_is_signed_in() && self.backend.soundcloud_writes.is_some()
    }

    /// `track` is a SoundCloud track the signed-in account can like or put in a playlist.
    #[must_use]
    pub fn sc_track_writable(&self, track: &Track) -> bool {
        track.origin == Origin::SoundCloud && self.sc_can_write()
    }

    /// The playlists the user made, in the order the library lists them.
    #[must_use]
    pub fn sc_own_playlists(&self) -> &[nemo_core::catalog::PlaylistSummary] {
        let d: &ScLibData = &self.sc_lib.data;
        &d.playlists[..d.own_playlists.min(d.playlists.len())]
    }

    /// `id` is a playlist the user made.
    #[must_use]
    pub fn sc_owns_playlist(&self, id: &BrowseId) -> bool {
        self.sc_own_playlists().iter().any(|p| &p.id == id)
    }

    /// `id` is a user the signed-in account follows.
    #[must_use]
    pub fn sc_follows(&self, id: &BrowseId) -> bool {
        self.sc_lib.data.artists.iter().any(|a| &a.id == id)
    }

    /// The playlist of the user's own that the open page shows, if it is one.
    #[must_use]
    pub fn sc_open_own_playlist(&self) -> Option<BrowseId> {
        match self.ui.route() {
            Route::Playlist(id) if self.sc_owns_playlist(id) => Some(id.clone()),
            _ => None,
        }
    }

    /// The title of one of the user's playlists, for the sentence that confirms a change.
    fn sc_playlist_title(&self, id: &BrowseId) -> String {
        self.sc_lib
            .data
            .playlists
            .iter()
            .find(|p| &p.id == id)
            .map_or_else(|| "the playlist".to_owned(), |p| p.title.clone())
    }

    /// The heart of a SoundCloud track: send the change; the heart follows the answer.
    pub(super) fn sc_like(&mut self, track: Track) -> Vec<Job> {
        if !self.sc_track_writable(&track) {
            return self.dispatch(Event::Notice(text::SIGN_IN_TO_LIKE.to_owned()));
        }
        let liked = !self.is_liked(&track);
        self.sc_send(ScWriteJob::Like { track, liked })
    }

    /// Send `job` unless another change is still on its way.
    fn sc_send(&mut self, job: ScWriteJob) -> Vec<Job> {
        if self.sc_write.busy {
            return self.dispatch(Event::Notice(text::WRITE_BUSY.to_owned()));
        }
        self.sc_write.busy = true;
        vec![Job::ScWrite(job)]
    }

    pub(super) fn sc_write_msg(&mut self, msg: ScWriteMsg) -> Vec<Job> {
        match msg {
            ScWriteMsg::PickPlaylist(on) => {
                self.sc_write.pick = on;
                Vec::new()
            }
            ScWriteMsg::NewPlaylist(first) => {
                self.sc_open_sheet(CreateTarget::SoundCloud(first), String::new())
            }
            ScWriteMsg::Rename(playlist) => {
                let title = self.sc_playlist_title(&playlist);
                self.sc_open_sheet(CreateTarget::Rename(playlist), title)
            }
            ScWriteMsg::AddTo { track, playlist } => {
                self.ui.menu = None;
                self.sc_write.pick = false;
                self.sc_send(ScWriteJob::Add { track, playlist })
            }
            ScWriteMsg::RemoveFrom { track, playlist } => {
                self.ui.menu = None;
                self.sc_send(ScWriteJob::Remove { track, playlist })
            }
            ScWriteMsg::Unfollow(user) => self.sc_send(ScWriteJob::Unfollow { user }),
            ScWriteMsg::Done(done) => self.sc_write_done(*done),
        }
    }

    /// Open the name sheet for `target`, with `name` typed in.
    fn sc_open_sheet(&mut self, target: CreateTarget, name: String) -> Vec<Job> {
        if !self.sc_can_write() {
            return self.dispatch(Event::Notice(text::SIGN_IN_TO_WRITE.to_owned()));
        }
        self.ui.menu = None;
        self.sc_write.pick = false;
        self.create_target = target;
        self.create_name = name;
        self.create_open = true;
        self.ui.text_focused = true;
        Vec::new()
    }

    /// The sheet was confirmed with `name` for a SoundCloud target.
    pub(super) fn sc_sheet_confirm(&mut self, name: String) -> Vec<Job> {
        let target = std::mem::take(&mut self.create_target);
        match target {
            CreateTarget::SoundCloud(first) => {
                self.sc_send(ScWriteJob::Create { title: name, first })
            }
            CreateTarget::Rename(playlist) => self.sc_send(ScWriteJob::Rename {
                playlist,
                title: name,
            }),
            CreateTarget::Account => Vec::new(),
        }
    }

    /// The server answered: show what happened, and only then change what is on screen.
    fn sc_write_done(&mut self, done: ScWriteDone) -> Vec<Job> {
        self.sc_write.busy = false;
        let ScWriteDone { job, result } = done;
        if let Err(error) = result {
            return self.sc_write_failed(&job, error);
        }
        let mut jobs = Vec::new();
        let notice = match &job {
            ScWriteJob::Like { track, liked } => {
                self.sc_apply_like(track, *liked);
                None
            }
            ScWriteJob::Add { playlist, .. } => {
                Some(text::added_to(&self.sc_playlist_title(playlist)))
            }
            ScWriteJob::Remove { playlist, .. } => {
                self.details.remove(&super::DetailKey {
                    kind: super::DetailKind::Playlist,
                    id: playlist.clone(),
                });
                Some(text::removed_from(&self.sc_playlist_title(playlist)))
            }
            ScWriteJob::Create { title, .. } => Some(text::created(title.trim())),
            ScWriteJob::Rename { playlist, title } => {
                self.details.remove(&super::DetailKey {
                    kind: super::DetailKind::Playlist,
                    id: playlist.clone(),
                });
                Some(text::renamed(title.trim()))
            }
            ScWriteJob::Unfollow { user } => {
                self.sc_lib.data = std::sync::Arc::new(ScLibData {
                    artists: self
                        .sc_lib
                        .data
                        .artists
                        .iter()
                        .filter(|a| &a.id != user)
                        .cloned()
                        .collect(),
                    ..(*self.sc_lib.data).clone()
                });
                Some(text::UNFOLLOWED.to_owned())
            }
        };
        // The library on screen is what SoundCloud now holds: read it again.
        if !matches!(job, ScWriteJob::Like { .. } | ScWriteJob::Unfollow { .. }) {
            jobs.extend(self.sc_reload());
        }
        if let Some(notice) = notice {
            jobs.extend(self.dispatch(Event::Notice(notice)));
        }
        jobs
    }

    /// A liked (or unliked) track is now liked (or not) on SoundCloud: the heart and the Likes
    /// tab follow.
    fn sc_apply_like(&mut self, track: &Track, liked: bool) {
        self.likes.insert(track.id.clone(), liked);
        let mut tracks = self.sc_lib.data.likes.clone();
        tracks.retain(|t| t.id != track.id);
        if liked {
            tracks.insert(0, track.clone());
        }
        self.sc_lib.data = std::sync::Arc::new(ScLibData {
            likes: tracks,
            ..(*self.sc_lib.data).clone()
        });
    }

    /// A change that did not happen, in one sentence. A track that was already (or was not) in
    /// the playlist is not a failure.
    fn sc_write_failed(&mut self, job: &ScWriteJob, error: WriteError) -> Vec<Job> {
        let sentence = match (job, error) {
            (ScWriteJob::Add { playlist, .. }, WriteError::Unchanged) => {
                text::already_in(&self.sc_playlist_title(playlist))
            }
            (ScWriteJob::Remove { playlist, .. }, WriteError::Unchanged) => {
                text::not_in(&self.sc_playlist_title(playlist))
            }
            _ => text::write_failed(job_action(job), error),
        };
        self.dispatch(Event::Notice(sentence))
    }
}

/// What the person tried, for the sentence that says it failed.
const fn job_action(job: &ScWriteJob) -> &'static str {
    match job {
        ScWriteJob::Like { liked: true, .. } => "like this track",
        ScWriteJob::Like { liked: false, .. } => "remove this like",
        ScWriteJob::Add { .. } => "add this track to the playlist",
        ScWriteJob::Remove { .. } => "remove this track from the playlist",
        ScWriteJob::Create { .. } => "create the playlist",
        ScWriteJob::Rename { .. } => "rename the playlist",
        ScWriteJob::Unfollow { .. } => "unfollow this user",
    }
}

impl From<ScWriteMsg> for Message {
    fn from(msg: ScWriteMsg) -> Self {
        Self::ScWrite(msg)
    }
}

#[cfg(test)]
mod tests;
