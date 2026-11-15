//! Tests for the Linux notifier against a fake `org.freedesktop.Notifications` server on a
//! private `dbus-daemon` that each test starts and kills. No desktop session and no real
//! notification daemon is used.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nemo_core::notify::{
    NotificationAction, NotificationEvent, NotificationGroup, NotificationReceiver, NotifierState,
    NotifyError, NotifyUnavailable, OsNotification, OsNotifier,
};
use zbus::blocking::Connection;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::OwnedValue;

use super::{IFACE, LinuxNotifier, PATH};
use crate::notify::compose::sanitize;

/// Generous: the worker answers in milliseconds, a loaded machine may take longer.
const DEADLINE: Duration = Duration::from_secs(10);

fn note(title: &str, body: &str) -> OsNotification {
    OsNotification {
        group: NotificationGroup::Releases,
        title: title.to_owned(),
        body: body.to_owned(),
    }
}

#[test]
fn no_session_bus_means_no_service() {
    let notifier = LinuxNotifier::start_with(|| {
        zbus::blocking::connection::Builder::address("unix:path=/nonexistent/nemo-test-bus")?
            .build()
    });
    assert_eq!(
        notifier.state(),
        NotifierState::Unavailable(NotifyUnavailable::NoService)
    );
    assert_eq!(
        notifier.notify(note("a", "b")),
        Err(NotifyError::Unavailable(NotifyUnavailable::NoService))
    );
    notifier.clear(NotificationGroup::Releases);
    assert_eq!(notifier.subscribe().try_next(), None);
}

/// A private `dbus-daemon`, killed on drop.
struct PrivateBus {
    child: Child,
    address: String,
}

impl PrivateBus {
    /// `None` when `dbus-daemon` is not installed; the caller then skips the test.
    fn start() -> Option<Self> {
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let mut line = String::new();
        let stdout = child.stdout.take().expect("piped stdout");
        BufReader::new(stdout)
            .read_line(&mut line)
            .expect("bus address");
        Some(Self {
            child,
            address: line.trim().to_owned(),
        })
    }

    fn connect(&self) -> Connection {
        zbus::blocking::connection::Builder::address(self.address.as_str())
            .expect("address")
            .build()
            .expect("connect")
    }

