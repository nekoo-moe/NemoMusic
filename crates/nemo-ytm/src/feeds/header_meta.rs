//! The facts in the header of an album or playlist page beside the title (collection-sort-meta
//! spec): the owner and their picture (`straplineTextOne` with `straplineThumbnail` on albums,
//! the `facepile` on playlists), privacy and year from `subtitle`, and views, track count and
//! length from `secondSubtitle`. Layout checked against anonymous answers of 2026-11-15
//! (`tests/fixtures/music/browse-playlist.json`, `browse-album.json`) and `ytmusicapi`
//! (`parsers/playlists.py`, `parse_playlist_header_meta`). What a header does not show stays
//! `None`; nothing is guessed.

use nemo_core::catalog::{CollectionMeta, Privacy};
use nemo_core::track::Thumbnail;
use serde_json::Value;

use super::json::{find_key, text_of, thumbnails_of};
use super::runs::Runs;
use crate::catalog::{map_thumbnails, parse_year};

/// The owner's name and picture from a playlist `facepile`.
fn facepile(header: &Value) -> (Option<String>, Vec<Thumbnail>) {
    let Some(stack) = header.pointer("/facepile/avatarStackViewModel") else {
        return (None, Vec::new());
    };
    let name = stack
        .pointer("/text/content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned);
    let sources: Vec<(String, u64, u64)> = stack
        .pointer("/avatars/0/avatarViewModel/image/sources")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|s| {
                    Some((
                        s.get("url")?.as_str()?.to_owned(),
                        s.get("width").and_then(Value::as_u64).unwrap_or(0),
                        s.get("height").and_then(Value::as_u64).unwrap_or(0),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    (name, map_thumbnails(&sources))
}

/// The owner shown in a header: the strapline (albums), else the facepile (playlists).
pub(super) fn owner(header: &Value) -> (Option<String>, Vec<Thumbnail>) {
    if let Some(name) = header.get("straplineTextOne").and_then(text_of) {
        let avatar = header
            .get("straplineThumbnail")
            .map(thumbnails_of)
            .unwrap_or_default();
        return (Some(name), avatar);
    }
    facepile(header)
}

/// A privacy word as the subtitle or the edit header writes it.
fn privacy_of(text: &str) -> Option<Privacy> {
    match text.trim().to_ascii_lowercase().as_str() {
        "public" => Some(Privacy::Public),
        "unlisted" => Some(Privacy::Unlisted),
        "private" => Some(Privacy::Private),
        _ => None,
    }
}

/// A count as YouTube writes it: `249`, `1,234`, `437K`, `1.2M`, `3B`. The number keeps the
/// precision it was written with.
pub(crate) fn count_of(text: &str) -> Option<u64> {
    let word = text.split_whitespace().next()?;
    let (digits, factor) = match word.chars().last()? {
        'K' | 'k' => (&word[..word.len() - 1], 1_000_u64),
        'M' | 'm' => (&word[..word.len() - 1], 1_000_000),
        'B' | 'b' => (&word[..word.len() - 1], 1_000_000_000),
        _ => (word, 1),
    };
    let digits = digits.replace(',', "");
    let (whole, frac) = digits.split_once('.').unwrap_or((&digits, ""));
    if whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if !frac.bytes().all(|b| b.is_ascii_digit()) || frac.len() > 3 {
        return None;
    }
    let whole: u64 = whole.parse().ok()?;
    let mut value = whole.checked_mul(factor)?;
    if !frac.is_empty() {
        let scale = 10_u64.pow(u32::try_from(frac.len()).ok()?);
        let part: u64 = frac.parse().ok()?;
        value = value.checked_add(part.checked_mul(factor)? / scale)?;
    }
    Some(value)
}

/// Read one plain run of `secondSubtitle` into `meta`.
fn second_subtitle_run(text: &str, meta: &mut CollectionMeta) {
    let lower = text.to_ascii_lowercase();
    let last = lower.split_whitespace().last().unwrap_or("");
    match last {
        "view" | "views" => meta.view_count = count_of(text),
        "song" | "songs" | "track" | "tracks" | "episode" | "episodes" | "video" | "videos" => {
            meta.track_count = count_of(text).and_then(|n| u32::try_from(n).ok());
        }
        _ if ["second", "minute", "hour"]
            .iter()
            .any(|u| lower.contains(u)) =>
        {
            meta.duration_text = Some(text.trim().to_owned());
        }
        _ => {}
    }
}

/// The header facts of a page. `root` is the whole answer (the edit header of one's own playlist
/// holds the privacy), `header` the responsive or detail header inside it.
pub(super) fn read(root: &Value, header: &Value) -> CollectionMeta {
    let (owner, owner_avatar) = owner(header);
    let subtitle = Runs::from_value(header.get("subtitle"));
    let mut meta = CollectionMeta {
        owner,
        owner_avatar,
        privacy: subtitle.plain.iter().find_map(|t| privacy_of(t)),
        year: subtitle.year_text().and_then(|t| parse_year(&t)),
        ..CollectionMeta::default()
    };
    if meta.privacy.is_none() {
        meta.privacy = find_key(root, "musicPlaylistEditHeaderRenderer")
            .and_then(|h| h.get("privacy"))
            .and_then(Value::as_str)
            .and_then(privacy_of);
    }
    for text in &Runs::from_value(header.get("secondSubtitle")).plain {
        second_subtitle_run(text, &mut meta);
    }
    meta
}

#[cfg(test)]
mod tests {
    use super::count_of;

    #[test]
    fn counts_keep_the_precision_they_were_written_with() {
        assert_eq!(count_of("411 views"), Some(411));
        assert_eq!(count_of("1,234 tracks"), Some(1234));
        assert_eq!(count_of("437K views"), Some(437_000));
        assert_eq!(count_of("1.2M views"), Some(1_200_000));
        assert_eq!(count_of("3B views"), Some(3_000_000_000));
        assert_eq!(count_of("No views"), None);
        assert_eq!(count_of(""), None);
        assert_eq!(count_of("1.2.3K"), None);
    }
}
