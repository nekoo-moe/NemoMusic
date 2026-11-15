//! Desktop notifications on Linux: `org.freedesktop.Notifications` on the session bus, through
//! blocking `zbus` (criteria 31, 35, 37 to 40). No host is contacted; the session bus is local.
//!
//! One worker thread (`nemo-notify`) owns the bus connection. It asks `GetCapabilities` once,
//! then serves the requests of [`LinuxNotifier`] in order: `Notify` with `replaces_id` set to
//! the live notification of the group (so a newer one replaces it), `desktop-entry`
//! `nemomusic`, urgency normal, the server's default timeout, no sound of its own, and the
//! `default` action only when the server lists `actions`. `clear` calls `CloseNotification`.
//!
//! The signals `ActionInvoked` and `NotificationClosed` are read by a second thread
//! (`nemo-notify-events`), because a blocking `zbus` signal iterator cannot wait on the request
//! channel at the same time. It only forwards each signal to the worker, which owns every
//! notification id and turns a `default` action into [`NotificationEvent::Activated`].
//!
//! Text: the caller passes text that `compose::sanitize` already cleaned and escaped. The body
//! is escaped again here in a way that leaves those escapes alone, so a body is always escaped
//! whether or not the server lists `body-markup`. The summary is plain text in the freedesktop
//! spec (markup is defined for the body only), so its escapes are undone; control characters
//! stay removed. Notification text is never logged, only its length at `debug`.
//!
//! No session bus, or no notification server on it, gives
//! `Unavailable(NotifyUnavailable::NoService)`. A failed `Notify` is dropped, not retried.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use nemo_core::notify::{
    NotificationAction, NotificationEvent, NotificationGroup, NotificationReceiver, NotifierState,
    NotifyError, NotifyUnavailable, OsNotification, OsNotifier,
};
use zbus::blocking::{Connection, MessageIterator};

pub use self::args::{APP_ICON, APP_NAME, DESKTOP_ENTRY};
use self::args::{Capabilities, DEFAULT_ACTION, notify_args};

mod args;

const DEST: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
const IFACE: &str = "org.freedesktop.Notifications";
/// Requests waiting for the worker. A full queue refuses, it never blocks.
const QUEUE: usize = 32;
/// How long [`LinuxNotifier::start`] waits for the bus before it returns.
const CONNECT_WAIT: Duration = Duration::from_secs(2);
/// The longest one call to the notification server may take.
const CALL_TIMEOUT: Duration = Duration::from_secs(5);

const CONNECTING: u8 = 0;
const READY: u8 = 1;
const NO_SERVICE: u8 = 2;

/// A signal of the notification server, forwarded to the worker.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Signal {
    ActionInvoked { id: u32, key: String },
    Closed { id: u32 },
}

enum Request {
    Show(OsNotification),
    Clear(NotificationGroup),
    Signal(Signal),
    Stop,
}

/// State shared by the notifier and its worker.
#[derive(Default)]
struct Shared {
    state: AtomicU8,
    subscribers: Mutex<Vec<Sender<NotificationEvent>>>,
}

impl Shared {
    fn set_state(&self, state: u8) {
        self.state.store(state, Ordering::Release);
    }

    fn broadcast(&self, event: NotificationEvent) {
        let mut subs = self
            .subscribers
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        subs.retain(|s| s.send(event).is_ok());
    }
}

/// The click action of a group.
const fn action_of(group: NotificationGroup) -> NotificationAction {
    match group {
        NotificationGroup::Releases => NotificationAction::OpenReleases,
    }
}

/// The worker's bus connection and the live notification id of each group.
struct Session {
    conn: Connection,
    caps: Capabilities,
    live: HashMap<NotificationGroup, u32>,
}

