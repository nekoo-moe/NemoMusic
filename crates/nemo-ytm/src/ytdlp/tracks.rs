//! Audio tracks of a video with several (dubbed or described audio) and the choice by the
//! preferred language (account-actions criteria 24 to 27).
//!
//! `yt-dlp` writes one format per track and bitrate. A format of a multi-track video carries
//! `language` (a BCP 47 tag, `-desc` for described audio), `language_preference` (10 for the
//! video's default, original track) and `format_note`, which starts with the track's display
//! name and `(default)` on the default track.

use nemo_core::stream::AudioTrackInfo;
use serde_json::Value;

/// `yt-dlp`'s `language_preference` of the default (original) track.
const DEFAULT_TRACK_PREFERENCE: i64 = 10;

/// The track facts of one format.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct TrackFacts {
    /// The format's language tag, if any.
    pub language: Option<String>,
    /// The format belongs to the video's default (original) track.
    pub is_default: bool,
    /// The track's display name as `yt-dlp` writes it.
    pub label: Option<String>,
}

/// Read the track facts of one `formats[]` entry.
pub(super) fn facts(entry: &Value) -> TrackFacts {
    let language = entry
        .get("language")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|l| !l.is_empty() && *l != "none")
        .map(str::to_owned);
    let note = entry
        .get("format_note")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let is_default = entry
        .get("language_preference")
        .and_then(Value::as_i64)
        .is_some_and(|p| p >= DEFAULT_TRACK_PREFERENCE)
        || note.contains("(default)");
    let label = note
        .split(", ")
        .next()
        .map(|first| first.replace("(default)", "").trim().to_owned())
        .filter(|l| !l.is_empty() && language.is_some());
    TrackFacts {
        language,
        is_default,
        label,
    }
}

/// Whether a format's `language` matches the requested tag: the same tag, or the same primary
/// language (`pt` and `pt-BR`), ignoring ASCII case. Described audio (`-desc`) never matches.
fn matches(format: &str, wanted: &str) -> bool {
    if format.ends_with("-desc") {
        return false;
    }
    let primary = |tag: &str| {
        tag.split(['-', '_'])
            .next()
            .unwrap_or(tag)
            .to_ascii_lowercase()
    };
    format.eq_ignore_ascii_case(wanted) || primary(format) == primary(wanted)
}

/// Which formats may play: with a preferred language, those of the matching track; with none,
/// or when no track matches, those of the default track. When the video marks no default
/// track, every format may play.
pub(super) fn playable(tracks: &[TrackFacts], wanted: Option<&str>) -> Vec<bool> {
    let exact = |t: &TrackFacts, w: &str| {
        t.language
            .as_deref()
            .is_some_and(|l| l.eq_ignore_ascii_case(w) && !l.ends_with("-desc"))
    };
    let close = |t: &TrackFacts, w: &str| t.language.as_deref().is_some_and(|l| matches(l, w));
    if let Some(w) = wanted {
        for rule in [&exact as &dyn Fn(&TrackFacts, &str) -> bool, &close] {
            if tracks.iter().any(|t| rule(t, w)) {
                return tracks.iter().map(|t| rule(t, w)).collect();
            }
        }
    }
    if tracks.iter().any(|t| t.is_default) {
        tracks.iter().map(|t| t.is_default).collect()
    } else {
        vec![true; tracks.len()]
    }
}

/// The distinct tracks among the formats, default first, then in order of appearance. Empty
/// when the video has one track or the formats name no language.
pub(super) fn list(tracks: &[TrackFacts]) -> Vec<AudioTrackInfo> {
    let mut out: Vec<AudioTrackInfo> = Vec::new();
    for t in tracks {
        let Some(language) = &t.language else {
            continue;
        };
        if let Some(seen) = out
            .iter_mut()
            .find(|i| i.language.as_deref() == Some(language.as_str()))
        {
            seen.is_default |= t.is_default;
            continue;
        }
        out.push(AudioTrackInfo {
            language: Some(language.clone()),
            label: t.label.clone().unwrap_or_else(|| language.clone()),
            is_default: t.is_default,
        });
    }
    if out.len() < 2 {
        return Vec::new();
    }
    out.sort_by_key(|i| !i.is_default);
    out
}

#[cfg(test)]
mod tests {
    use super::{TrackFacts, list, matches, playable};

    fn track(language: &str, is_default: bool) -> TrackFacts {
        TrackFacts {
            language: Some(language.to_owned()),
            is_default,
            label: None,
        }
    }

    #[test]
    fn languages_match_by_tag_or_primary_language() {
        assert!(matches("ja", "ja"));
        assert!(matches("pt-BR", "pt"));
        assert!(matches("en", "EN-us"));
        assert!(!matches("en-desc", "en"));
        assert!(!matches("es", "en"));
    }

    #[test]
    fn an_exact_tag_wins_over_a_primary_match() {
        let tracks = [
            track("pt-PT", false),
            track("pt-BR", false),
            track("en", true),
        ];
        assert_eq!(playable(&tracks, Some("pt-BR")), [false, true, false]);
        assert_eq!(playable(&tracks, Some("pt")), [true, true, false]);
    }

    #[test]
    fn no_match_and_automatic_play_the_default_track() {
        let tracks = [track("en", true), track("ja", false)];
        assert_eq!(playable(&tracks, Some("de")), [true, false]);
        assert_eq!(playable(&tracks, None), [true, false]);
        let unmarked = [track("en", false), track("ja", false)];
        assert_eq!(playable(&unmarked, None), [true, true]);
    }

    #[test]
    fn one_track_lists_nothing() {
        assert!(list(&[track("en", true), track("en", true)]).is_empty());
        assert!(list(&[TrackFacts::default()]).is_empty());
    }
}
