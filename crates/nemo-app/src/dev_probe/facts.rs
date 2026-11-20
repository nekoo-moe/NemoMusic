//! Facts the probe gathers from settings and the credential store, reduced to what is safe to
//! show: which keys are set, and the size class and names of stored sessions. Never a value of
//! a secret.

use nemo_core::dev::{DevConfigKey, DevLenClass};
use serde_json::Value;

use crate::dev_capture::Redactor;

/// Settings keys whose value is never shown, only that they are set.
const HIDDEN_WORDS: [&str; 12] = [
    "path", "folder", "device", "token", "secret", "key", "url", "endpoint", "password", "cookie",
    "account", "user",
];

/// The longest value shown, in characters.
const VALUE_MAX: usize = 40;

/// The most cookie names listed.
const COOKIE_NAMES_MAX: usize = 24;

/// The settings keys whose value differs from the default, with the value for a plain setting.
pub(super) fn set_keys(current: &Value, default: &Value, redactor: &Redactor) -> Vec<DevConfigKey> {
    let (Some(now), Some(base)) = (current.as_object(), default.as_object()) else {
        return Vec::new();
    };
    now.iter()
        .filter(|(key, value)| base.get(*key) != Some(*value))
        .map(|(key, value)| DevConfigKey {
            key: key.clone(),
            value: shown(key, value, redactor),
        })
        .collect()
}

/// The text of a plain value, or `None` for a hidden key or a structure.
fn shown(key: &str, value: &Value, redactor: &Redactor) -> Option<String> {
    let lower = key.to_ascii_lowercase();
    if HIDDEN_WORDS.iter().any(|word| lower.contains(word)) {
        return None;
    }
    let text = match value {
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Null => "none".to_owned(),
        Value::Array(items) => format!("{} items", items.len()),
        Value::Object(map) => format!("{} entries", map.len()),
    };
    let text = redactor.redact(&text);
    Some(text.chars().take(VALUE_MAX).collect())
}

/// What the stored session of one site looks like from outside.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SecretFacts {
    /// Whether something is stored; `None` when the store could not be read.
    pub(crate) present: Option<bool>,
    /// The size class of the stored value.
    pub(crate) len: DevLenClass,
    /// The names of the cookies in it.
    pub(crate) cookie_names: Vec<String>,
}

impl SecretFacts {
    /// Facts of a stored value, or of an empty store when `stored` is `None`. The value itself
    /// is dropped here: only its length and, for a cookie header, the names are kept.
    pub(crate) fn of(stored: Option<&str>, is_cookie: bool) -> Self {
        let Some(value) = stored else {
            return Self {
                present: Some(false),
                len: DevLenClass::Absent,
                cookie_names: Vec::new(),
            };
        };
        Self {
            present: Some(true),
            len: DevLenClass::of(value.chars().count()),
            cookie_names: if is_cookie {
                cookie_names(value)
            } else {
                Vec::new()
            },
        }
    }
}

/// The names in a `name=value; name=value` cookie header, cut to safe characters.
fn cookie_names(header: &str) -> Vec<String> {
    let mut names: Vec<String> = header
        .split(';')
        .filter_map(|part| part.split_once('=').map(|(name, _)| name.trim()))
        .filter(|name| !name.is_empty())
        .map(|name| {
            name.chars()
                .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
                .take(40)
                .collect::<String>()
        })
        .filter(|name| !name.is_empty())
        .collect();
    names.sort();
    names.dedup();
    names.truncate(COOKIE_NAMES_MAX);
    names
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn only_changed_keys_are_listed_and_secret_looking_ones_are_hidden() {
        let default = json!({"volume": 100, "theme": "nemo", "yt_dlp_path": null, "folders": [], "shelves": []});
        let now = json!({
            "volume": 80,
            "theme": "nemo",
            "yt_dlp_path": "/home/alice/bin/yt-dlp",
            "folders": ["/music"],
            "shelves": ["a"],
            "extra_mode": "fast"
        });
        let keys = set_keys(&now, &default, &Redactor::with_home("/home/alice"));
        let find = |k: &str| keys.iter().find(|c| c.key == k).map(|c| c.value.clone());
        assert_eq!(find("volume"), Some(Some("80".to_owned())));
        assert_eq!(find("yt_dlp_path"), Some(None));
        assert_eq!(find("folders"), Some(None));
        assert_eq!(find("shelves"), Some(Some("1 items".to_owned())));
        assert_eq!(find("extra_mode"), Some(Some("fast".to_owned())));
        assert_eq!(find("theme"), None);
    }

    #[test]
    fn a_shown_value_is_redacted_and_cut() {
        let default = json!({"label": ""});
        let now = json!({"label": format!("mail me@example.com {}", "x".repeat(100))});
        let keys = set_keys(&now, &default, &Redactor::with_home("/home/alice"));
        let value = keys[0].value.clone().expect("shown");
        assert!(value.starts_with("mail [email]"));
        assert!(value.chars().count() <= VALUE_MAX);
    }

    #[test]
    fn a_stored_session_gives_a_size_class_and_cookie_names_only() {
        let header =
            "SAPISID=abc123SECRET; __Secure-3PSID=g.a000zzzSECRET2 ; empty=; =novalue; HSID=x";
        let facts = SecretFacts::of(Some(header), true);
        assert_eq!(facts.present, Some(true));
        assert_eq!(facts.len, DevLenClass::Medium);
        assert_eq!(
            facts.cookie_names,
            vec!["HSID", "SAPISID", "__Secure-3PSID", "empty"]
        );
        let text = format!("{facts:?}");
        assert!(!text.contains("SECRET"), "{text}");
    }

    #[test]
    fn a_token_gives_only_its_size_class() {
        let facts = SecretFacts::of(Some("2-292593-1234567890-AbCdEfGhIj"), false);
        assert_eq!(facts.len, DevLenClass::Short);
        assert!(facts.cookie_names.is_empty());
        let none = SecretFacts::of(None, false);
        assert_eq!((none.present, none.len), (Some(false), DevLenClass::Absent));
    }
}
