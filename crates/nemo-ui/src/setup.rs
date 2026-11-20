//! The first-run setup flow as a pure state machine (startup-flow spec, task 2): which step is
//! showing, how Back, Next, Skip and Enter move it, and how the notice gates the way on.
//!
//! No pixels here. The screens (task 3) read [`Setup`] and send [`SetupEvent`]s; the reducer in
//! [`crate::state`] calls [`UiState::setup_event`].

use crate::settings::UiSettings;
use crate::state::{Effect, UiState};

/// The steps of the flow, in order. The notice comes before the sources because signing in to
/// YouTube Music needs it (nothing leaves the computer before it is accepted).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SetupStep {
    /// What NemoMusic is, and that it runs on this computer.
    Welcome,
    /// Base mode, accent strength, corner style, Now Playing style, reduce motion.
    Look,
    /// The first-run notice with its unticked checkbox.
    Notice,
    /// YouTube Music, SoundCloud and local folders.
    Sources,
    /// Ad blocking and SponsorBlock.
    Playback,
    /// A summary and `Start listening`.
    Done,
}

impl SetupStep {
    /// Every step, in order.
    pub const ALL: [Self; 6] = [
        Self::Welcome,
        Self::Look,
        Self::Notice,
        Self::Sources,
        Self::Playback,
        Self::Done,
    ];

    /// Position in [`Self::ALL`], from 0.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Welcome => 0,
            Self::Look => 1,
            Self::Notice => 2,
            Self::Sources => 3,
            Self::Playback => 4,
            Self::Done => 5,
        }
    }

    /// Name of the step, for the step dots' accessible names and the page title.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Welcome => "Welcome",
            Self::Look => "Look",
            Self::Notice => "Privacy and risk",
            Self::Sources => "Sources",
            Self::Playback => "Playback",
            Self::Done => "Done",
        }
    }

    /// Name of the step localized according to user language.
    #[must_use]
    pub const fn title_localized(self, lang: crate::settings::Language) -> &'static str {
        crate::i18n::step_title(self, lang)
    }

    /// The step after this one, if any.
    #[must_use]
    pub fn next(self) -> Option<Self> {
        Self::ALL.get(self.index() + 1).copied()
    }

    /// The step before this one, if any.
    #[must_use]
    pub fn prev(self) -> Option<Self> {
        self.index()
            .checked_sub(1)
            .and_then(|i| Self::ALL.get(i))
            .copied()
    }
}

/// A key or button of the flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupEvent {
    /// Show the flow from the first step (first run, or `Run setup again`).
    Open,
    /// Next button.
    Next,
    /// Back button.
    Back,
    /// `Skip setup`.
    Skip,
    /// `Start listening` on the last step.
    Finish,
    /// Enter: the primary action of the step.
    Enter,
    /// `Continue` on the notice step (needs the ticked checkbox).
    NoticeContinue,
    /// `Not now` on the notice step.
    NoticeLater,
    /// `Use without an account` on the Sources step.
    UseWithoutAccount,
    /// `Skip` on the local music card of the Sources step.
    SkipFolders,
}

/// Where the flow is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Setup {
    active: bool,
    step: SetupStep,
    forward: bool,
    without_account: bool,
    folders_skipped: bool,
}

impl Default for Setup {
    /// Not showing, at the first step.
    fn default() -> Self {
        Self {
            active: false,
            step: SetupStep::Welcome,
            forward: true,
            without_account: false,
            folders_skipped: false,
        }
    }
}

impl Setup {
    /// `true` while the flow covers the window.
    #[must_use]
    pub fn active(&self) -> bool {
        self.active
    }

    /// The step showing (or the one it would open on).
    #[must_use]
    pub fn step(&self) -> SetupStep {
        self.step
    }

    /// `true` when the last move went forward (the next step slides in from the right).
    #[must_use]
    pub fn forward(&self) -> bool {
        self.forward
    }

    /// `true` once the user chose `Use without an account` on the Sources step.
    #[must_use]
    pub fn without_account(&self) -> bool {
        self.without_account
    }

    /// `true` once the user chose `Skip` on the local music card.
    #[must_use]
    pub fn folders_skipped(&self) -> bool {
        self.folders_skipped
    }

