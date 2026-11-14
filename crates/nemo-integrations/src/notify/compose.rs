//! The plain text of a release notification (criteria 33 and 36). Pure, no I/O.
//!
//! A notification carries no cover, lyrics, token or account id: only artist names and
//! release titles, cleaned by [`sanitize`].

use nemo_core::catalog::AlbumSummary;
use nemo_core::notify::{NotificationGroup, OsNotification};
use nemo_core::releases::ReleaseNotice;

/// The most characters of a notification title.
pub const TITLE_MAX_CHARS: usize = 100;
/// The most characters of a notification body.
pub const BODY_MAX_CHARS: usize = 200;
/// The most artist names a notification of several releases lists.
pub const MAX_ARTISTS: usize = 3;

const UNKNOWN_ARTIST: &str = "Unknown artist";

/// The notification of one release check: "Artist - Title" for one release, "N new releases"
/// with up to [`MAX_ARTISTS`] artist names for several, `None` for none.
#[must_use]
pub fn release_notification(notices: &[ReleaseNotice]) -> Option<OsNotification> {
    let (title, body) = match notices {
        [] => return None,
        [one] => (
            format!("{} - {}", artists_of(&one.album), one.album.title),
            "New release".to_owned(),
        ),
        several => (
            format!("{} new releases", several.len()),
            first_artists(several).join(", "),
        ),
    };
    Some(OsNotification {
        group: NotificationGroup::Releases,
        title: sanitize(&title, TITLE_MAX_CHARS),
        body: sanitize(&body, BODY_MAX_CHARS),
    })
}

/// The artist names of an album, joined, or "Unknown artist" when it names none.
fn artists_of(album: &AlbumSummary) -> String {
    if album.artists.is_empty() {
        UNKNOWN_ARTIST.to_owned()
    } else {
        album.artists.join(", ")
    }
}

/// The first artist of each notice, each name once, in order, at most [`MAX_ARTISTS`].
fn first_artists(notices: &[ReleaseNotice]) -> Vec<&str> {
    let mut names: Vec<&str> = Vec::new();
    for notice in notices {
        let name = notice
            .album
            .artists
            .first()
            .map_or(UNKNOWN_ARTIST, String::as_str);
        if !names.contains(&name) {
            names.push(name);
        }
        if names.len() == MAX_ARTISTS {
            break;
        }
    }
    names
}

/// Plain notification text: line breaks and tabs become spaces, other control characters are
/// removed, the text is cut at `max_chars` characters (the last one an ellipsis), then `&`, `<`
/// and `>` are escaped so no notification server reads markup.
#[must_use]
pub fn sanitize(text: &str, max_chars: usize) -> String {
    let plain: String = text
        .chars()
        .filter_map(|c| match c {
            '\n' | '\r' | '\t' => Some(' '),
            c if c.is_control() => None,
            c => Some(c),
        })
        .collect();
    let plain = plain.trim();
    let cut: String = if plain.chars().count() > max_chars {
        let mut s: String = plain.chars().take(max_chars.saturating_sub(1)).collect();
        s.push('\u{2026}');
        s
    } else {
        plain.to_owned()
    };
    let mut out = String::with_capacity(cut.len());
    for c in cut.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use nemo_core::catalog::{AlbumSummary, BrowseId};
    use nemo_core::notify::NotificationGroup;
    use nemo_core::releases::ReleaseNotice;

    use super::{BODY_MAX_CHARS, TITLE_MAX_CHARS, release_notification, sanitize};

    fn notice(artist: &str, title: &str) -> ReleaseNotice {
        ReleaseNotice {
            album: AlbumSummary {
                id: BrowseId(format!("album-{title}")),
                title: title.to_owned(),
                artists: if artist.is_empty() {
                    Vec::new()
                } else {
                    vec![artist.to_owned()]
                },
                year: Some(2026),
                thumbnails: Vec::new(),
            },
            found_at: SystemTime::UNIX_EPOCH,
            seen: false,
        }
    }

    #[test]
    fn no_release_gives_no_notification() {
        assert_eq!(release_notification(&[]), None);
    }

    #[test]
    fn one_release_shows_artist_and_title() {
        let n = release_notification(&[notice("Lam Khuê", "Đêm Mưa")]).expect("one notice");
        assert_eq!(n.group, NotificationGroup::Releases);
        assert_eq!(n.title, "Lam Khuê - Đêm Mưa");
        assert_eq!(n.body, "New release");
        let unknown = release_notification(&[notice("", "Demo")]).expect("one notice");
        assert_eq!(unknown.title, "Unknown artist - Demo");
    }

    #[test]
    fn several_releases_show_the_count_and_up_to_three_artists() {
        let two = [notice("A", "1"), notice("B", "2")];
        let n = release_notification(&two).expect("two notices");
        assert_eq!(
            (n.title.as_str(), n.body.as_str()),
            ("2 new releases", "A, B")
        );
        let five = [
            notice("A", "1"),
            notice("A", "2"),
            notice("B", "3"),
            notice("C", "4"),
            notice("D", "5"),
        ];
        let n = release_notification(&five).expect("five notices");
        assert_eq!(
            (n.title.as_str(), n.body.as_str()),
            ("5 new releases", "A, B, C")
        );
    }

    #[test]
    fn titles_and_bodies_are_cut_at_their_limits() {
        let long = "x".repeat(300);
        let title = sanitize(&long, TITLE_MAX_CHARS);
        assert_eq!(title.chars().count(), 100);
        assert!(title.ends_with('\u{2026}'));
        assert_eq!(sanitize(&long, BODY_MAX_CHARS).chars().count(), 200);
        let exact = "y".repeat(100);
        assert_eq!(sanitize(&exact, TITLE_MAX_CHARS), exact);
        let n = release_notification(&[notice(&"a".repeat(80), &"t".repeat(80))]).expect("one");
        assert_eq!(n.title.chars().count(), 100);
    }

    #[test]
    fn control_characters_are_removed() {
        assert_eq!(sanitize("Tom\u{0}\u{7}\u{1b}[31m", 100), "Tom[31m");
        assert_eq!(sanitize("Line one\nline\ttwo\r", 100), "Line one line two");
    }

    #[test]
    fn markup_is_escaped() {
        assert_eq!(sanitize("Tom & Jerry <3", 100), "Tom &amp; Jerry &lt;3");
        let html = "<b>Bold</b><img src=x onerror=alert(1)>";
        let text = sanitize(html, 100);
        assert!(!text.contains('<') && !text.contains('>'));
        assert_eq!(
            text,
            "&lt;b&gt;Bold&lt;/b&gt;&lt;img src=x onerror=alert(1)&gt;"
        );
        let n = release_notification(&[notice("A&B", "<i>Hi</i>")]).expect("one");
        assert_eq!(n.title, "A&amp;B - &lt;i&gt;Hi&lt;/i&gt;");
    }
}
