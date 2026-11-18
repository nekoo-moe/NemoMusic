//! The setup flow's share of [`App`]: the step transition and the keys (startup-flow spec).

use super::App;
use super::message::Job;
use crate::keys::Key;
use crate::setup::{SetupEvent, SetupStep};
use crate::state::Event;

/// Slide distance of a step change, in px (`design.md` section 8: translate up to 16 px).
pub const SLIDE_PX: f32 = 16.0;

impl App {
    /// Start the step transition when the flow opens or moves to another step. Reduced motion
    /// shows the new step at once.
    pub(super) fn sync_setup(&mut self) {
        let now = self.ui.setup.active().then(|| self.ui.setup.step());
        if self.setup_seen == now {
            return;
        }
        self.setup_seen = now;
        if now.is_none() {
            self.setup_anim.snap(1.0);
            return;
        }
        self.setup_anim.snap(0.0);
        self.setup_anim.target = 1.0;
        if self.ui.reduced_motion {
            self.setup_anim.finish();
        }
    }

    /// A key press while the flow is open: Enter is the primary action, everything else
    /// (Esc included) does nothing, so nothing destructive hides behind a key.
    pub(super) fn setup_key(&mut self, key: Key) -> Vec<Job> {
        if key == Key::Enter {
            return self.dispatch(Event::Setup(SetupEvent::Enter));
        }
        Vec::new()
    }

    /// Progress of the step transition, 0 to 1.
    #[must_use]
    pub fn setup_progress(&self) -> f32 {
        self.setup_anim.value.clamp(0.0, 1.0)
    }

    /// The step the transition belongs to, if the flow is open.
    #[must_use]
    pub fn setup_step(&self) -> Option<SetupStep> {
        self.setup_seen
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::gui::app::{AccountMsg, Message, UiOptions};
    use crate::keys::Mods;
    use crate::mock::mock_backend;
    use crate::settings::UiSettings;

    fn first_run(reduced: bool) -> App {
        let options = UiOptions {
            start_setup: true,
            reduced_motion: reduced,
            settings: UiSettings::default(),
            ..UiOptions::default()
        };
        let (mut app, _) = App::new(mock_backend(), &options);
        app.finish_animations();
        app
    }

    fn key(app: &mut App, k: Key) {
        drop(app.update_core(Message::Key(k, Mods::default())));
    }

    #[test]
    fn a_new_user_starts_in_the_flow_and_the_notice_sheet_waits() {
        let app = first_run(true);
        assert!(app.ui.setup.active());
        assert_eq!(app.ui.setup.step(), SetupStep::Welcome);
        assert!(!app.ui.tos_sheet_visible());
        assert!(!app.ui.settings.tos_acknowledged && !app.ui.tos_checked);
    }

    #[test]
    fn an_existing_user_is_not_sent_through_the_flow() {
        let settings = UiSettings {
            tos_acknowledged: true,
            ..UiSettings::default()
        };
        let options = UiOptions {
            start_setup: settings.needs_setup(),
            settings,
            ..UiOptions::default()
        };
        let (app, _) = App::new(mock_backend(), &options);
        assert!(!app.ui.setup.active());
    }

    #[test]
    fn enter_is_next_and_escape_and_other_keys_do_nothing() {
        let mut app = first_run(true);
        key(&mut app, Key::Escape);
        key(&mut app, Key::Space);
        key(&mut app, Key::Char('n'));
        assert!(app.ui.setup.active());
        assert_eq!(app.ui.setup.step(), SetupStep::Welcome);
        assert!(!app.ui.settings.setup_completed);
        key(&mut app, Key::Enter);
        assert_eq!(app.ui.setup.step(), SetupStep::Look);
    }

    #[test]
    fn enter_in_the_account_sheet_does_not_move_the_flow() {
        let mut app = first_run(true);
        drop(app.update_core(Message::Account(AccountMsg::Open)));
        key(&mut app, Key::Enter);
        assert_eq!(app.ui.setup.step(), SetupStep::Welcome);
    }

    #[test]
    fn a_step_slides_in_within_400_ms_and_reduced_motion_is_instant() {
        let mut app = first_run(false);
        key(&mut app, Key::Enter);
        assert!(app.setup_progress() < 0.1, "starts at the beginning");
        assert!(app.needs_frames());
        let t0 = Instant::now();
        let mut ms = 0_u32;
        while !app.setup_anim.is_settled() && ms < 1_000 {
            ms += 16;
            drop(app.update_core(Message::Frame(t0 + Duration::from_millis(u64::from(ms)))));
        }
        assert!(ms <= 400, "settled after {ms} ms");
        assert!((app.setup_progress() - 1.0).abs() < 0.01);

        let mut app = first_run(true);
        key(&mut app, Key::Enter);
        assert!((app.setup_progress() - 1.0).abs() < f32::EPSILON);
        assert!(!app.needs_frames());
    }

    #[test]
    fn every_step_builds_a_view_in_every_base_mode() {
        use crate::scheme::BaseMode;
        for base in BaseMode::ALL {
            let options = UiOptions {
                start_setup: true,
                reduced_motion: true,
                settings: UiSettings {
                    base,
                    ..UiSettings::default()
                },
                ..UiOptions::default()
            };
            let (mut app, _) = App::new(mock_backend(), &options);
            for step in SetupStep::ALL {
                assert_eq!(app.ui.setup.step(), step);
                drop(app.view());
                let next = if step == SetupStep::Notice {
                    SetupEvent::NoticeLater
                } else {
                    SetupEvent::Next
                };
                drop(app.dispatch(Event::Setup(next)));
            }
        }
    }
}