impl Session {
    /// Connect, start the signal thread, and ask the capabilities. `None` when there is no bus
    /// or no notification server on it.
    fn open(
        connect: impl FnOnce() -> zbus::Result<Connection>,
        signals: SyncSender<Request>,
    ) -> Option<Self> {
        let conn = connect().ok()?;
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface(IFACE)
            .ok()?
            .path(PATH)
            .ok()?
            .build();
        // Subscribe before the first `Notify`, so no signal of ours is missed.
        let events = MessageIterator::for_match_rule(rule, &conn, Some(QUEUE)).ok()?;
        let caps: Vec<String> = conn
            .call_method(Some(DEST), PATH, Some(IFACE), "GetCapabilities", &())
            .and_then(|reply| reply.body().deserialize())
            .ok()?;
        thread::Builder::new()
            .name("nemo-notify-events".to_owned())
            .spawn(move || forward_signals(events, &signals))
            .ok()?;
        Some(Self {
            conn,
            caps: Capabilities::from_list(&caps),
            live: HashMap::new(),
        })
    }

    fn show(&mut self, n: &OsNotification) {
        let replaces = self.live.get(&n.group).copied().unwrap_or(0);
        let args = notify_args(n, replaces, self.caps);
        let shown = self
            .conn
            .call_method(Some(DEST), PATH, Some(IFACE), "Notify", &args.body())
            .and_then(|reply| reply.body().deserialize::<u32>());
        if let Ok(id) = shown {
            self.live.insert(n.group, id);
            tracing::debug!(
                title_len = n.title.chars().count(),
                body_len = n.body.chars().count(),
                "notification shown"
            );
        } else {
            // The server's error text is not logged: it may quote the notification.
            tracing::debug!("the notification server refused a notification");
        }
    }

    fn clear(&mut self, group: NotificationGroup) {
        let Some(id) = self.live.remove(&group) else {
            return;
        };
        let closed =
            self.conn
                .call_method(Some(DEST), PATH, Some(IFACE), "CloseNotification", &(id,));
        if closed.is_err() {
            tracing::debug!("the notification server did not close a notification");
        }
    }

    fn group_of(&self, id: u32) -> Option<NotificationGroup> {
        self.live
            .iter()
            .find_map(|(group, live)| (*live == id).then_some(*group))
    }

    fn on_signal(&mut self, signal: Signal, shared: &Shared) {
        match signal {
            Signal::ActionInvoked { id, key } if key == DEFAULT_ACTION => {
                if let Some(group) = self.group_of(id) {
                    shared.broadcast(NotificationEvent::Activated(action_of(group)));
                }
            }
            Signal::ActionInvoked { .. } => {}
            Signal::Closed { id } => {
                if let Some(group) = self.group_of(id) {
                    self.live.remove(&group);
                    shared.broadcast(NotificationEvent::Closed(group));
                }
            }
        }
    }
}

/// Read the server's signals and hand them to the worker. Ends when the connection or the
/// worker ends.
fn forward_signals(events: MessageIterator, worker: &SyncSender<Request>) {
    for msg in events {
        let Ok(msg) = msg else { break };
        let header = msg.header();
        let signal = match header.member().map(zbus::names::MemberName::as_str) {
            Some("ActionInvoked") => msg
                .body()
                .deserialize::<(u32, String)>()
                .ok()
                .map(|(id, key)| Signal::ActionInvoked { id, key }),
            Some("NotificationClosed") => msg
                .body()
                .deserialize::<(u32, u32)>()
                .ok()
                .map(|(id, _reason)| Signal::Closed { id }),
            _ => None,
        };
        if let Some(signal) = signal
            && worker.send(Request::Signal(signal)).is_err()
        {
            break;
        }
    }
}

