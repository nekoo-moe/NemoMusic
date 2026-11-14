//! The runs of a subtitle or row column.

use serde_json::Value;

use super::json::runs_of;
use crate::catalog::parse_year;

fn is_separator(text: &str) -> bool {
    matches!(text, "•" | "·" | "&" | "," | "-" | "–" | "|")
}

fn is_type_label(text: &str) -> bool {
    matches!(
        text,
        "Song"
            | "Video"
            | "Album"
            | "Single"
            | "EP"
            | "Playlist"
            | "Artist"
            | "Podcast"
            | "Episode"
    )
}

fn has_digit(text: &str) -> bool {
    text.chars().any(|c| c.is_ascii_digit())
}

/// What the runs of a subtitle or row column hold.
#[derive(Debug, Default)]
pub(super) struct Runs {
    /// Runs that link to an artist or channel (`UC...`).
    pub(super) artists: Vec<String>,
    /// The run that links to an album (`MPRE...`).
    pub(super) album: Option<String>,
    /// Runs without a link, separators removed.
    pub(super) plain: Vec<String>,
}

impl Runs {
    fn read(runs: &[Value]) -> Self {
        let mut out = Self::default();
        for run in runs {
            let Some(text) = run.get("text").and_then(Value::as_str).map(str::trim) else {
                continue;
            };
            if text.is_empty() || is_separator(text) {
                continue;
            }
            let id = run
                .pointer("/navigationEndpoint/browseEndpoint/browseId")
                .and_then(Value::as_str);
            match id {
                Some(id) if id.starts_with("UC") => out.artists.push(text.to_owned()),
                Some(id) if id.starts_with("MPRE") => out.album = Some(text.to_owned()),
                Some(_) => {}
                None => out.plain.push(text.to_owned()),
            }
        }
        out
    }

    pub(super) fn from_value(value: Option<&Value>) -> Self {
        value.map(|v| Self::read(runs_of(v))).unwrap_or_default()
    }

    /// Plain texts that can be a name: no digits, not a type label.
    pub(super) fn names(&self) -> impl Iterator<Item = &String> {
        self.plain
            .iter()
            .filter(|t| !is_type_label(t) && !has_digit(t))
    }

    /// Artist names: linked ones, else plain name-like texts.
    pub(super) fn artist_names(&self) -> Vec<String> {
        if self.artists.is_empty() {
            self.names().cloned().collect()
        } else {
            self.artists.clone()
        }
    }

    pub(super) fn year_text(&self) -> Option<String> {
        self.plain.iter().find(|t| parse_year(t).is_some()).cloned()
    }
}
