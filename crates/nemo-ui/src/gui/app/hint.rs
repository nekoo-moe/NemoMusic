//! Resolve ahead of the click (cold-start spec): the pointer rests on a track row or a card's
//! play button, or goes down on one, and the backend is told which song to resolve.
//!
//! One song at a time, never a list. The rules, as a small state machine with no window in it:
//! - a pointer that rests on a play target for [`HOVER_HINT_MS`] starts the hint of that song;
//! - a press on it starts the hint at once;
//! - a pointer that leaves cancels a hint that was started (`None` goes to the hook);
//! - a hint is told to the hook once per hover, however many presses follow.

use nemo_core::player::PlayerCommand;
use nemo_core::track::VideoId;

use super::App;
use super::message::{Job, Message};
use crate::state::Event;

/// How long the pointer rests on a play target before its song is hinted.
pub const HOVER_HINT_MS: u64 = 150;

/// What the app does next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// Wait [`HOVER_HINT_MS`], then report the generation back ([`HintState::due`]).
    Delay(u64),
    /// Tell the hook: `Some` starts the hint of that song, `None` cancels the running one.
    Tell(Option<VideoId>),
}

/// Where the pointer is and what the hook was told.
#[derive(Debug, Default)]
pub struct HintState {
    /// Counts the entries and exits of the pointer, so a late timer can be told from a current one.
    generation: u64,
    /// The song of the play target under the pointer.
    over: Option<VideoId>,
    /// The song the hook was told to resolve, until the pointer leaves.
    told: Option<VideoId>,
}

impl HintState {
    /// The pointer entered the play target of `video` (`None`: a target with no song to hint,
    /// such as a local file).
    pub fn enter(&mut self, video: Option<VideoId>) -> Vec<Act> {
        let mut acts = self.leave();
        self.over = video;
        if self.over.is_some() {
            acts.push(Act::Delay(self.generation));
        }
        acts
    }

    /// The pointer left its play target.
    pub fn leave(&mut self) -> Vec<Act> {
        self.generation += 1;
        self.over = None;
        self.told
            .take()
            .map(|_| Act::Tell(None))
            .into_iter()
            .collect()
    }

    /// The delay of generation `generation` ended.
    pub fn due(&mut self, generation: u64) -> Vec<Act> {
        if generation != self.generation {
            return Vec::new();
        }
        self.tell()
    }

    /// The pointer went down on whatever is under it.
    pub fn press(&mut self) -> Vec<Act> {
        self.tell()
    }

    fn tell(&mut self) -> Vec<Act> {
        match &self.over {
            Some(video) if self.told.as_ref() != Some(video) => {
                self.told = Some(video.clone());
                vec![Act::Tell(Some(video.clone()))]
            }
            _ => Vec::new(),
        }
    }
}

impl App {
    /// Handle the pointer messages of the hint: where the pointer rests, the delay, a press.
    pub(super) fn handle_hint(&mut self, message: Message) -> Vec<Job> {
        let acts = match message {
            Message::HoverTarget(Some(play)) => {
                let video = self.hint_video(&play);
                self.hint.enter(video)
            }
            Message::HoverTarget(None) => self.hint.leave(),
            Message::HoverDue(generation) => self.hint.due(generation),
            Message::PointerDown => self.hint.press(),
            _ => Vec::new(),
        };
        let mut jobs = Vec::new();
        for act in acts {
            match act {
                Act::Delay(generation) => jobs.push(Job::HoverDelay(generation)),
                Act::Tell(video) => {
                    if let Some(tell) = &self.hooks.on_hint_song {
                        tell(video);
                    }
                }
            }
        }
        jobs
    }

    /// The song a play message plays first, when it is a YouTube song (a local file has no
    /// stream to resolve).
    fn hint_video(&self, play: &Message) -> Option<VideoId> {
        self.hooks.on_hint_song.as_ref()?;
        let track = match play {
            Message::PlayAt { source, index } => self.tracks_of(source).into_iter().nth(*index)?,
            Message::Ui(Event::Command(PlayerCommand::Play(track))) => track.clone(),
            _ => return None,
        };
        track
            .local_path
            .is_none()
            .then_some(track.video_id)
            .flatten()
    }
}

#[cfg(test)]
mod tests;