    /// Show the flow from the first step.
    pub fn open(&mut self) {
        *self = Self {
            active: true,
            ..Self::default()
        };
    }

    fn close(&mut self) {
        self.active = false;
    }

    fn go_next(&mut self) {
        if let Some(next) = self.step.next() {
            self.step = next;
            self.forward = true;
        }
    }

    fn go_back(&mut self) {
        if let Some(prev) = self.step.prev() {
            self.step = prev;
            self.forward = false;
        }
    }
}

impl UiState {
    /// `true` once the user has answered the notice: accepted it, or chosen `Not now`. The notice
    /// step lets Next through only then.
    #[must_use]
    pub fn notice_decided(&self) -> bool {
        self.settings.tos_acknowledged || self.tos_dismissed
    }

    /// Run one event of the setup flow. Skip and Finish store `setup_completed`; nothing else
    /// here touches sign-in, settings or data, so running the flow again resets nothing.
    pub(crate) fn setup_event(&mut self, event: SetupEvent, fx: &mut Vec<Effect>) {
        if event == SetupEvent::Open {
            self.setup.open();
            return;
        }
        if !self.setup.active() {
            return;
        }
        match event {
            SetupEvent::Open => {}
            SetupEvent::Next => self.setup_next(),
            SetupEvent::Back => self.setup.go_back(),
            SetupEvent::Skip | SetupEvent::Finish => self.setup_end(fx),
            SetupEvent::Enter => match self.setup.step() {
                SetupStep::Notice if !self.notice_decided() => {
                    self.setup_event(SetupEvent::NoticeContinue, fx);
                }
                SetupStep::Done => self.setup_end(fx),
                _ => self.setup_next(),
            },
            SetupEvent::NoticeContinue => {
                if self.setup.step() == SetupStep::Notice && self.tos_checked {
                    self.settings.tos_acknowledged = true;
                    fx.push(Effect::SettingsChanged(self.settings.clone()));
                    self.setup.go_next();
                }
            }
            SetupEvent::NoticeLater => {
                if self.setup.step() == SetupStep::Notice {
                    self.tos_dismissed = true;
                    self.setup.go_next();
                }
            }
            SetupEvent::UseWithoutAccount => self.setup.without_account = true,
            SetupEvent::SkipFolders => self.setup.folders_skipped = true,
        }
    }

    fn setup_next(&mut self) {
        if self.setup.step() == SetupStep::Notice && !self.notice_decided() {
            return;
        }
        self.setup.go_next();
    }

    fn setup_end(&mut self, fx: &mut Vec<Effect>) {
        self.setup.close();
        if !self.settings.setup_completed {
            self.settings.setup_completed = true;
            fx.push(Effect::SettingsChanged(self.settings.clone()));
        }
    }
}

/// What the Done step lists: one line per choice, `(what, how it is set)`. It reads the
/// settings and the flow's own choices only; the account name comes from the caller and no
/// token, cookie or id ever reaches it.
#[must_use]
pub fn summary(
    s: &UiSettings,
    setup: &Setup,
    account_name: Option<&str>,
    folders: usize,
) -> Vec<(&'static str, String)> {
    let on_off = |on: bool| if on { "On" } else { "Off" }.to_owned();
    let youtube = match (account_name, s.tos_acknowledged, setup.without_account()) {
        (Some(name), _, _) => format!("Signed in as {name}"),
        (None, false, _) => "Waiting for the notice".to_owned(),
        (None, true, true) => "Without an account".to_owned(),
        (None, true, false) => "Not signed in".to_owned(),
    };
    let local = match folders {
        0 => "No folder added".to_owned(),
        1 => "1 folder".to_owned(),
        n => format!("{n} folders"),
    };
    vec![
        (
            "Look",
            format!(
                "{}, {} accent, {} corners",
                s.base.label(),
                s.accent_intensity.label(),
                s.corner_style.label()
            ),
        ),
        ("Now Playing", s.now_playing_style.label().to_owned()),
        ("YouTube Music", youtube),
        ("SoundCloud", "Off".to_owned()),
        ("Local music", local),
        ("Ad blocking", on_off(s.block_ads)),
        ("SponsorBlock", on_off(s.sponsorblock)),
    ]
}

pub mod motion;

#[cfg(test)]
mod tests;
