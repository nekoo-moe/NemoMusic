//! The activity Discord shows, and its JSON in the shape the local IPC takes.

use serde_json::{Map, Value, json};

/// The kind of activity, which sets the first words of Discord's card (`Listening to ...`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityKind {
    /// `Playing ...`.
    Playing,
    /// `Listening to ...`.
    Listening,
    /// `Watching ...`.
    Watching,
}

impl ActivityKind {
    /// The number Discord's API uses.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Playing => 0,
            Self::Listening => 2,
            Self::Watching => 3,
        }
    }
}

/// One image with its tooltip. `key` is an asset name of the Discord application or an https
/// address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// Asset name or https address.
    pub key: String,
    /// Tooltip shown on hover.
    pub text: String,
}

/// What Discord shows. Every text is already fitted ([`crate::discord::fit`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity {
    /// The kind.
    pub kind: ActivityKind,
    /// First line.
    pub details: String,
    /// Second line.
    pub state: Option<String>,
    /// Start time, in seconds since the Unix epoch.
    pub start: Option<i64>,
    /// End time, in seconds since the Unix epoch. Discord draws a progress bar when both exist.
    pub end: Option<i64>,
    /// The large picture.
    pub large: Option<Image>,
    /// The small picture.
    pub small: Option<Image>,
}

impl Activity {
    /// The `activity` object of a `SET_ACTIVITY` command.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut out = Map::new();
        out.insert("type".into(), json!(self.kind.code()));
        out.insert("details".into(), json!(self.details));
        if let Some(state) = &self.state {
            out.insert("state".into(), json!(state));
        }
        let mut times = Map::new();
        if let Some(start) = self.start {
            times.insert("start".into(), json!(start));
        }
        if let Some(end) = self.end {
            times.insert("end".into(), json!(end));
        }
        if !times.is_empty() {
            out.insert("timestamps".into(), Value::Object(times));
        }
        let mut assets = Map::new();
        if let Some(large) = &self.large {
            assets.insert("large_image".into(), json!(large.key));
            assets.insert("large_text".into(), json!(large.text));
        }
        if let Some(small) = &self.small {
            assets.insert("small_image".into(), json!(small.key));
            assets.insert("small_text".into(), json!(small.text));
        }
        if !assets.is_empty() {
            out.insert("assets".into(), Value::Object(assets));
        }
        Value::Object(out)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Activity, ActivityKind, Image};

    fn image(key: &str, text: &str) -> Image {
        Image {
            key: key.into(),
            text: text.into(),
        }
    }

    #[test]
    fn a_full_activity_uses_the_discord_field_names() {
        let a = Activity {
            kind: ActivityKind::Listening,
            details: "Title".into(),
            state: Some("Artist".into()),
            start: Some(100),
            end: Some(340),
            large: Some(image("nemo", "NemoMusic")),
            small: Some(image("local", "Local file")),
        };
        assert_eq!(
            a.to_json(),
            json!({
                "type": 2,
                "details": "Title",
                "state": "Artist",
                "timestamps": {"start": 100, "end": 340},
                "assets": {
                    "large_image": "nemo", "large_text": "NemoMusic",
                    "small_image": "local", "small_text": "Local file"
                }
            })
        );
    }

    #[test]
    fn empty_parts_are_left_out() {
        let a = Activity {
            kind: ActivityKind::Playing,
            details: "Idle".into(),
            state: None,
            start: None,
            end: None,
            large: None,
            small: None,
        };
        assert_eq!(a.to_json(), json!({"type": 0, "details": "Idle"}));
    }

    #[test]
    fn kinds_have_the_numbers_discord_uses() {
        assert_eq!(ActivityKind::Playing.code(), 0);
        assert_eq!(ActivityKind::Listening.code(), 2);
        assert_eq!(ActivityKind::Watching.code(), 3);
    }
}
