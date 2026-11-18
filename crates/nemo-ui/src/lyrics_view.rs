//! Lyrics view logic (`design.md` section 10, `lyrics-rebuild` spec). Pure: which line is active,
//! where the lines sit and where the scroll offset must be, and the footer text. The two clocks
//! are in [`clock`], type sizes and the lit look in [`metrics`], line slots and the scroll
//! anchor in [`layout`].

use std::time::Duration;

use nemo_core::lyrics::{Granularity, LyricsCredit, LyricsDoc};

pub mod clock;
pub mod layout;
pub mod metrics;
pub mod note;

/// The active line sits at this fraction of the viewport height.
pub const ANCHOR: f32 = 0.35;
/// Manual scrolling pauses auto-follow for this long.
pub const FOLLOW_PAUSE_MS: f32 = 4000.0;
/// Share of the viewport height at the top and at the bottom over which the lines fade out
/// (the soft edge mask).
pub const EDGE_FADE: f32 = 0.2;

/// Opacity a line gets from the soft edge mask: 1 in the middle of the viewport, easing to 0 at
/// the top and bottom edges over [`EDGE_FADE`] of its height. `center_y` is the line's centre,
/// px from the top of the viewport.
#[must_use]
pub fn edge_fade(center_y: f32, viewport_h: f32) -> f32 {
    let zone = (EDGE_FADE * viewport_h).max(1.0);
    let ease = |d: f32| {
        let t = (d / zone).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    ease(center_y) * ease(viewport_h - center_y)
}

/// Text for the sync level in the footer (wave-lyrics criterion 9).
#[must_use]
pub const fn level_text(g: Granularity) -> &'static str {
    match g {
        Granularity::Syllable => "Syllable synced",
        Granularity::Word => "Word synced",
        Granularity::Line => "Synced",
        Granularity::Unsynced => "Not synced",
    }
}

/// Short level tag for source lists (Tools panel, Settings).
#[must_use]
pub const fn level_tag(g: Granularity) -> &'static str {
    match g {
        Granularity::Syllable => "Syllable",
        Granularity::Word => "Word",
        Granularity::Line => "Line",
        Granularity::Unsynced => "Unsynced",
    }
}

/// Index of the active line for a playback position, or `None` before the first line, for
/// unsynced lyrics and for empty documents. A line is active once it has started on the scroll
/// clock ([`clock::Clocks`]): position plus `offset_ms` (a positive offset shows lines earlier),
/// the sync offset of the lyrics level and the 0.5 s scroll lead.
#[must_use]
pub fn active_line(doc: &LyricsDoc, position_ms: i64, offset_ms: i64) -> Option<usize> {
    if doc.granularity == Granularity::Unsynced {
        return None;
    }
    let t = clock::Clocks::new(doc.granularity, position_ms, offset_ms).scroll_ms;
    doc.lines
        .iter()
        .enumerate()
        .take_while(|(_, l)| {
            l.start_ms
                .is_some_and(|s| i64::try_from(s).unwrap_or(i64::MAX) <= t)
        })
        .map(|(i, _)| i)
        .last()
}

/// [`active_line`] at a playback `position`, read in whole milliseconds as the lyrics view
/// reads `PlayerSnapshot.position`. The lyrics view and the editor preview both find their
/// line here, so they agree for the same position and offset (lyrics-plus criterion 5).
#[must_use]
pub fn active_line_at(doc: &LyricsDoc, position: Duration, offset_ms: i64) -> Option<usize> {
    let position_ms = i64::try_from(position.as_millis()).unwrap_or(i64::MAX);
    active_line(doc, position_ms, offset_ms)
}

/// Footer text: `Lyrics: <provider> . <level>` (design.md section 10, wave-lyrics criterion 9).
/// Never hidden.
#[must_use]
pub fn footer(doc: Option<&LyricsDoc>) -> String {
    match doc {
        None => "Lyrics: Unknown".to_owned(),
        Some(d) => {
            let name = d.provider.as_str();
            let name = if name.is_empty() { "Unknown" } else { name };
            format!("Lyrics: {name} \u{b7} {}", level_text(d.granularity))
        }
    }
}

/// The one footer line: [`footer`] with the credit a source asks for merged into it, so the
/// provider is named once. The credit label is left out when it already names the provider;
/// the submitter (`by ...`) and the link host follow.
#[must_use]
pub fn footer_line(doc: Option<&LyricsDoc>, credit: Option<&LyricsCredit>) -> String {
    let mut line = footer(doc);
    let Some(c) = credit else {
        return line;
    };
    let provider = doc.map_or("", |d| d.provider.as_str()).to_lowercase();
    let names_provider = !provider.is_empty() && c.label.to_lowercase().contains(&provider);
    let mut parts = Vec::new();
    if !names_provider && !c.label.trim().is_empty() {
        parts.push(c.label.clone());
    }
    if let Some(by) = &c.by {
        parts.push(format!("by {by}"));
    }
    if let Some(url) = &c.url {
        let host = url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/');
        if !host.is_empty() {
            parts.push(host.to_owned());
        }
    }
    for p in parts {
        line.push_str(" \u{b7} ");
        line.push_str(&p);
    }
    line
}

#[cfg(test)]
mod tests;
