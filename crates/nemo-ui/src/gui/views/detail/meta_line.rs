//! The two meta lines under the title of an album or playlist page, written like YouTube Music
//! (collection-sort-meta spec): `Playlist • Public • 2026`, then `411 views • 249 tracks •
//! 13 hours`. A fact the page does not give is left out, never guessed.

use std::time::Duration;

use nemo_core::catalog::{CollectionMeta, Privacy};
use nemo_core::track::Track;

const DOT: &str = " \u{2022} ";

/// The word a privacy is shown as.
#[must_use]
pub const fn privacy_label(p: Privacy) -> &'static str {
    match p {
        Privacy::Public => "Public",
        Privacy::Unlisted => "Unlisted",
        Privacy::Private => "Private",
    }
}

/// `Playlist • Public • 2026`; the kind alone when nothing else is known.
#[must_use]
pub fn kind_line(kind: &str, privacy: Option<Privacy>, year: Option<u16>) -> String {
    let mut parts = vec![kind.to_owned()];
    if let Some(p) = privacy {
        parts.push(privacy_label(p).to_owned());
    }
    if let Some(y) = year {
        parts.push(y.to_string());
    }
    parts.join(DOT)
}

/// A count the way YouTube shortens it: `411`, `4.3K`, `437K`, `1.2M`, `3B`.
#[must_use]
pub fn short_count(n: u64) -> String {
    let (unit, div) = match n {
        0..1_000 => return n.to_string(),
        1_000..1_000_000 => ("K", 1_000),
        1_000_000..1_000_000_000 => ("M", 1_000_000),
        _ => ("B", 1_000_000_000),
    };
    let whole = n / div;
    let tenth = (n % div) * 10 / div;
    if whole < 10 && tenth > 0 {
        format!("{whole}.{tenth}{unit}")
    } else {
        format!("{whole}{unit}")
    }
}

/// `43 min` or `1 h 5 min` for a summed length.
fn total_text(total: Duration) -> String {
    let mins = total.as_secs().div_ceil(60);
    if mins >= 60 {
        format!("{} h {} min", mins / 60, mins % 60)
    } else {
        format!("{mins} min")
    }
}

/// `411 views • 249 tracks • 13 hours` (`songs` on an album page). The count is the one the
/// header gives, else the songs read. The length is the header's text; without it the songs are
/// summed, but only when every song of the page was read (a cut list would understate it).
#[must_use]
pub fn count_line(meta: &CollectionMeta, tracks: &[Track], album: bool) -> String {
    let mut parts = Vec::new();
    if let Some(v) = meta.view_count {
        parts.push(if v == 1 {
            "1 view".to_owned()
        } else {
            format!("{} views", short_count(v))
        });
    }
    let count = meta.track_count.map_or(tracks.len() as u64, u64::from);
    let word = match (album, count == 1) {
        (true, true) => "song",
        (true, false) => "songs",
        (false, true) => "track",
        (false, false) => "tracks",
    };
    parts.push(format!("{count} {word}"));
    let complete = meta
        .track_count
        .is_none_or(|n| usize::try_from(n).is_ok_and(|n| n <= tracks.len()));
    let total: Duration = tracks.iter().filter_map(|t| t.duration).sum();
    if let Some(text) = &meta.duration_text {
        parts.push(text.clone());
    } else if complete && !total.is_zero() {
        parts.push(total_text(total));
    }
    parts.join(DOT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock;

    #[test]
    fn the_kind_line_shows_what_is_known() {
        assert_eq!(
            kind_line("Playlist", Some(Privacy::Public), Some(2026)),
            "Playlist \u{2022} Public \u{2022} 2026"
        );
        assert_eq!(kind_line("Album", None, Some(2019)), "Album \u{2022} 2019");
        assert_eq!(kind_line("Playlist", None, None), "Playlist");
    }

    #[test]
    fn counts_are_shortened_like_youtube() {
        assert_eq!(short_count(411), "411");
        assert_eq!(short_count(4_300), "4.3K");
        assert_eq!(short_count(437_000), "437K");
        assert_eq!(short_count(1_200_000), "1.2M");
        assert_eq!(short_count(3_000_000_000), "3B");
    }

    #[test]
    fn the_count_line_prefers_the_header_and_never_guesses() {
        let tracks: Vec<Track> = mock::tracks().into_iter().take(4).collect();
        let full = CollectionMeta {
            view_count: Some(411),
            track_count: Some(249),
            duration_text: Some("13+ hours".to_owned()),
            ..CollectionMeta::default()
        };
        assert_eq!(
            count_line(&full, &tracks, false),
            "411 views \u{2022} 249 tracks \u{2022} 13+ hours"
        );
        // No header facts: the songs read, and their summed length.
        let none = CollectionMeta::default();
        let line = count_line(&none, &tracks, true);
        assert!(line.starts_with("4 songs \u{2022} "), "{line}");
        assert!(line.ends_with(" min"), "{line}");
        // The header says 249 but only 4 were read: no summed length (it would be too short).
        let cut = CollectionMeta {
            track_count: Some(249),
            ..CollectionMeta::default()
        };
        assert_eq!(count_line(&cut, &tracks, false), "249 tracks");
        assert_eq!(
            count_line(&none, &tracks[..1], true).split(DOT).next(),
            Some("1 song")
        );
        let mut silent = tracks[0].clone();
        silent.duration = None;
        assert_eq!(count_line(&none, &[silent], false), "1 track");
    }
}