    fn notifier(&self) -> LinuxNotifier {
        let address = self.address.clone();
        LinuxNotifier::start_with(move || {
            zbus::blocking::connection::Builder::address(address.as_str())?.build()
        })
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        // The daemon may have exited already.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One `Notify` call as the fake server received it.
#[derive(Debug, Clone)]
struct NotifyCall {
    app_name: String,
    replaces_id: u32,
    app_icon: String,
    summary: String,
    body: String,
    actions: Vec<String>,
    hints: HashMap<String, OwnedValue>,
    expire_timeout: i32,
}

#[derive(Default)]
struct Calls {
    notify: Vec<NotifyCall>,
    closed: Vec<u32>,
}

/// A fake notification server that records every call.
struct FakeServer {
    caps: Vec<String>,
    calls: Arc<Mutex<Calls>>,
    next_id: AtomicU32,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl FakeServer {
    fn get_capabilities(&self) -> Vec<String> {
        self.caps.clone()
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the D-Bus method has eight arguments"
    )]
    fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        let id = if replaces_id == 0 {
            self.next_id.fetch_add(1, Ordering::SeqCst)
        } else {
            replaces_id
        };
        self.calls.lock().expect("lock").notify.push(NotifyCall {
            app_name,
            replaces_id,
            app_icon,
            summary,
            body,
            actions,
            hints,
            expire_timeout,
        });
        id
    }

    async fn close_notification(
        &self,
        id: u32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        self.calls.lock().expect("lock").closed.push(id);
        // Reason 3: closed by a call to CloseNotification.
        Self::notification_closed(&emitter, id, 3).await?;
        Ok(())
    }

    #[zbus(signal)]
    async fn notification_closed(
        emitter: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;
}

/// Serve a fake with these capabilities on `bus`. The returned connection keeps it alive.
fn serve(bus: &PrivateBus, caps: &[&str]) -> (Connection, Arc<Mutex<Calls>>) {
    let calls = Arc::new(Mutex::new(Calls::default()));
    let fake = FakeServer {
        caps: caps.iter().map(|c| (*c).to_owned()).collect(),
        calls: Arc::clone(&calls),
        next_id: AtomicU32::new(41),
    };
    let conn = zbus::blocking::connection::Builder::address(bus.address.as_str())
        .expect("address")
        .name("org.freedesktop.Notifications")
        .expect("name")
        .serve_at(PATH, fake)
        .expect("serve")
        .build()
        .expect("fake server");
    (conn, calls)
}

/// Wait until `done` holds, or fail after [`DEADLINE`].
fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < DEADLINE, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn next_event(events: &mut dyn NotificationReceiver) -> NotificationEvent {
    let mut got = None;
    wait_until("an event", || {
        got = events.try_next();
        got.is_some()
    });
    got.expect("checked above")
}

fn emit(server: &Connection, member: &str, body: &(u32, &str)) {
    server
        .emit_signal(None::<&str>, PATH, IFACE, member, body)
        .expect("emit");
}

#[test]
fn shows_replaces_and_clears_on_a_fake_server() {
    let Some(bus) = PrivateBus::start() else {
        return; // dbus-daemon is not installed: not run.
    };
    let (_server, calls) = serve(&bus, &["actions", "body", "body-markup"]);
    let notifier = bus.notifier();
    wait_until("the notifier", || notifier.state() == NotifierState::Ready);

    notifier
        .notify(note("Tom &amp; Jerry - Live", "Tom & Jerry <3"))
        .expect("queued");
    wait_until("the first Notify", || {
        calls.lock().expect("lock").notify.len() == 1
    });
    notifier
        .notify(note("2 new releases", "A, B"))
        .expect("queued");
    wait_until("the second Notify", || {
        calls.lock().expect("lock").notify.len() == 2
    });
    notifier.clear(NotificationGroup::Releases);
    wait_until("CloseNotification", || {
        !calls.lock().expect("lock").closed.is_empty()
    });

    let calls = calls.lock().expect("lock");
    let first = &calls.notify[0];
    assert_eq!(first.app_name, "NemoMusic");
    assert_eq!(first.replaces_id, 0);
    assert_eq!(first.app_icon, "nemomusic");
    assert_eq!(
        first.summary, "Tom & Jerry - Live",
        "the summary is plain text"
    );
    assert_eq!(first.body, "Tom &amp; Jerry &lt;3");
    assert_eq!(first.actions, ["default", "Open"]);
    assert_eq!(first.expire_timeout, -1);
    assert_eq!(first.hints.len(), 2);
    assert_eq!(
        first.hints["urgency"].downcast_ref::<u8>().expect("a byte"),
        1
    );
    assert_eq!(
        first.hints["desktop-entry"]
            .downcast_ref::<String>()
            .expect("a string"),
        "nemomusic"
    );
    assert_eq!(calls.notify[1].replaces_id, 41, "replaces the live one");
    assert_eq!(calls.closed, [41]);
}

#[test]
fn a_click_and_a_close_become_events() {
    let Some(bus) = PrivateBus::start() else {
        return; // dbus-daemon is not installed: not run.
    };
    let (server, calls) = serve(&bus, &["actions"]);
    let notifier = bus.notifier();
    wait_until("the notifier", || notifier.state() == NotifierState::Ready);
    let mut events = notifier.subscribe();
    notifier.notify(note("a", "b")).expect("queued");
    wait_until("Notify", || calls.lock().expect("lock").notify.len() == 1);

    // Ignored: another action key, and an id that is not ours. Signals from one sender keep
    // their order, so the first event must come from the third signal.
    emit(&server, "ActionInvoked", &(41, "other"));
    emit(&server, "ActionInvoked", &(999, "default"));
    emit(&server, "ActionInvoked", &(41, "default"));
    assert_eq!(
        next_event(events.as_mut()),
        NotificationEvent::Activated(NotificationAction::OpenReleases)
    );

    let closed = (41_u32, 2_u32); // reason 2: dismissed by the user
    server
        .emit_signal(None::<&str>, PATH, IFACE, "NotificationClosed", &closed)
        .expect("emit");
    assert_eq!(
        next_event(events.as_mut()),
        NotificationEvent::Closed(NotificationGroup::Releases)
    );

    // Closed means not live: the next one is new, not a replacement, and `clear` has nothing to
    // close.
    notifier.notify(note("c", "d")).expect("queued");
    wait_until("Notify", || calls.lock().expect("lock").notify.len() == 2);
    assert_eq!(calls.lock().expect("lock").notify[1].replaces_id, 0);
}

#[test]
fn without_the_actions_capability_no_action_is_sent() {
    let Some(bus) = PrivateBus::start() else {
        return; // dbus-daemon is not installed: not run.
    };
    let (_server, calls) = serve(&bus, &["body"]);
    let notifier = bus.notifier();
    wait_until("the notifier", || notifier.state() == NotifierState::Ready);
    notifier.notify(note("a", "b")).expect("queued");
    wait_until("Notify", || calls.lock().expect("lock").notify.len() == 1);
    assert!(calls.lock().expect("lock").notify[0].actions.is_empty());
}

#[test]
fn a_bus_without_a_notification_server_means_no_service() {
    let Some(bus) = PrivateBus::start() else {
        return; // dbus-daemon is not installed: not run.
    };
    let _other_client = bus.connect();
    let notifier = bus.notifier();
    assert_eq!(
        notifier.state(),
        NotifierState::Unavailable(NotifyUnavailable::NoService)
    );
    assert!(notifier.notify(note("a", "b")).is_err());
}

/// Manual check on a real desktop (task 33, GNOME and KDE): one notification with the text
/// `Tom & Jerry <3` appears, is replaced by a second one after 3 seconds, and is closed after
/// 3 more. Run with `cargo test -p nemo-integrations real_daemon -- --ignored`.
#[test]
#[ignore = "manual: needs a desktop notification daemon on the session bus"]
fn real_daemon_shows_replaces_and_clears() {
    let notifier = LinuxNotifier::start();
    assert_eq!(notifier.state(), NotifierState::Ready);
    let text = sanitize("Tom & Jerry <3", 200);
    notifier
        .notify(note(&sanitize("NemoMusic test", 100), &text))
        .expect("queued");
    std::thread::sleep(Duration::from_secs(3));
    notifier
        .notify(note(&sanitize("NemoMusic test (replaced)", 100), &text))
        .expect("queued");
    std::thread::sleep(Duration::from_secs(3));
    notifier.clear(NotificationGroup::Releases);
    std::thread::sleep(Duration::from_secs(1));
}
