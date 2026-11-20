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
    pub(super) fn sc_reset_page(&mut self) {
        self.sc.paste = false;
        self.sc.token = Secret::new(String::new());
        self.sc.error = None;
        self.sc.confirm_sign_out = false;
        self.sc.confirm_off = false;
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
            ScMsg::TurnOff => return self.sc_turn_off(),
            ScMsg::ConfirmOff => return self.sc_off_now(),
            ScMsg::CancelOff => self.sc.confirm_off = false,
            ScMsg::TurnedOff(result) => {
                self.sc.busy = false;
                self.sc_reset_page();
                if result.is_err() {
                    // The switch goes off anyway; the next start tries to remove the token again.
                    self.sc.error = Some(text::OFF_NOT_REMOVED.to_owned());
                }
                return self.sc_switch_off();
            }
            ScMsg::SignIn => {
                let jobs = self.sc_sign_in(Job::ScSignIn);
                self.sc.window = !jobs.is_empty();
                self.sc.rechecked = false;
                return jobs;
            }
            ScMsg::Recheck => {
                if let Some(account) = &self.backend.soundcloud_account {
                    account.recheck_sign_in();
                    self.sc.rechecked = true;
                }
            }
            ScMsg::PasteMode(on) => {
                self.sc.paste = on;
                self.sc.error = None;
                if !on {
                    self.sc.token = Secret::new(String::new());
                }
                self.ui.text_focused = on;
            }
            ScMsg::PasteInput(text) => self.sc.token = text,
            ScMsg::PasteSubmit => return self.sc_paste_submit(),
            ScMsg::AskSignOut => self.sc.confirm_sign_out = true,
            ScMsg::CancelSignOut => self.sc.confirm_sign_out = false,
            ScMsg::SignOut => {
                self.sc.busy = true;
                self.sc.confirm_sign_out = false;
                return vec![Job::ScSignOut];
            }
            ScMsg::WindowEnded(result) => {
                // A pasted token took over from the window: its answer ("cancelled") is not news.
                if !std::mem::take(&mut self.sc.stale_window) {
                    self.sc.window = false;
                    self.sc_signed_in(result);
                }
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
            ScMsg::ToggleWarning => {
                self.sc.warning_open = !self.sc.warning_open;
            }
        }
        Vec::new()
    }

    /// Store the switch as off and forget the page.
    fn sc_switch_off(&mut self) -> Vec<Job> {
        self.sc.confirming = false;
        let error = self.sc.error.take();
        self.sc_reset_page();
        self.sc.error = error;
        let state = OptInState {
            enabled: false,
            ..self.ui.settings.soundcloud
        };
        self.sc_store(state)
    }

    /// The switch was turned off (owner decision, 2026-11-20: off also signs out and wipes the
    /// token). Signed in, the page asks first. A sign-in window that is open is closed with the
    /// switch. With no account to sign out, the switch goes off at once; a token an earlier
    /// session kept is removed at the next start (`nemo_soundcloud::off`).
    fn sc_turn_off(&mut self) -> Vec<Job> {
        if self.sc.busy {
            // The sign-in window is closed with the switch; a token being checked is a moment,
            // then the switch answers again.
            return if self.sc.window {
                self.sc_off_now()
            } else {
                Vec::new()
            };
        }
        if self.backend.soundcloud_account.is_none() {
            return self.sc_switch_off();
        }
        if matches!(self.sc_state(), Some(SourceAccountState::SignedIn(_))) {
            self.sc.confirm_off = true;
            return Vec::new();
        }
        self.sc_switch_off()
    }

    /// Sign out, then turn the switch off (the answer arrives as [`ScMsg::TurnedOff`]).
    fn sc_off_now(&mut self) -> Vec<Job> {
        if self.sc.window {
            // The account closes the window; its answer ("cancelled") is not news.
            self.sc.window = false;
            self.sc.stale_window = true;
        }
        self.sc.confirm_off = false;
        self.sc.busy = true;
        vec![Job::ScTurnOff]
    }

    /// Send the typed token. While the sign-in window is still open the token takes over: the
    /// account closes the window, and the window's own answer is dropped when it arrives.
    fn sc_paste_submit(&mut self) -> Vec<Job> {
        let over_window = self.sc.busy && self.sc.window;
        if self.sc.token.expose().trim().is_empty() || (self.sc.busy && !over_window) {
            return Vec::new();
        }
        if over_window {
            self.sc.busy = false;
            self.sc.window = false;
            self.sc.stale_window = true;
        }
        // The token leaves the page state here; only the account keeps it.
        let token = std::mem::replace(&mut self.sc.token, Secret::new(String::new()));
        self.sc_sign_in(Job::ScSignInToken(token))
    }

    /// The sign-in window waited long without finding a session: the page then offers "I am
    /// signed in, check again" next to the token paste.
    #[must_use]
    pub fn sc_sign_in_hint(&self) -> bool {
        self.sc.busy
            && self.sc.window
            && self
                .backend
                .soundcloud_account
                .as_ref()
                .is_some_and(|a| a.sign_in_hint())
    }

    /// `true` while a SoundCloud sign-in or sign-out runs, here or in the account.
    #[must_use]
    pub fn sc_busy(&self) -> bool {
        self.sc.busy || matches!(self.sc_state(), Some(SourceAccountState::SigningIn))
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
            Ok(_) => {
                self.sc_reset_page();
                self.sc.read_after_sign_in = true;
            }
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

    /// A running SoundCloud whose account is in `state`.
    fn app_in(state: SourceAccountState) -> App {
        build_app(
            &mock_backend_soundcloud(state),
            &Scenario {
                soundcloud: Some(ScScene::Account),
                ..Scenario::default()
            },
        )
    }

    fn typed(app: &mut App, text: &str) {
        send(app, ScMsg::PasteMode(true));
        send(app, ScMsg::PasteInput(Secret::new(text.to_owned())));
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
    fn turning_off_while_signed_in_asks_first_and_cancel_keeps_it_on() {
        let mut app = app_in(SourceAccountState::SignedIn(
            crate::mock::MockScAccount::info(),
        ));
        assert!(app.sc_open());
        assert!(
            send(&mut app, ScMsg::TurnOff).is_empty(),
            "nothing runs before the answer"
        );
        assert!(app.sc.confirm_off && app.sc_open(), "still on, asking");
        send(&mut app, ScMsg::CancelOff);
        assert!(!app.sc.confirm_off && app.sc_open());
    }

    #[test]
    fn confirming_signs_out_and_only_then_turns_the_switch_off() {
        let mut app = app_in(SourceAccountState::SignedIn(
            crate::mock::MockScAccount::info(),
        ));
        send(&mut app, ScMsg::TurnOff);
        let jobs = send(&mut app, ScMsg::ConfirmOff);
        assert!(matches!(jobs.as_slice(), [Job::ScTurnOff]));
        assert!(app.sc.busy && !app.sc.confirm_off);
        assert!(
            app.sc_open(),
            "the switch stays on until the sign-out answered"
        );
        send(
            &mut app,
            ScMsg::TurnedOff(Ok(nemo_core::account::SignOutReport::default())),
        );
        assert!(!app.sc_open() && !app.sc.busy && app.sc.error.is_none());
        assert_eq!(
            app.ui.settings.soundcloud.confirmed_version, 2,
            "the notice stays confirmed"
        );
    }

    #[test]
    fn a_token_that_could_not_be_removed_is_said_and_the_switch_still_goes_off() {
        let mut app = app_in(SourceAccountState::SignedIn(
            crate::mock::MockScAccount::info(),
        ));
        send(&mut app, ScMsg::TurnOff);
        send(&mut app, ScMsg::ConfirmOff);
        send(
            &mut app,
            ScMsg::TurnedOff(Err(SourceAccountError::Storage("x".to_owned()))),
        );
        assert!(!app.sc_open());
        assert_eq!(app.sc.error.as_deref(), Some(text::OFF_NOT_REMOVED));
    }

    #[test]
    fn off_without_a_session_goes_off_at_once() {
        // Signed out and running.
        let mut out = app(ScScene::Account, true);
        assert!(send(&mut out, ScMsg::TurnOff).is_empty());
        assert!(!out.sc_open() && !out.sc.confirm_off);
        // On, but the services did not start (no account to sign out).
        let mut waiting = app(ScScene::Restart, false);
        assert!(send(&mut waiting, ScMsg::TurnOff).is_empty());
        assert!(!waiting.sc_open());
    }

    #[test]
    fn off_while_the_sign_in_window_is_open_closes_it_without_asking() {
        let mut app = app(ScScene::Account, true);
        send(&mut app, ScMsg::SignIn);
        let jobs = send(&mut app, ScMsg::TurnOff);
        assert!(matches!(jobs.as_slice(), [Job::ScTurnOff]), "{jobs:?}");
        assert!(app.sc.stale_window && !app.sc.window);
        // The window's own answer is dropped, so no "window closed" sentence appears.
        send(
            &mut app,
            ScMsg::WindowEnded(Err(SourceAccountError::Cancelled)),
        );
        send(
            &mut app,
            ScMsg::TurnedOff(Ok(nemo_core::account::SignOutReport::default())),
        );
        assert!(!app.sc_open() && app.sc.error.is_none());
    }

    #[test]
    fn off_while_a_pasted_token_is_checked_waits() {
        let mut app = app(ScScene::Account, true);
        send(&mut app, ScMsg::PasteMode(true));
        send(
            &mut app,
            ScMsg::PasteInput(Secret::new("2-293232-7000001-synthetic".to_owned())),
        );
        send(&mut app, ScMsg::PasteSubmit);
        assert!(app.sc.busy && !app.sc.window);
        assert!(send(&mut app, ScMsg::TurnOff).is_empty());
        assert!(app.sc_open(), "the switch did not move");
    }

    #[test]
    fn the_switch_text_says_that_off_signs_out() {
        assert!(text::STATUS_SIGNED_IN.contains("signs you out and removes your token"));
        assert!(text::STAYS.contains("signs you out and removes your token"));
        assert!(text::OFF_CONFIRM.contains("signs you out and removes your token"));
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
    fn a_token_pasted_while_the_window_is_open_takes_over_and_the_window_answer_is_dropped() {
        let mut app = app(ScScene::Account, true);
        assert!(matches!(
            send(&mut app, ScMsg::SignIn).as_slice(),
            [Job::ScSignIn]
        ));
        assert!(app.sc.busy && app.sc.window);
        typed(&mut app, "2-293232-7000001-synthetic");
        let jobs = send(&mut app, ScMsg::PasteSubmit);
        assert!(matches!(jobs.as_slice(), [Job::ScSignInToken(_)]));
        assert!(app.sc.busy && !app.sc.window && app.sc.stale_window);

        // The window's own answer ("cancelled") arrives and changes nothing on the page.
        send(
            &mut app,
            ScMsg::WindowEnded(Err(SourceAccountError::Cancelled)),
        );
        assert!(app.sc.busy && !app.sc.stale_window && app.sc.error.is_none());

        // The token's answer decides.
        send(&mut app, ScMsg::SignedIn(Err(SourceAccountError::Rejected)));
        assert!(!app.sc.busy && app.sc.error.is_some());
    }

    #[test]
    fn the_window_answer_can_arrive_after_the_token_answer() {
        let mut app = app(ScScene::Account, true);
        send(&mut app, ScMsg::SignIn);
        typed(&mut app, "2-293232-7000001-synthetic");
        send(&mut app, ScMsg::PasteSubmit);
        send(&mut app, ScMsg::SignedIn(Err(SourceAccountError::Rejected)));
        send(
            &mut app,
            ScMsg::WindowEnded(Err(SourceAccountError::Cancelled)),
        );
        let error = app.sc.error.clone().expect("the refusal stays");
        assert!(error.contains("did not accept"), "{error}");
        assert!(!app.sc.busy && !app.sc.stale_window);
    }

    #[test]
    fn a_token_is_not_sent_while_another_token_is_being_checked() {
        let mut app = app(ScScene::Account, true);
        typed(&mut app, "2-293232-7000001-synthetic");
        assert_eq!(send(&mut app, ScMsg::PasteSubmit).len(), 1);
        send(
            &mut app,
            ScMsg::PasteInput(Secret::new("2-293232-7000002-other".to_owned())),
        );
        assert!(send(&mut app, ScMsg::PasteSubmit).is_empty());
        assert_eq!(app.sc.token.expose(), "2-293232-7000002-other");
    }

    #[test]
    fn a_window_that_ends_on_its_own_is_shown_as_before() {
        let mut app = app(ScScene::Account, true);
        send(&mut app, ScMsg::SignIn);
        send(
            &mut app,
            ScMsg::WindowEnded(Err(SourceAccountError::Cancelled)),
        );
        assert!(!app.sc.busy && !app.sc.window);
        assert!(app.sc.error.is_some(), "a closed window is told");
    }

    #[test]
    fn the_hint_shows_only_while_the_window_waits_and_check_again_asks_the_window() {
        let mut app = app_in(SourceAccountState::SigningIn);
        assert!(!app.sc_sign_in_hint(), "no window of ours is open");
        send(&mut app, ScMsg::SignIn);
        // The mock account says the window found nothing for a long time.
        assert!(app.sc.window && app.sc_sign_in_hint());
        assert!(!app.sc.rechecked);
        assert!(send(&mut app, ScMsg::Recheck).is_empty());
        assert!(app.sc.rechecked, "the page says it looked again");
        // A new sign-in starts without that memory.
        send(
            &mut app,
            ScMsg::WindowEnded(Err(SourceAccountError::Cancelled)),
        );
        send(&mut app, ScMsg::SignIn);
        assert!(!app.sc.rechecked);
        // A token check is not a window: no hint for it.
        let mut checking = app_in(SourceAccountState::SigningIn);
        checking.sc.busy = true;
        assert!(!checking.sc_sign_in_hint());
    }

    #[test]
    fn a_sign_in_keeps_the_frames_coming_so_the_spinner_turns() {
        let mut app = app_in(SourceAccountState::SignedOut);
        assert!(!app.sc_busy());
        send(&mut app, ScMsg::SignIn);
        assert!(app.sc_busy());
        let account_busy = app.account_busy();
        assert!(!account_busy, "the Google account is not what is busy");
        app.focused = true;
        assert!(app.needs_frames());
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
