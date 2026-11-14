//! Account messages and queue shuffling for [`App`].

use super::App;
use super::message::Job;
use super::model::AccountMsg;
use crate::copy;
use crate::state::Event;
use nemo_core::account::AccountError;
use nemo_core::player::PlayerCommand;
use nemo_core::secret::Secret;
use nemo_core::track::Track;

impl App {
    pub(super) fn account_msg(&mut self, msg: AccountMsg) -> Vec<Job> {
        let acc = &mut self.account;
        match msg {
            AccountMsg::Open => {
                acc.open = true;
                acc.error = None;
                acc.confirm_sign_out = false;
                acc.cookie_mode = false;
                self.anim.sheet.snap(0.0);
            }
            AccountMsg::Close => {
                acc.open = false;
                acc.cookie = Secret::new(String::new());
                acc.cookie_mode = false;
                acc.notice = None;
                acc.confirm_sign_out = false;
                self.ui.text_focused = false;
            }
            AccountMsg::SignIn => {
                if self.ui.settings.tos_acknowledged && !acc.busy {
                    acc.busy = true;
                    acc.error = None;
                    acc.notice = None;
                    return vec![Job::SignIn];
                }
            }
            AccountMsg::CookieMode(on) => {
                acc.cookie_mode = on;
                acc.error = None;
                if !on {
                    acc.cookie = Secret::new(String::new());
                }
                self.ui.text_focused = on;
            }
            AccountMsg::CookieInput(text) => acc.cookie = text,
            AccountMsg::CookieSubmit => {
                if self.ui.settings.tos_acknowledged
                    && !acc.busy
                    && !acc.cookie.expose().trim().is_empty()
                {
                    acc.busy = true;
                    acc.error = None;
                    // The cookie leaves the UI state here; only the service keeps it.
                    let cookie = std::mem::replace(&mut acc.cookie, Secret::new(String::new()));
                    return vec![Job::SignInCookie(cookie)];
                }
            }
            AccountMsg::AskSignOut => acc.confirm_sign_out = true,
            AccountMsg::CancelSignOut => acc.confirm_sign_out = false,
            AccountMsg::SignOut => {
                acc.busy = true;
                acc.confirm_sign_out = false;
                return vec![Job::SignOut];
            }
            AccountMsg::SignedIn(result) => {
                acc.busy = false;
                match result {
                    Ok(_) => {
                        acc.error = None;
                        acc.cookie_mode = false;
                        acc.notice = None;
                        self.ui.text_focused = false;
                    }
                    Err(e) => {
                        acc.error = Some(copy::account_error(&e));
                        if matches!(e, AccountError::Blocked | AccountError::Unavailable(_)) {
                            acc.cookie_mode = true;
                            self.ui.text_focused = true;
                        }
                    }
                }
            }
            AccountMsg::SignedOut(result) => {
                acc.busy = false;
                match result {
                    Ok(report) => {
                        acc.error = None;
                        acc.notice = Some(copy::sign_out_result(&report));
                    }
                    Err(e) => acc.error = Some(copy::account_error(&e)),
                }
            }
        }
        Vec::new()
    }

    /// Shuffle what plays after the current track without interrupting it: every other entry is
    /// removed and added back in random order.
    pub(super) fn shuffle_queue(&mut self) -> Vec<Job> {
        let snap = self.ui.snapshot.clone();
        let Some(current) = snap.queue_index else {
            return Vec::new();
        };
        let mut jobs = Vec::new();
        let mut others: Vec<Track> = snap
            .queue
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != current)
            .map(|(_, t)| t.clone())
            .collect();
        if others.len() < 2 {
            return jobs;
        }
        shuffle(&mut others);
        for i in (0..snap.queue.len()).rev() {
            if i != current {
                jobs.extend(self.dispatch(Event::Command(PlayerCommand::RemoveAt(i))));
            }
        }
        for t in others {
            jobs.extend(self.dispatch(Event::Command(PlayerCommand::Enqueue(t))));
        }
        jobs
    }
}

/// Shuffle in place with a small xorshift generator seeded from the clock.
pub(super) fn shuffle(tracks: &mut [Track]) {
    let mut x = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0x9E37_79B9_7F4A_7C15, |d| {
            u64::from(d.subsec_nanos()) ^ d.as_secs().rotate_left(32)
        })
        | 1;
    for i in (1..tracks.len()).rev() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        // The modulus is at most the list length, so it fits usize.
        #[allow(clippy::cast_possible_truncation)]
        let j = (x % (i as u64 + 1)) as usize;
        tracks.swap(i, j);
    }
}
