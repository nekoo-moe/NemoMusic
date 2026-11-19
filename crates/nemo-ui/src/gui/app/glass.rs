//! The see-through window as the app sees it (appearance-glass spec): the settings plus the one
//! fact the settings cannot hold, whether the window was created transparent.

use super::App;
use crate::scheme::{self, PANE_ALPHA};
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
}