fn run_worker(
    connect: impl FnOnce() -> zbus::Result<Connection>,
    requests: &Receiver<Request>,
    signals: SyncSender<Request>,
    shared: &Shared,
    started: &Sender<()>,
) {
    let Some(mut session) = Session::open(connect, signals) else {
        tracing::debug!("no desktop notification service on the session bus");
        shared.set_state(NO_SERVICE);
        // `start` may have stopped waiting; nobody to tell then.
        let _ = started.send(());
        return;
    };
    shared.set_state(READY);
    let _ = started.send(());
    while let Ok(request) = requests.recv() {
        match request {
            Request::Show(n) => session.show(&n),
            Request::Clear(group) => session.clear(group),
            Request::Signal(signal) => session.on_signal(signal, shared),
            Request::Stop => break,
        }
    }
    shared.set_state(NO_SERVICE);
    // Closing ends the signal thread's iterator too. A failure means it is already closed.
    let _ = session.conn.close();
}

/// The Linux `OsNotifier`: freedesktop notifications on the session bus.
pub struct LinuxNotifier {
    requests: SyncSender<Request>,
    shared: Arc<Shared>,
}

impl std::fmt::Debug for LinuxNotifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinuxNotifier")
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}

impl LinuxNotifier {
    /// Start the notifier on the user's session bus. Waits at most 2 seconds for the bus; until
    /// the worker is connected, [`OsNotifier::state`] reports `Unavailable(NoService)`.
    #[must_use]
    pub fn start() -> Self {
        Self::start_with(|| {
            zbus::blocking::connection::Builder::session()?
                .method_timeout(CALL_TIMEOUT)
                .build()
        })
    }

    /// Start the notifier with a connection made by `connect` on the worker thread.
    fn start_with(connect: impl FnOnce() -> zbus::Result<Connection> + Send + 'static) -> Self {
        let (requests, inbox) = mpsc::sync_channel(QUEUE);
        let shared = Arc::new(Shared::default());
        shared.set_state(CONNECTING);
        let (started, wait) = mpsc::channel();
        let worker_shared = Arc::clone(&shared);
        let signals = requests.clone();
        let spawned = thread::Builder::new()
            .name("nemo-notify".to_owned())
            .spawn(move || run_worker(connect, &inbox, signals, &worker_shared, &started));
        match spawned {
            // A timeout leaves the state at "connecting"; the worker sets it later.
            Ok(_) => drop(wait.recv_timeout(CONNECT_WAIT)),
            Err(_) => shared.set_state(NO_SERVICE),
        }
        Self { requests, shared }
    }
}

impl Drop for LinuxNotifier {
    fn drop(&mut self) {
        // Never blocks: with the worker gone there is nothing to stop, and with its queue full
        // it is still busy and ends with the process.
        let _ = self.requests.try_send(Request::Stop);
    }
}

/// A subscriber's end of the event broadcast.
struct Events(Receiver<NotificationEvent>);

impl NotificationReceiver for Events {
    fn try_next(&mut self) -> Option<NotificationEvent> {
        self.0.try_recv().ok()
    }
}

impl OsNotifier for LinuxNotifier {
    fn state(&self) -> NotifierState {
        match self.shared.state.load(Ordering::Acquire) {
            READY => NotifierState::Ready,
            _ => NotifierState::Unavailable(NotifyUnavailable::NoService),
        }
    }

    fn notify(&self, n: OsNotification) -> Result<(), NotifyError> {
        if let NotifierState::Unavailable(why) = self.state() {
            return Err(NotifyError::Unavailable(why));
        }
        self.requests
            .try_send(Request::Show(n))
            .map_err(|e| match e {
                TrySendError::Full(_) => NotifyError::Service,
                TrySendError::Disconnected(_) => {
                    NotifyError::Unavailable(NotifyUnavailable::NoService)
                }
            })
    }

    fn clear(&self, group: NotificationGroup) {
        // A full queue or a stopped worker means nothing can be closed now; clearing is
        // best effort (criterion 39, "where the OS allows it").
        let _ = self.requests.try_send(Request::Clear(group));
    }

    fn subscribe(&self) -> Box<dyn NotificationReceiver> {
        let (tx, rx) = mpsc::channel();
        self.shared
            .subscribers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(tx);
        Box::new(Events(rx))
    }
}

#[cfg(test)]
mod tests;
