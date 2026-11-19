//! The see-through window as the app sees it (appearance-glass spec): the settings plus the one
//! fact the settings cannot hold, whether the window was created transparent.

use iced::Task;

use super::{App, Message};
use crate::copy::appearance as words;
use crate::scheme::{self, PANE_ALPHA};
use crate::state::Event;
use crate::window_glass::WindowGlass;

impl App {
    /// The window glass inputs now.
    #[must_use]
    pub fn window_glass(&self) -> WindowGlass {
        self.ui.settings.window_glass(self.window_transparent)
    }

    /// `true` when the window draws see-through now. The mini player is always solid.
    #[must_use]
    pub fn window_is_clear(&self) -> bool {
        !self.mini && self.window_glass().is_clear()
    }

    /// `true` while the `Restart now` pill is on offer: the switch asks for a see-through window
    /// the running one was not created as, the parent can restart, and no restart has started.
    #[must_use]
    pub fn restart_offered(&self) -> bool {
        self.on_restart.is_some() && !self.restarting && self.window_glass().needs_restart()
    }

    /// The pill was pressed (bar-material spec): ask the parent for a new copy of the program,
    /// then close this window through iced, which releases the audio device. When the new copy
    /// did not start, say so in one sentence and keep running. Nothing else restarts the app.
    pub(super) fn restart(&mut self) -> Task<Message> {
        let Some(start) = self.on_restart.clone().filter(|_| self.restart_offered()) else {
            return Task::none();
        };
        if start() {
            self.restarting = true;
            iced::exit()
        } else {
            let jobs = self.dispatch(Event::Notice(words::RESTART_FAILED.to_owned()));
            self.jobs_to_task(jobs)
        }
    }

    /// Publish the pane alpha the views read ([`scheme::pane_alpha`]). Called at start and after
    /// every settings change.
    pub(super) fn sync_pane_alpha(&self) {
        scheme::set_pane_alpha(self.window_glass().pane_alpha(PANE_ALPHA));
    }
}

#[cfg(test)]
mod tests {
    use crate::gui::headless::{Scenario, build_app};
    use crate::mock::mock_backend;
    use crate::settings::{SettingChange, UiSettings};
    use crate::state::Event;
    use crate::window_glass::GlassState;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::super::Message;

    fn app(window_glass: bool) -> crate::gui::App {
        build_app(
            &mock_backend(),
            &Scenario {
                settings: UiSettings {
                    tos_acknowledged: true,
                    window_glass,
                    ..UiSettings::default()
                },
                ..Scenario::default()
            },
        )
    }

    fn set(app: &mut crate::gui::App, change: SettingChange) {
        let _ = app.dispatch(Event::Setting(change));
    }

    #[test]
    fn a_window_created_see_through_follows_the_switch_at_once() {
        let mut app = app(true);
        assert!(app.window_is_clear());
        set(&mut app, SettingChange::WindowGlass(false));
        assert!(!app.window_is_clear(), "turning it off applies at once");
        set(&mut app, SettingChange::WindowGlass(true));
        assert!(app.window_is_clear());
        set(&mut app, SettingChange::WindowGlassOpacity(55));
        assert!((app.window_glass().backdrop_alpha(false) - 0.55).abs() < 1e-6);
    }

    #[test]
    fn a_window_created_solid_stays_solid_until_a_restart() {
        let mut app = app(false);
        assert_eq!(app.window_glass().state(), GlassState::Off);
        set(&mut app, SettingChange::WindowGlass(true));
        assert!(!app.window_is_clear());
        assert_eq!(app.window_glass().state(), GlassState::NeedsRestart);
    }

    #[test]
    fn reduce_transparency_and_the_mini_player_keep_the_window_solid() {
        let mut app = app(true);
        set(&mut app, SettingChange::ReduceTransparency(true));
        assert!(!app.window_is_clear());
        assert_eq!(app.window_glass().state(), GlassState::Overridden);
        set(&mut app, SettingChange::ReduceTransparency(false));
        assert!(app.window_is_clear());
        app.mini = true;
        assert!(!app.window_is_clear());
    }

    /// A restart hook that counts its calls and answers `ok`.
    fn counting(app: &mut crate::gui::App, ok: bool) -> Arc<AtomicUsize> {
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&calls);
        app.on_restart = Some(Arc::new(move || {
            seen.fetch_add(1, Ordering::SeqCst);
            ok
        }));
        calls
    }

    fn press(app: &mut crate::gui::App, message: Message) {
        // The task closes the window or shows a notice; only the state is checked here.
        let _ = app.update(message);
    }

    #[test]
    fn the_pill_is_offered_only_when_the_switch_needs_a_restart() {
        let mut solid = app(false);
        assert!(!solid.restart_offered(), "nothing changed");
        press(
            &mut solid,
            Message::Ui(Event::Setting(SettingChange::WindowGlass(true))),
        );
        assert!(
            solid.restart_offered(),
            "on, but the window was created solid"
        );
        assert!(solid.surfaces.restart.is_on(), "the pill enters");
        press(
            &mut solid,
            Message::Ui(Event::Setting(SettingChange::ReduceTransparency(true))),
        );
        assert!(
            !solid.restart_offered(),
            "Reduce transparency overrides the switch"
        );
        assert!(!solid.surfaces.restart.is_on());
        press(
            &mut solid,
            Message::Ui(Event::Setting(SettingChange::ReduceTransparency(false))),
        );
        press(
            &mut solid,
            Message::Ui(Event::Setting(SettingChange::WindowGlass(false))),
        );
        assert!(!solid.restart_offered(), "off again: back where it started");
        let clear = app(true);
        assert!(
            !clear.restart_offered(),
            "created see-through: nothing to restart"
        );
        let mut no_hook = app(false);
        press(
            &mut no_hook,
            Message::Ui(Event::Setting(SettingChange::WindowGlass(true))),
        );
        no_hook.on_restart = None;
        assert!(!no_hook.restart_offered(), "no hook, no pill");
    }

    #[test]
    fn a_press_asks_the_parent_once_and_never_without_need() {
        let mut app = app(false);
        let calls = counting(&mut app, true);
        press(&mut app, Message::Restart);
        assert_eq!(calls.load(Ordering::SeqCst), 0, "nothing to restart for");
        press(
            &mut app,
            Message::Ui(Event::Setting(SettingChange::WindowGlass(true))),
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "turning the switch on does not restart"
        );
        press(&mut app, Message::Restart);
        press(&mut app, Message::Restart);
        assert_eq!(calls.load(Ordering::SeqCst), 1, "one request per restart");
        assert!(app.restarting && !app.restart_offered());
    }

    #[test]
    fn a_failed_restart_says_so_once_and_keeps_running() {
        let mut app = app(false);
        let calls = counting(&mut app, false);
        press(
            &mut app,
            Message::Ui(Event::Setting(SettingChange::WindowGlass(true))),
        );
        press(&mut app, Message::Restart);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(!app.restarting, "still running");
        assert!(app.restart_offered(), "the pill stays, to try again");
        let toast = app.ui.toast.as_ref().map(|t| t.text.as_str());
        assert_eq!(toast, Some(crate::copy::appearance::RESTART_FAILED));
    }
}
