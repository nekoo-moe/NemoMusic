//! The brain of the worker: joins the session, the sleeping rule, the rate limiter, the dedupe
//! and the connect schedule. No clock and no I/O: the caller passes the monotonic time and the
//! wall time and does what the returned [`Action`]s say.

use std::time::{Duration, Instant};

use nemo_core::hooks::PlayHookEvent;

use super::activity::Activity;
use super::away::{AWAY_AFTER, AwayRule};
use super::limiter::RateLimiter;
use super::session::Session;
use super::state::{Browsing, Place, PresenceState, build};

/// The times the engine keeps. [`Timing::default`] holds the values of the spec; tests use
/// short ones.
#[derive(Debug, Clone, Copy)]
pub struct Timing {
    /// Least time between two updates (criterion 14).
    pub min_gap: Duration,
    /// Time between connect attempts while Discord is not reachable (criterion 16).
    pub retry: Duration,
    /// Time out of focus with nothing playing before `Away` (criterion 6).
    pub away_after: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            min_gap: Duration::from_secs(15),
            retry: Duration::from_secs(30),
            away_after: AWAY_AFTER,
        }
    }
}

/// Something that happened.
#[derive(Debug, Clone)]
pub enum Input {
    /// A play event.
    Play(Box<PlayHookEvent>),
    /// The window got or lost the focus.
    Window {
        /// It has the focus.
        active: bool,
    },
    /// The browsed part changed.
    Section(Option<Browsing>),
    /// A sleep timer started or stopped.
    SleepTimer(bool),
    /// The sound moved to another place.
    Place(Place),
    /// `Hide song names on Discord` changed.
    Private(bool),
    /// The switch was turned on or off.
    Switch(bool),
    /// A connect attempt ended (`true`: Discord answered `READY`), or a connection was lost
    /// (`false`).
    Connected(bool),
    /// Time passed.
    Tick,
}

/// What the caller must do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Try to connect; answer with [`Input::Connected`].
    Connect,
    /// Send this activity.
    Set(Box<Activity>),
    /// Clear the activity.
    Clear,
    /// Close the connection (and give up a connect attempt).
    Disconnect,
}

/// How far the connection to Discord is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// The switch is off.
    Off,
    /// On, not connected, and no attempt runs: the next one is at `retry_at`.
    Waiting,
    /// On, and an attempt runs.
    Connecting,
    /// On and connected.
    Connected,
}

/// The state machine. See the module text.
#[derive(Debug)]
pub struct Engine {
    timing: Timing,
    phase: Phase,
    private: bool,
    session: Session,
    away: AwayRule,
    limiter: RateLimiter<Activity>,
    /// The newest activity asked for.
    wanted: Option<Activity>,
    /// The activity Discord shows now.
    last_sent: Option<Activity>,
    was_sleeping: bool,
    retry_at: Option<Instant>,
}

impl Engine {
    /// An engine with the switch off. `wall` is the wall time in Unix seconds.
    #[must_use]
    pub const fn new(timing: Timing, wall: i64) -> Self {
        Self {
            timing,
            phase: Phase::Off,
            private: false,
            session: Session::new(wall),
            away: AwayRule::new(timing.away_after),
            limiter: RateLimiter::new(timing.min_gap),
            wanted: None,
            last_sent: None,
            was_sleeping: false,
            retry_at: None,
        }
    }

    /// Take one input and say what to do.
    pub fn step(&mut self, now: Instant, wall: i64, input: Input) -> Vec<Action> {
        let mut out = Vec::new();
        match input {
            Input::Play(event) => {
                self.session.on_play(&event, wall);
                self.away.set_playing(now, self.session.playing());
            }
            Input::Window { active } => self.away.set_window_active(now, active),
            Input::Section(section) => self.session.set_section(section),
            Input::SleepTimer(on) => self.session.set_sleep_timer(on),
            Input::Place(place) => self.session.set_place(place),
            Input::Private(on) => self.private = on,
            Input::Switch(on) => self.switch(now, on, &mut out),
            Input::Connected(ok) => self.on_connected(now, ok),
            Input::Tick => {}
        }
        self.drive(now, &mut out);
        out
    }

    /// How long until the engine needs a [`Input::Tick`], if it does.
    #[must_use]
    pub fn wake_in(&self, now: Instant) -> Option<Duration> {
        match self.phase {
            Phase::Off | Phase::Connecting => None,
            Phase::Connected => [self.limiter.wait(now), self.away.wake_in(now)]
                .into_iter()
                .flatten()
                .min(),
            Phase::Waiting => self.retry_at.map(|at| at.saturating_duration_since(now)),
        }
    }

    fn switch(&mut self, now: Instant, on: bool, out: &mut Vec<Action>) {
        if on == (self.phase != Phase::Off) {
            return;
        }
        let was_connected = self.phase == Phase::Connected;
        self.wanted = None;
        self.last_sent = None;
        self.limiter = RateLimiter::new(self.timing.min_gap);
        if on {
            self.phase = Phase::Waiting;
            self.retry_at = Some(now);
        } else {
            if was_connected {
                out.push(Action::Clear);
            }
            out.push(Action::Disconnect);
            self.phase = Phase::Off;
            self.retry_at = None;
        }
    }

    fn on_connected(&mut self, now: Instant, ok: bool) {
        if self.phase == Phase::Off {
            return;
        }
        self.wanted = None;
        self.last_sent = None;
        self.limiter.drop_pending();
        if ok {
            self.phase = Phase::Connected;
            self.retry_at = None;
            self.limiter.reset();
        } else {
            self.phase = Phase::Waiting;
            self.retry_at = Some(now + self.timing.retry);
        }
    }

    fn drive(&mut self, now: Instant, out: &mut Vec<Action>) {
        match self.phase {
            Phase::Off | Phase::Connecting => {}
            Phase::Waiting => {
                if self.retry_at.is_some_and(|at| now >= at) {
                    self.phase = Phase::Connecting;
                    self.retry_at = None;
                    out.push(Action::Connect);
                }
            }
            Phase::Connected => self.refresh(now, out),
        }
    }

    fn refresh(&mut self, now: Instant, out: &mut Vec<Action>) {
        let state = self.session.state(self.away.asleep(now));
        let sleeping = state == PresenceState::Sleeping;
        let want = build(&state, self.private);
        let leaving_sleep = self.was_sleeping && !sleeping;
        self.was_sleeping = sleeping;
        if self.last_sent.as_ref() == Some(&want) {
            self.limiter.drop_pending();
            self.wanted = Some(want);
            return;
        }
        let item = if self.wanted.as_ref() == Some(&want) {
            self.limiter.poll(now)
        } else {
            self.wanted = Some(want.clone());
            if leaving_sleep {
                Some(self.limiter.force(now, want))
            } else {
                self.limiter.offer(now, want)
            }
        };
        if let Some(activity) = item {
            self.last_sent = Some(activity.clone());
            out.push(Action::Set(Box::new(activity)));
        }
    }
}

#[cfg(test)]
mod tests;
