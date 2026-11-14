//! The OS notification contract (integrations): a toast on Windows, a freedesktop notification
//! on Linux.
//!
//! Every desktop notification goes through one [`OsNotifier`]; no other code path shows one.
//! No I/O here. Callers never block: [`OsNotifier::notify`] only queues the request for the
//! notifier's own worker thread. Notification text is plain and already sanitized, and it is
//! never logged above `debug`.

/// A kind of notification. One notification per group is live; a newer one replaces it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotificationGroup {
    /// New releases of followed artists (account-actions).
    Releases,
}

/// One notification to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsNotification {
    /// Which live notification this replaces.
    pub group: NotificationGroup,
    /// Title line. Plain text, already sanitized.
    pub title: String,
    /// Body text. Plain text, already sanitized.
    pub body: String,
}

/// What a click on a notification asks the app to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationAction {
    /// Raise the window and open the release list.
    OpenReleases,
}

/// Something the user did with a notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationEvent {
    /// The user clicked the notification.
    Activated(NotificationAction),
    /// The notification of this group was closed or expired.
    Closed(NotificationGroup),
}

/// Why notifications cannot be shown. Settings shows the reason in one line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyUnavailable {
    /// No notification service: no desktop session bus on Linux, or no notifier built in.
    NoService,
    /// The app is not installed for notifications (on Windows, no Start Menu shortcut with the
    /// app id).
    NotInstalled,
    /// The OS or the user turned notifications off for this app.
    Refused,
}

/// Whether notifications can be shown now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifierState {
    /// Notifications can be shown.
    Ready,
    /// Notifications cannot be shown, for this reason.
    Unavailable(NotifyUnavailable),
}

/// Why a notification was not queued. The texts hold no notification text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NotifyError {
    /// Notifications cannot be shown, for this reason.
    #[error("notifications are not available")]
    Unavailable(NotifyUnavailable),
    /// The notification service failed.
    #[error("the notification service failed")]
    Service,
}

/// Receives notification events without blocking. The UI polls it once per frame.
pub trait NotificationReceiver: Send {
    /// The next event, or `None` when there is none right now.
    fn try_next(&mut self) -> Option<NotificationEvent>;
}

/// Shows OS notifications. The only way the app shows one.
pub trait OsNotifier: Send + Sync {
    /// Whether notifications can be shown now. Cheap, safe to call every frame.
    fn state(&self) -> NotifierState;

    /// Queue a notification for the notifier's worker thread; it replaces the live one of the
    /// same group. Never blocks. A notification that fails later is dropped, not retried.
    ///
    /// # Errors
    /// [`NotifyError::Unavailable`] when notifications cannot be shown, [`NotifyError::Service`]
    /// when the request cannot be queued.
    fn notify(&self, n: OsNotification) -> Result<(), NotifyError>;

    /// Remove the notification of this group, including from the OS history. Never blocks.
    fn clear(&self, group: NotificationGroup);

    /// A new event receiver. Each receiver gets every event from the moment it is created.
    fn subscribe(&self) -> Box<dyn NotificationReceiver>;
}

/// A notifier with no notification service. Shows nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoNotifier;

/// The receiver of [`NoNotifier`]: no event ever arrives.
struct NoEvents;

impl NotificationReceiver for NoEvents {
    fn try_next(&mut self) -> Option<NotificationEvent> {
        None
    }
}

impl OsNotifier for NoNotifier {
    fn state(&self) -> NotifierState {
        NotifierState::Unavailable(NotifyUnavailable::NoService)
    }

    fn notify(&self, _n: OsNotification) -> Result<(), NotifyError> {
        Err(NotifyError::Unavailable(NotifyUnavailable::NoService))
    }

    fn clear(&self, _group: NotificationGroup) {}

    fn subscribe(&self) -> Box<dyn NotificationReceiver> {
        Box::new(NoEvents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_notifier_is_unavailable_and_refuses_to_notify() {
        assert_eq!(
            NoNotifier.state(),
            NotifierState::Unavailable(NotifyUnavailable::NoService)
        );
        let n = OsNotification {
            group: NotificationGroup::Releases,
            title: "Artist - Title".to_owned(),
            body: "New single".to_owned(),
        };
        assert_eq!(
            NoNotifier.notify(n),
            Err(NotifyError::Unavailable(NotifyUnavailable::NoService))
        );
        NoNotifier.clear(NotificationGroup::Releases);
        assert_eq!(NoNotifier.subscribe().try_next(), None);
    }

    #[test]
    fn error_texts_hold_no_notification_text() {
        let texts = [
            NotifyError::Unavailable(NotifyUnavailable::NotInstalled).to_string(),
            NotifyError::Service.to_string(),
        ];
        assert_eq!(
            texts,
            [
                "notifications are not available",
                "the notification service failed"
            ]
        );
    }
}
