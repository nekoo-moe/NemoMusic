//! Reloading the user's music data (library-refresh spec): after a sign-in, after an account
//! switch, and when the user presses Refresh.
//!
//! Pure: [`Refresh`] decides which [`Step`] runs next and whether an answer still counts. The
//! GUI turns a step into catalog calls and hands the answer back. One reload runs at a time and
//! its steps run one after the other, so requests stay human-paced.

use nemo_core::account::AccountState;

/// The one message a failed reload shows. The data loaded before stays on screen.
pub const FAILED_NOTICE: &str = "Could not refresh your music. Showing what loaded before.";

/// Tooltip of the Refresh control, which also names it (iced 0.14 has no accessibility tree):
/// the name and key, or what it is doing.
#[must_use]
pub const fn tooltip(busy: bool) -> &'static str {
    if busy {
        "Refreshing your music\u{2026}"
    } else {
        "Refresh (F5)"
    }
}

/// Whose data the screen shows, as far as a reload cares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Owner {
    /// Nobody is signed in.
    SignedOut,
    /// A sign-in call runs. Nothing is decided until it ends.
    Pending,
    /// This account, told apart by its name and handle.
    SignedIn {
        /// Account name.
        name: String,
        /// `@handle`, when the account has one.
        handle: Option<String>,
    },
}

impl Owner {
    /// The owner for an account state.
    #[must_use]
    pub fn of(state: &AccountState) -> Self {
        match state {
            AccountState::SignedOut => Self::SignedOut,
            AccountState::SigningIn => Self::Pending,
            AccountState::SignedIn(info) => Self::SignedIn {
                name: info.name.clone(),
                handle: info.handle.clone(),
            },
        }
    }
}

/// One part of a reload. A full reload runs [`Step::Home`], then [`Step::Account`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// The Home shelves (with the account's playlists and liked songs when signed in).
    Home,
    /// Library playlists, liked songs, albums, artists, history and the explore data.
    Account,
}

/// A step to start now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Start {
    /// The run it belongs to; its answer carries it back.
    pub generation: u64,
    /// What to load.
    pub step: Step,
}

/// What to do with the answer of a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Drop the data: it is from an older run, or for an account that is no longer the owner.
    /// Start `next` when it is set (the reload for the new owner).
    Drop {
        /// Step to start now.
        next: Option<Start>,
    },
    /// Use the data (when the step worked), then start `next` when it is set.
    Use {
        /// Step to start now.
        next: Option<Start>,
        /// `Some(failed)` when this answer ended the run: `failed` is `true` when any step of
        /// the run failed.
        ended: Option<bool>,
    },
}

/// The reload state of the app.
#[derive(Debug, Clone, Default)]
pub struct Refresh {
    /// Raised at every start; an answer from an older run is dropped.
    generation: u64,
    /// The step in flight and its run.
    running: Option<(u64, Step)>,
    /// The owner changed while a step was in flight: the first step of the run to start when
    /// that answer is in.
    queued: Option<Step>,
    /// A step of the current run failed.
    failed: bool,
    /// The owner of the data on screen. `None` until the start-up loads were sent.
    owner: Option<Owner>,
}

impl Refresh {
    /// `true` while a reload runs (the Refresh control shows its busy state).
    #[must_use]
    pub fn busy(&self) -> bool {
        self.running.is_some()
    }

    /// `true` once the start-up loads were sent, so Refresh has something to reload.
    #[must_use]
    pub fn ready(&self) -> bool {
        self.owner.is_some()
    }

    /// Number of reloads started so far (0: only the start-up loads ran).
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The owner of the data on screen (`None` before the start-up loads).
    #[must_use]
    pub fn owner(&self) -> Option<&Owner> {
        self.owner.as_ref()
    }

    /// Record whose data the start-up loads fetch. Only the first call counts.
    pub fn begin(&mut self, owner: Owner) {
        if self.owner.is_none() {
            self.owner = Some(owner);
        }
    }

    /// Look at the account now. A new owner starts a reload: a full one after a sign-in or a
    /// switch, only the account step after a sign-out (as the start-up load did before). A
    /// sign-in that is still running changes nothing.
    pub fn observe(&mut self, now: Owner) -> Option<Start> {
        let seen = self.owner.as_ref()?;
        if now == Owner::Pending || *seen == now {
            return None;
        }
        let first = if now == Owner::SignedOut {
            Step::Account
        } else {
            Step::Home
        };
        self.owner = Some(now);
        if self.busy() {
            // The step in flight is for the old owner. A queued full reload covers the account
            // step too, so it is never narrowed.
            self.queued = Some(match self.queued {
                Some(Step::Home) => Step::Home,
                _ => first,
            });
            return None;
        }
        Some(self.start(first))
    }

    /// The user asked for a reload. Ignored while one runs or before the start-up loads.
    pub fn request(&mut self) -> Option<Start> {
        if !self.ready() || self.busy() {
            return None;
        }
        Some(self.start(Step::Home))
    }

    /// Step `step` of run `generation` answered; `ok` is `false` when it failed.
    pub fn answered(&mut self, generation: u64, step: Step, ok: bool) -> Answer {
        if self.running != Some((generation, step)) {
            return Answer::Drop { next: None };
        }
        self.running = None;
        if let Some(first) = self.queued.take() {
            return Answer::Drop {
                next: Some(self.start(first)),
            };
        }
        self.failed |= !ok;
        match step {
            Step::Home => {
                self.running = Some((generation, Step::Account));
                Answer::Use {
                    next: Some(Start {
                        generation,
                        step: Step::Account,
                    }),
                    ended: None,
                }
            }
            Step::Account => Answer::Use {
                next: None,
                ended: Some(self.failed),
            },
        }
    }

