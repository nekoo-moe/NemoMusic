//! The thread that owns the connection to the local Discord app (criteria 13, 16 to 18).
//!
//! The thread exists only while the switch is on. It waits on a mailbox, feeds each input to the
//! [`Engine`] and does what the engine says: connect, set, clear, disconnect. It reads no clock
//! except to tell the engine the time, and it never logs a title, an artist or an address.

use std::collections::VecDeque;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::engine::{Action, Engine, Input, Timing};
use super::ipc::{Connector, Incoming, Link};

/// Inputs a mailbox keeps before it drops the oldest play event.
const MAILBOX_CAPACITY: usize = 64;

/// What the worker needs.
#[derive(Clone)]
pub struct Config {
    /// The Discord application id.
    pub client_id: String,
    /// The times of the engine.
    pub timing: Timing,
    /// Where connections come from.
    pub connector: Arc<dyn Connector>,
}

enum Msg {
    Input(Input),
    Exit,
}

impl Msg {
    /// Only a play event may be dropped when the mailbox is full; a switch or a window fact
    /// must not be lost.
    const fn droppable(&self) -> bool {
        matches!(self, Self::Input(Input::Play(_)))
    }
}

/// A bounded queue that drops the oldest play event when full and wakes the waiting thread.
struct Mailbox {
    queue: Mutex<VecDeque<Msg>>,
    signal: Condvar,
    capacity: usize,
}

impl Mailbox {
    fn new(capacity: usize) -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            signal: Condvar::new(),
            capacity,
        }
    }

    fn lock(&self) -> MutexGuard<'_, VecDeque<Msg>> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn push(&self, msg: Msg) {
        let mut queue = self.lock();
        if queue.len() >= self.capacity {
            let oldest = queue.iter().position(Msg::droppable).unwrap_or(0);
            queue.remove(oldest);
        }
        queue.push_back(msg);
        drop(queue);
        self.signal.notify_one();
    }

    /// The next message, waiting at most `wait` (for ever when `None`).
    fn pop(&self, wait: Option<Duration>) -> Option<Msg> {
        let deadline = wait.map(|w| Instant::now() + w);
        let mut queue = self.lock();
        loop {
            if let Some(msg) = queue.pop_front() {
                return Some(msg);
            }
            match deadline {
                None => {
                    queue = self
                        .signal
                        .wait(queue)
                        .unwrap_or_else(PoisonError::into_inner);
                }
                Some(at) => {
                    let left = at.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return None;
                    }
                    queue = self
                        .signal
                        .wait_timeout(queue, left)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0;
                }
            }
        }
    }
}

/// The running worker. Dropping it asks the thread to end without waiting; [`Worker::stop`]
/// waits a little so the presence is cleared before the app exits.
pub struct Worker {
    mailbox: Arc<Mailbox>,
    thread: Option<JoinHandle<()>>,
    done: Arc<AtomicBool>,
}

impl std::fmt::Debug for Worker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Worker").finish_non_exhaustive()
    }
}

impl Worker {
    /// Start the thread. It tries to connect at once and keeps trying.
    ///
    /// # Errors
    /// The I/O error when the thread cannot start.
    pub fn start(config: Config) -> io::Result<Self> {
        let mailbox = Arc::new(Mailbox::new(MAILBOX_CAPACITY));
        let done = Arc::new(AtomicBool::new(false));
        let (inbox, flag) = (Arc::clone(&mailbox), Arc::clone(&done));
        let thread = thread::Builder::new()
            .name("discord-presence".into())
            .spawn(move || {
                run(&config, &inbox);
                flag.store(true, Ordering::SeqCst);
            })?;
        mailbox.push(Msg::Input(Input::Switch(true)));
        Ok(Self {
            mailbox,
            thread: Some(thread),
            done,
        })
    }

    /// Hand an input to the thread. Never blocks on I/O.
    pub fn send(&self, input: Input) {
        self.mailbox.push(Msg::Input(input));
    }

    /// Clear the presence, close the connection and end the thread, waiting at most `wait` for
    /// it. A thread that is stuck in a call to Discord is left to end by itself.
    pub fn stop(mut self, wait: Duration) {
        self.mailbox.push(Msg::Exit);
        let until = Instant::now() + wait;
        while !self.done.load(Ordering::SeqCst) && Instant::now() < until {
            thread::sleep(Duration::from_millis(5));
        }
        if let Some(thread) = self.thread.take() {
            if self.done.load(Ordering::SeqCst) && thread.join().is_err() {
                tracing::warn!("the Discord presence thread panicked");
            }
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        if self.thread.is_some() {
            self.mailbox.push(Msg::Exit);
        }
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// The one fixed trace event of an operation (criterion 18): no text, no ids.
fn trace(op: &'static str, ok: bool) {
    tracing::debug!(
        target: "nemo_trace",
        provider = "discord",
        op,
        status = if ok { "ok" } else { "err" },
    );
}

struct Runner<'a> {
    config: &'a Config,
    engine: Engine,
    link: Option<Box<dyn Link>>,
    /// The connected state last logged, so only a change is.
    shown: bool,
}

impl Runner<'_> {
    fn feed(&mut self, input: Input) {
        let mut inputs = VecDeque::from([input]);
        while let Some(next) = inputs.pop_front() {
            for action in self.engine.step(Instant::now(), unix_now(), next) {
                if let Some(result) = self.perform(action) {
                    inputs.push_back(result);
                }
            }
        }
    }

    fn perform(&mut self, action: Action) -> Option<Input> {
        match action {
            Action::Connect => Some(self.connect()),
            Action::Set(activity) => self.send(Some(&activity)),
            Action::Clear => self.send(None),
            Action::Disconnect => {
                if let Some(link) = self.link.take() {
                    link.close();
                    self.note_connected(false);
                }
                None
            }
        }
    }

    fn connect(&mut self) -> Input {
        let opened = self.config.connector.open(&self.config.client_id);
        trace("connect", opened.is_ok());
        let ok = opened.is_ok();
        self.link = opened.ok();
        self.note_connected(ok);
        Input::Connected(ok)
    }

    /// Send the activity (or a clear). A lost connection becomes `Connected(false)`.
    fn send(&mut self, activity: Option<&super::activity::Activity>) -> Option<Input> {
        let op = if activity.is_some() {
            "set_activity"
        } else {
            "clear"
        };
        let link = self.link.as_mut()?;
        match link.set_activity(activity) {
            Ok(Incoming::Error) => {
                trace(op, false);
                None
            }
            Ok(_) => {
                trace(op, true);
                None
            }
            Err(_) => {
                trace(op, false);
                self.link = None;
                self.note_connected(false);
                Some(Input::Connected(false))
            }
        }
    }

    fn note_connected(&mut self, connected: bool) {
        if connected != self.shown {
            self.shown = connected;
            tracing::debug!(connected, "Discord presence connection changed");
        }
    }
}

fn run(config: &Config, mailbox: &Mailbox) {
    let mut runner = Runner {
        config,
        engine: Engine::new(config.timing, unix_now()),
        link: None,
        shown: false,
    };
    loop {
        let wait = runner.engine.wake_in(Instant::now());
        match mailbox.pop(wait) {
            Some(Msg::Input(input)) => runner.feed(input),
            Some(Msg::Exit) => {
                runner.feed(Input::Switch(false));
                return;
            }
            None => runner.feed(Input::Tick),
        }
    }
}

#[cfg(test)]
mod tests;
