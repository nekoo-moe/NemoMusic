//! The speaker button of the player bar: mute and unmute through the existing `SetVolume`
//! command. Nothing here touches the audio path.

use nemo_core::player::PlayerCommand;

use super::App;
use super::message::Job;
use crate::state::Event;

/// Volume a press restores when nothing was muted before (the volume was dragged to 0).
pub const UNMUTE_DEFAULT: u8 = 50;

/// The volume a speaker press sets: 0 when the sound is on, else the remembered one.
#[must_use]
pub const fn target(now: u8, before: u8) -> u8 {
    if now > 0 {
        0
    } else if before > 0 {
        before
    } else {
        UNMUTE_DEFAULT
    }
}

impl App {
    /// Mute, or restore the volume from before the mute.
    pub(super) fn toggle_mute(&mut self) -> Vec<Job> {
        let now = self.ui.snapshot.volume_percent;
        if now > 0 {
            self.volume_before_mute = now;
        }
        let next = target(now, self.volume_before_mute);
        self.dispatch(Event::Command(PlayerCommand::SetVolume(next)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::app::Message;
    use crate::gui::headless::{Scenario, build_app};
    use crate::mock::mock_backend;

    #[test]
    fn a_press_mutes_a_press_restores_and_a_dragged_zero_gets_a_default() {
        assert_eq!(target(70, 0), 0);
        assert_eq!(target(0, 70), 70);
        assert_eq!(target(0, 0), UNMUTE_DEFAULT);
    }

    #[test]
    fn the_speaker_mutes_the_player_and_restores_the_old_volume() {
        let backend = mock_backend();
        let mut app = build_app(&backend, &Scenario::default());
        let before = backend.player.snapshot().volume_percent;
        assert!(before > 0);
        let jobs = app.update_core(Message::ToggleMute);
        app.settle_blocking(jobs);
        let muted = backend.player.snapshot();
        assert_eq!(muted.volume_percent, 0);
        app.dispatch(crate::state::Event::Snapshot(muted));
        let jobs = app.update_core(Message::ToggleMute);
        app.settle_blocking(jobs);
        assert_eq!(backend.player.snapshot().volume_percent, before);
    }
}