    fn start(&mut self, step: Step) -> Start {
        self.generation += 1;
        self.failed = false;
        self.running = Some((self.generation, step));
        Start {
            generation: self.generation,
            step,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nemo_core::account::AccountInfo;

    fn mira() -> Owner {
        Owner::SignedIn {
            name: "Mira".to_owned(),
            handle: Some("@mira".to_owned()),
        }
    }

    fn kasumi() -> Owner {
        Owner::SignedIn {
            name: "Kasumi".to_owned(),
            handle: None,
        }
    }

    fn started() -> Refresh {
        let mut r = Refresh::default();
        r.begin(Owner::SignedOut);
        r
    }

    #[test]
    fn owner_follows_the_account_state() {
        assert_eq!(Owner::of(&AccountState::SignedOut), Owner::SignedOut);
        assert_eq!(Owner::of(&AccountState::SigningIn), Owner::Pending);
        let info = AccountInfo {
            name: "Mira".to_owned(),
            handle: Some("@mira".to_owned()),
            avatar: Vec::new(),
        };
        assert_eq!(Owner::of(&AccountState::SignedIn(info)), mira());
    }

    #[test]
    fn a_sign_in_starts_one_full_reload_in_sequence() {
        let mut r = started();
        assert_eq!(r.observe(Owner::Pending), None);
        let first = r.observe(mira()).expect("a reload starts");
        assert_eq!(first.step, Step::Home);
        assert!(r.busy());
        // Seeing the same account again starts nothing more.
        assert_eq!(r.observe(mira()), None);
        let Answer::Use { next, ended } = r.answered(first.generation, Step::Home, true) else {
            panic!("the home answer counts");
        };
        assert_eq!(ended, None);
        let next = next.expect("the account step follows");
        assert_eq!(next.step, Step::Account);
        assert_eq!(
            r.answered(next.generation, Step::Account, true),
            Answer::Use {
                next: None,
                ended: Some(false)
            }
        );
        assert!(!r.busy());
        assert_eq!(r.generation(), 1);
    }

    #[test]
    fn a_second_refresh_while_busy_is_ignored() {
        let mut r = started();
        let first = r.request().expect("idle: starts");
        assert_eq!(r.request(), None);
        assert_eq!(r.request(), None);
        assert_eq!(r.generation(), 1);
        let Answer::Use { next, .. } = r.answered(first.generation, Step::Home, true) else {
            panic!("counts");
        };
        assert_eq!(r.request(), None, "still busy during the account step");
        let next = next.expect("account step");
        r.answered(next.generation, Step::Account, true);
        assert!(r.request().is_some(), "idle again");
    }

    #[test]
    fn nothing_reloads_before_the_start_up_loads() {
        let mut r = Refresh::default();
        assert!(!r.ready());
        assert_eq!(r.request(), None);
        assert_eq!(r.observe(mira()), None);
        r.begin(mira());
        r.begin(Owner::SignedOut);
        assert_eq!(r.observe(mira()), None, "the first owner sticks");
    }

    #[test]
    fn a_failed_step_is_reported_when_the_run_ends() {
        let mut r = started();
        let first = r.request().expect("starts");
        let Answer::Use { next, ended: None } = r.answered(first.generation, Step::Home, false)
        else {
            panic!("a failed step still moves on");
        };
        let next = next.expect("account step");
        assert_eq!(
            r.answered(next.generation, Step::Account, true),
            Answer::Use {
                next: None,
                ended: Some(true)
            }
        );
        // The next run starts clean.
        let again = r.request().expect("starts");
        r.answered(again.generation, Step::Home, true);
        assert_eq!(
            r.answered(again.generation, Step::Account, true),
            Answer::Use {
                next: None,
                ended: Some(false)
            }
        );
    }

    #[test]
    fn a_switch_while_busy_drops_the_old_answer_and_reloads_once() {
        let mut r = started();
        let first = r.request().expect("starts");
        assert_eq!(r.observe(mira()), None, "waits for the step in flight");
        assert_eq!(r.observe(kasumi()), None);
        let Answer::Drop { next: Some(next) } = r.answered(first.generation, Step::Home, true)
        else {
            panic!("the old answer is dropped and the new run starts");
        };
        assert_eq!(next.step, Step::Home);
        assert_eq!(r.generation(), 2);
        // A late answer of the old run never counts.
        assert_eq!(
            r.answered(first.generation, Step::Account, true),
            Answer::Drop { next: None }
        );
    }

    #[test]
    fn sign_out_reloads_only_the_account_step() {
        let mut r = Refresh::default();
        r.begin(mira());
        let s = r.observe(Owner::SignedOut).expect("starts");
        assert_eq!(s.step, Step::Account);
        assert_eq!(
            r.answered(s.generation, Step::Account, true),
            Answer::Use {
                next: None,
                ended: Some(false)
            }
        );
        // Signing in again (same account) reloads in full.
        assert_eq!(r.observe(mira()).map(|s| s.step), Some(Step::Home));
    }
}
