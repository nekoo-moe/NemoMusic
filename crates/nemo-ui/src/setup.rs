//! The first-run setup flow as a pure state machine (startup-flow spec, task 2): which step is
//! showing, how Back, Next, Skip and Enter move it, and how the notice gates the way on.
//!
//! No pixels here. The screens (task 3) read [`Setup`] and send [`SetupEvent`]s; the reducer in
//! [`crate::state`] calls [`UiState::setup_event`].

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
}

/// Where the flow is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Setup {
    active: bool,
    step: SetupStep,
    forward: bool,
}

impl Default for Setup {
    /// Not showing, at the first step.
    fn default() -> Self {
        Self {
            active: false,
            step: SetupStep::Welcome,
            forward: true,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::Route;
    use crate::settings::UiSettings;
    use crate::state::Event;
    use nemo_core::player::PlayerSnapshot;

    fn state(settings: UiSettings) -> UiState {
        UiState::new(Route::Home, PlayerSnapshot::default(), false).with_settings(settings)
    }

    fn open() -> UiState {
        let mut s = state(UiSettings::default());
        s.apply(Event::Setup(SetupEvent::Open));
        s
    }

    fn go_to(s: &mut UiState, step: SetupStep) {
        while s.setup.step() != step {
            if s.setup.step() == SetupStep::Notice {
                s.apply(Event::Setup(SetupEvent::NoticeLater));
            } else {
                s.apply(Event::Setup(SetupEvent::Next));
            }
        }
    }

    #[test]
    fn a_new_user_needs_setup_and_an_existing_user_does_not() {
        assert!(UiSettings::default().needs_setup());
        let accepted = UiSettings {
            tos_acknowledged: true,
            ..UiSettings::default()
        };
        assert!(!accepted.needs_setup(), "the notice was accepted before");
        let done = UiSettings {
            setup_completed: true,
            ..UiSettings::default()
        };
        assert!(!done.needs_setup());
        // Not showing until it is opened: the reducer starts with no flow.
        assert!(!state(UiSettings::default()).setup.active());
    }

    #[test]
    fn next_and_back_walk_the_steps_and_stop_at_the_ends() {
        let mut s = open();
        assert_eq!(s.setup.step(), SetupStep::Welcome);
        s.apply(Event::Setup(SetupEvent::Back));
        assert_eq!(s.setup.step(), SetupStep::Welcome);
        s.apply(Event::Setup(SetupEvent::Next));
        assert_eq!(s.setup.step(), SetupStep::Look);
        assert!(s.setup.forward());
        s.apply(Event::Setup(SetupEvent::Back));
        assert_eq!(s.setup.step(), SetupStep::Welcome);
        assert!(!s.setup.forward());
        for step in SetupStep::ALL {
            assert_eq!(SetupStep::ALL[step.index()], step);
        }
        assert_eq!(SetupStep::Done.next(), None);
        assert_eq!(SetupStep::Welcome.prev(), None);
    }

    #[test]
    fn skip_is_always_available_and_stores_completion_once() {
        for step in SetupStep::ALL {
            let mut s = open();
            go_to(&mut s, step);
            let fx = s.apply(Event::Setup(SetupEvent::Skip));
            assert!(!s.setup.active(), "skip from {step:?}");
            assert!(s.settings.setup_completed);
            assert!(matches!(&fx[..], [Effect::SettingsChanged(x)] if x.setup_completed));
        }
        // Running it again and skipping again stores nothing new.
        let mut s = open();
        s.apply(Event::Setup(SetupEvent::Skip));
        s.apply(Event::Setup(SetupEvent::Open));
        assert!(s.setup.active());
        assert!(s.apply(Event::Setup(SetupEvent::Skip)).is_empty());
    }

    #[test]
    fn skipping_leaves_the_notice_for_the_first_run_sheet() {
        let mut s = open();
        assert!(!s.tos_sheet_visible(), "the flow covers the sheet");
        s.apply(Event::Setup(SetupEvent::Skip));
        assert!(s.tos_sheet_visible() && !s.youtube_enabled());
    }

    #[test]
    fn the_notice_is_unticked_and_next_waits_for_an_answer() {
        let mut s = open();
        go_to(&mut s, SetupStep::Notice);
        assert!(!s.tos_checked, "never pre-ticked");
        s.apply(Event::Setup(SetupEvent::Next));
        s.apply(Event::Setup(SetupEvent::Enter));
        assert_eq!(s.setup.step(), SetupStep::Notice);
        // Continue without the tick does nothing.
        assert!(s.apply(Event::Setup(SetupEvent::NoticeContinue)).is_empty());
        assert_eq!(s.setup.step(), SetupStep::Notice);
        assert!(!s.settings.tos_acknowledged);
    }

    #[test]
    fn ticking_and_continuing_accepts_the_notice_and_moves_on() {
        let mut s = open();
        go_to(&mut s, SetupStep::Notice);
        s.apply(Event::TosCheck(true));
        let fx = s.apply(Event::Setup(SetupEvent::Enter));
        assert!(matches!(&fx[..], [Effect::SettingsChanged(x)] if x.tos_acknowledged));
        assert_eq!(s.setup.step(), SetupStep::Sources);
        assert!(s.youtube_enabled());
    }

    #[test]
    fn not_now_moves_on_without_accepting() {
        let mut s = open();
        go_to(&mut s, SetupStep::Notice);
        s.apply(Event::Setup(SetupEvent::NoticeLater));
        assert_eq!(s.setup.step(), SetupStep::Sources);
        assert!(!s.youtube_enabled() && s.tos_dismissed);
        // Finishing does not bring the sheet back this session, as with `Not now` on the sheet.
        s.apply(Event::Setup(SetupEvent::Finish));
        assert!(!s.tos_sheet_visible());
    }

    #[test]
    fn enter_is_next_and_on_the_last_step_it_finishes() {
        let mut s = open();
        s.apply(Event::Setup(SetupEvent::Enter));
        assert_eq!(s.setup.step(), SetupStep::Look);
        go_to(&mut s, SetupStep::Done);
        let fx = s.apply(Event::Setup(SetupEvent::Enter));
        assert!(!s.setup.active());
        assert!(matches!(&fx[..], [Effect::SettingsChanged(x)] if x.setup_completed));
    }

    #[test]
    fn running_setup_again_resets_nothing() {
        let mut settings = UiSettings {
            tos_acknowledged: true,
            setup_completed: true,
            sponsorblock: false,
            ..UiSettings::default()
        };
        settings.volume = 40;
        let mut s = state(settings.clone());
        s.apply(Event::Setup(SetupEvent::Open));
        assert!(s.setup.active() && s.setup.step() == SetupStep::Welcome);
        assert!(s.apply(Event::Setup(SetupEvent::Next)).is_empty());
        assert!(s.apply(Event::Setup(SetupEvent::Skip)).is_empty());
        assert_eq!(s.settings, settings);
        assert!(s.notice_decided());
    }

    #[test]
    fn events_do_nothing_while_the_flow_is_closed() {
        let mut s = state(UiSettings::default());
        for ev in [
            SetupEvent::Next,
            SetupEvent::Back,
            SetupEvent::Skip,
            SetupEvent::Finish,
            SetupEvent::Enter,
            SetupEvent::NoticeContinue,
            SetupEvent::NoticeLater,
        ] {
            assert!(s.apply(Event::Setup(ev)).is_empty());
        }
        assert!(!s.settings.setup_completed && !s.tos_dismissed);
    }
}
