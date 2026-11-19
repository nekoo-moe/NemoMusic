//! The SoundCloud switch and account as the app sees them (soundcloud-account spec): the opt-in
//! choice, the sign-in and sign-out calls, and the restart a first turn-on needs. The services
//! are built when the app starts, so turning the switch on shows `Restart now`; turning it off
//! stops the requests at once (the app switches its client off from the same setting).

use iced::Task;
use nemo_core::opt_in::OptInInfo;
use nemo_core::secret::Secret;
use nemo_core::source_account::{SourceAccountError, SourceAccountState};

use super::message::Job;
use super::model::ScMsg;
use super::{App, Message};
use crate::copy::{appearance as words, soundcloud as text};
use crate::settings::{OptInState, SettingChange};
use crate::state::Event;

impl App {
    /// What the SoundCloud switch shows, when the app has SoundCloud at all.
    #[must_use]
    pub fn sc_info(&self) -> Option<&OptInInfo> {
        self.soundcloud_info.as_ref()
    }

    /// The switch is on and the current notice was confirmed.
    #[must_use]
    pub fn sc_open(&self) -> bool {
        self.sc_info()
            .is_some_and(|info| self.ui.settings.soundcloud.is_open(info.version))
    }

    /// The SoundCloud services are running in this session (they exist only when the switch was
    /// on and confirmed at start).
    #[must_use]
    pub fn sc_running(&self) -> bool {
        self.backend.soundcloud.is_some()
    }

    /// The account state while SoundCloud is on and running; `None` otherwise (nothing to show).
    #[must_use]
    pub fn sc_state(&self) -> Option<SourceAccountState> {
        if !self.sc_open() {
            return None;
        }
        self.backend.soundcloud_account.as_ref().map(|a| a.state())
    }

    /// `true` while the `Restart now` button is on offer: the switch is on but the services did
    /// not start with this session, the parent can restart, and no restart has started.
    #[must_use]
    pub fn sc_restart_offered(&self) -> bool {
        self.on_restart.is_some() && !self.restarting && self.sc_open() && !self.sc_running()
    }

    /// Store the new switch state through the shared settings path (saved by the app).
    fn sc_store(&mut self, state: OptInState) -> Vec<Job> {
        self.dispatch(Event::Setting(SettingChange::SoundCloud(state)))
    }

    /// Forget what a sign-in left in the page: the typed token, errors, the confirmation.
    fn sc_reset_page(&mut self) {
        self.sc.paste = false;
        self.sc.token = Secret::new(String::new());
        self.sc.error = None;
        self.sc.confirm_sign_out = false;
        self.ui.text_focused = false;
    }

    pub(super) fn sc_msg(&mut self, msg: ScMsg) -> Vec<Job> {
        let Some(info) = self.soundcloud_info else {
            return Vec::new();
        };
        match msg {
            ScMsg::Source(_) | ScMsg::Tab(_) | ScMsg::Reload | ScMsg::LibraryLoaded { .. } => {
                return self.sc_library_msg(msg);
            }
            ScMsg::Ask => self.sc.confirming = true,
            ScMsg::Cancel => self.sc.confirming = false,
            ScMsg::TurnOn => {
                self.sc.confirming = false;
                let state = OptInState {
                    enabled: true,
                    confirmed_version: info.version,
                };
                return self.sc_store(state);
            }
            ScMsg::TurnOff => {
                self.sc.confirming = false;
                self.sc_reset_page();
                let state = OptInState {
                    enabled: false,
                    ..self.ui.settings.soundcloud
                };
                return self.sc_store(state);
            }
            ScMsg::SignIn => return self.sc_sign_in(Job::ScSignIn),
            ScMsg::PasteMode(on) => {
                self.sc.paste = on;
                self.sc.error = None;
                if !on {
                    self.sc.token = Secret::new(String::new());
                }
                self.ui.text_focused = on;
            }
            ScMsg::PasteInput(text) => self.sc.token = text,
            ScMsg::PasteSubmit => {
                if !self.sc.token.expose().trim().is_empty() {
                    // The token leaves the page state here; only the account keeps it.
                    let token = std::mem::replace(&mut self.sc.token, Secret::new(String::new()));
                    return self.sc_sign_in(Job::ScSignInToken(token));
                }
            }
            ScMsg::AskSignOut => self.sc.confirm_sign_out = true,
            ScMsg::CancelSignOut => self.sc.confirm_sign_out = false,
            ScMsg::SignOut => {
                self.sc.busy = true;
                self.sc.confirm_sign_out = false;
                return vec![Job::ScSignOut];
            }
            ScMsg::SignedIn(result) => self.sc_signed_in(result),
            ScMsg::SignedOut(result) => {
                self.sc.busy = false;
                match result {
                    Ok(report) => {
                        self.sc.error = None;
                        self.sc.notice = Some(text::sign_out_result(&report));
                    }
                    Err(e) => self.sc.error = Some(text::error(&e)),
                }
            }
        }
        Vec::new()
    }

