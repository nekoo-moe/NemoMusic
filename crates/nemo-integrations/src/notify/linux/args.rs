//! The arguments of one `Notify` call, and the escaping of the body. Pure, no I/O.

use std::collections::HashMap;

use nemo_core::notify::OsNotification;
use zbus::zvariant::Value;

/// `app_name` of every notification.
pub const APP_NAME: &str = "NemoMusic";
/// Icon name of every notification, from the icon theme (installed by the Linux package).
pub const APP_ICON: &str = "nemomusic";
/// The `desktop-entry` hint: the name of `nemomusic.desktop` without the extension.
pub const DESKTOP_ENTRY: &str = "nemomusic";

/// Urgency "normal" (the spec's byte value 1). Never raised (criterion 37).
const URGENCY_NORMAL: u8 = 1;
/// Let the server pick how long the notification stays.
const EXPIRE_DEFAULT: i32 = -1;
/// The action a click on the notification itself sends.
pub(super) const DEFAULT_ACTION: &str = "default";
const DEFAULT_ACTION_LABEL: &str = "Open";

/// What the server can do (`GetCapabilities`), as far as the notifier cares.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Capabilities {
    /// The server shows actions; only then is the `default` action sent.
    pub(super) actions: bool,
}

impl Capabilities {
    pub(super) fn from_list(list: &[String]) -> Self {
        Self {
            actions: list.iter().any(|c| c == "actions"),
        }
    }
}

/// The arguments of one `Notify` call. Built by [`notify_args`], pure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NotifyArgs {
    pub(super) replaces_id: u32,
    pub(super) summary: String,
    pub(super) body: String,
    pub(super) actions: Vec<&'static str>,
}

/// The `Notify` body in D-Bus order: `(s u s s s as a{sv} i)`.
pub(super) type NotifyBody<'a> = (
    &'static str,
    u32,
    &'static str,
    &'a str,
    &'a str,
    &'a [&'static str],
    HashMap<&'static str, Value<'static>>,
    i32,
);

impl NotifyArgs {
    /// `desktop-entry` and `urgency` (a byte, as the spec requires).
    pub(super) fn hints() -> HashMap<&'static str, Value<'static>> {
        HashMap::from([
            ("desktop-entry", Value::from(DESKTOP_ENTRY)),
            ("urgency", Value::U8(URGENCY_NORMAL)),
        ])
    }

    pub(super) fn body(&self) -> NotifyBody<'_> {
        (
            APP_NAME,
            self.replaces_id,
            APP_ICON,
            &self.summary,
            &self.body,
            &self.actions,
            Self::hints(),
            EXPIRE_DEFAULT,
        )
    }
}

/// The `Notify` arguments for `n`, replacing the notification `replaces_id` (0 for none).
pub(super) fn notify_args(n: &OsNotification, replaces_id: u32, caps: Capabilities) -> NotifyArgs {
    NotifyArgs {
        replaces_id,
        summary: n.title.clone(),
        body: escape_markup(&n.body),
        actions: if caps.actions {
            vec![DEFAULT_ACTION, DEFAULT_ACTION_LABEL]
        } else {
            Vec::new()
        },
    }
}

/// Whether `s` starts with one of the escapes `sanitize` writes (or another XML one).
fn starts_with_escape(s: &str) -> bool {
    ["&amp;", "&lt;", "&gt;", "&quot;", "&apos;"]
        .iter()
        .any(|e| s.starts_with(e))
}

/// Escape `<`, `>` and every `&` that does not already start an escape. Text that
/// `compose::sanitize` wrote passes unchanged; raw markup is escaped.
pub(super) fn escape_markup(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (i, c) in text.char_indices() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' if !text.get(i..).is_some_and(starts_with_escape) => out.push_str("&amp;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use nemo_core::notify::{NotificationGroup, OsNotification};
    use zbus::zvariant::Value;

    use super::{APP_ICON, APP_NAME, Capabilities, DESKTOP_ENTRY, escape_markup, notify_args};
    use crate::notify::compose::sanitize;

    fn note(title: &str, body: &str) -> OsNotification {
        OsNotification {
            group: NotificationGroup::Releases,
            title: title.to_owned(),
            body: body.to_owned(),
        }
    }

    #[test]
    fn args_carry_the_fixed_fields_and_the_action_only_when_listed() {
        let n = note("Artist - Title", "New release");
        let with = notify_args(&n, 7, Capabilities { actions: true });
        assert_eq!(with.replaces_id, 7);
        assert_eq!(with.summary, "Artist - Title");
        assert_eq!(with.body, "New release");
        assert_eq!(with.actions, ["default", "Open"]);
        let without = notify_args(&n, 0, Capabilities::default());
        assert!(without.actions.is_empty());

        let body = with.body();
        assert_eq!((body.0, body.1, body.2), (APP_NAME, 7, APP_ICON));
        assert_eq!(body.7, -1, "the server's own timeout");
        assert_eq!(body.6.len(), 2, "only desktop-entry and urgency");
        assert_eq!(body.6["desktop-entry"], Value::from(DESKTOP_ENTRY));
        assert_eq!(body.6["urgency"], Value::U8(1), "normal, as a byte");
    }

    #[test]
    fn capabilities_read_the_actions_entry() {
        let list = |l: &[&str]| l.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert!(Capabilities::from_list(&list(&["body", "actions"])).actions);
        assert!(!Capabilities::from_list(&list(&["body", "body-markup"])).actions);
    }

    #[test]
    fn the_body_is_always_escaped_and_sanitized_text_is_left_alone() {
        assert_eq!(
            escape_markup("<b>Tom</b> & Jerry"),
            "&lt;b&gt;Tom&lt;/b&gt; &amp; Jerry"
        );
        for raw in [
            "Tom & Jerry <3",
            "<a href=\"x\">link</a>",
            "AT&T &amp; co",
            "plain",
            "",
        ] {
            let clean = sanitize(raw, 200);
            assert_eq!(escape_markup(&clean), clean, "{raw}");
            assert!(!escape_markup(raw).contains('<'), "{raw}");
        }
        let n = note("t", "Tom & Jerry <3");
        assert_eq!(
            notify_args(&n, 0, Capabilities::default()).body,
            "Tom &amp; Jerry &lt;3"
        );
    }
}