    /// Start `job` when SoundCloud is on and running and no call is open.
    fn sc_sign_in(&mut self, job: Job) -> Vec<Job> {
        if !self.sc_open() || self.sc.busy || self.backend.soundcloud_account.is_none() {
            return Vec::new();
        }
        self.sc.busy = true;
        self.sc.error = None;
        self.sc.notice = None;
        vec![job]
    }

    fn sc_signed_in(
        &mut self,
        result: Result<nemo_core::source_account::SourceAccountInfo, SourceAccountError>,
    ) {
        self.sc.busy = false;
        match result {
            Ok(_) => self.sc_reset_page(),
            Err(e) => {
                self.sc.error = Some(text::error(&e));
                if matches!(e, SourceAccountError::Unavailable(_)) {
                    // The window cannot open: the paste field is the way in.
                    self.sc.paste = true;
                    self.ui.text_focused = true;
                }
            }
        }
    }

    /// The `Restart now` button of the SoundCloud page: ask the parent for a new copy of the
    /// program, then close this window through iced. When the new copy did not start, say so in
    /// one sentence and keep running.
    pub(super) fn sc_restart(&mut self) -> Task<Message> {
        let Some(start) = self
            .on_restart
            .clone()
            .filter(|_| self.sc_restart_offered())
        else {
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
}

#[cfg(test)]
mod tests {
    use nemo_core::source_account::SourceAccountState;

    use super::*;
    use crate::gui::headless::soundcloud::{ScScene, sample_info};
    use crate::gui::headless::{Scenario, build_app};
    use crate::mock::{mock_backend, mock_backend_soundcloud};

    /// An app in `scene`, with SoundCloud running in the backend when `running`.
    fn app(scene: ScScene, running: bool) -> App {
        let backend = if running {
            mock_backend_soundcloud(SourceAccountState::SignedOut)
        } else {
            mock_backend()
        };
        build_app(
            &backend,
            &Scenario {
                soundcloud: Some(scene),
                ..Scenario::default()
            },
        )
    }

    fn send(app: &mut App, msg: ScMsg) -> Vec<Job> {
        app.update_core(Message::Sc(msg))
    }

    #[test]
    fn the_switch_needs_the_panel_before_it_turns_on_and_cancel_changes_nothing() {
        let mut app = app(ScScene::Off, false);
        assert!(!app.sc_open());
        send(&mut app, ScMsg::Ask);
        assert!(
            app.sc.confirming && !app.sc_open(),
            "asking is not turning on"
        );
        send(&mut app, ScMsg::Cancel);
        assert!(!app.sc.confirming && !app.sc_open());
        send(&mut app, ScMsg::Ask);
        send(&mut app, ScMsg::TurnOn);
        assert!(!app.sc.confirming && app.sc_open());
        assert_eq!(
            app.ui.settings.soundcloud.confirmed_version,
            sample_info().version
        );
    }

    #[test]
    fn turning_off_keeps_the_confirmed_notice_and_forgets_the_page() {
        let mut app = app(ScScene::Paste, true);
        assert!(app.sc_open() && app.sc.paste);
        send(&mut app, ScMsg::TurnOff);
        assert!(!app.sc_open());
        assert!(!app.sc.paste && app.sc.token.expose().is_empty());
        assert_eq!(app.ui.settings.soundcloud.confirmed_version, 2);
        send(&mut app, ScMsg::TurnOn);
        assert!(app.sc_open(), "no new panel for the same notice");
    }

    #[test]
    fn a_notice_of_an_older_version_does_not_open_the_gate() {
        let mut app = app(ScScene::Off, false);
        app.ui.settings.soundcloud = OptInState {
            enabled: true,
            confirmed_version: 1,
        };
        assert!(!app.sc_open());
    }

    #[test]
    fn sign_in_starts_one_call_only_when_on_and_running() {
        let mut off = app(ScScene::Off, true);
        assert!(send(&mut off, ScMsg::SignIn).is_empty(), "off");
        let mut not_running = app(ScScene::Restart, false);
        assert!(
            send(&mut not_running, ScMsg::SignIn).is_empty(),
            "no account built"
        );
        let mut on = app(ScScene::Account, true);
        let jobs = send(&mut on, ScMsg::SignIn);
        assert!(matches!(jobs.as_slice(), [Job::ScSignIn]));
        assert!(on.sc.busy);
        assert!(
            send(&mut on, ScMsg::SignIn).is_empty(),
            "one call at a time"
        );
    }

    #[test]
    fn a_pasted_token_leaves_the_page_when_sent_and_never_shows_in_debug() {
        let mut app = app(ScScene::Account, true);
        send(&mut app, ScMsg::PasteMode(true));
        send(&mut app, ScMsg::PasteInput(Secret::new("   ".to_owned())));
        assert!(
            send(&mut app, ScMsg::PasteSubmit).is_empty(),
            "blank is not sent"
        );
        send(
            &mut app,
            ScMsg::PasteInput(Secret::new("2-293232-7000001-synthetic".to_owned())),
        );
        assert!(!format!("{:?}", app.sc).contains("synthetic"));
        let jobs = send(&mut app, ScMsg::PasteSubmit);
        assert!(matches!(jobs.as_slice(), [Job::ScSignInToken(_)]));
        assert!(app.sc.token.expose().is_empty(), "emptied when sent");
        assert!(!format!("{jobs:?}").contains("synthetic"));
    }

    #[test]
    fn a_window_that_cannot_open_offers_the_paste_and_success_clears_the_page() {
        let mut app = app(ScScene::Account, true);
        send(
            &mut app,
            ScMsg::SignedIn(Err(SourceAccountError::Unavailable("x".to_owned()))),
        );
        assert!(app.sc.paste && app.sc.error.is_some());
        send(
            &mut app,
            ScMsg::SignedIn(Ok(nemo_core::source_account::SourceAccountInfo {
                name: "n".to_owned(),
                avatar: Vec::new(),
            })),
        );
        assert!(!app.sc.paste && app.sc.error.is_none() && !app.sc.busy);
    }

    #[test]
    fn sign_out_asks_first_and_reports_what_it_removed() {
        let mut app = app(ScScene::Account, true);
        send(&mut app, ScMsg::AskSignOut);
        assert!(app.sc.confirm_sign_out);
        send(&mut app, ScMsg::CancelSignOut);
        assert!(!app.sc.confirm_sign_out);
        let jobs = send(&mut app, ScMsg::SignOut);
        assert!(matches!(jobs.as_slice(), [Job::ScSignOut]));
        send(
            &mut app,
            ScMsg::SignedOut(Ok(nemo_core::account::SignOutReport::default())),
        );
        assert!(!app.sc.busy);
        assert!(
            app.sc
                .notice
                .as_deref()
                .is_some_and(|n| n.starts_with("Signed out of SoundCloud"))
        );
    }

    #[test]
    fn restart_is_offered_only_when_on_but_not_running() {
        assert!(app(ScScene::Restart, false).sc_restart_offered());
        assert!(
            !app(ScScene::Account, true).sc_restart_offered(),
            "already running"
        );
        assert!(!app(ScScene::Off, false).sc_restart_offered(), "off");
    }

    #[test]
    fn an_app_without_soundcloud_ignores_every_message() {
        let mut app = build_app(&mock_backend(), &Scenario::default());
        assert!(app.sc_info().is_none());
        for msg in [ScMsg::Ask, ScMsg::TurnOn, ScMsg::SignIn] {
            assert!(send(&mut app, msg).is_empty());
        }
        assert!(!app.sc_open() && !app.sc.confirming);
    }
}
