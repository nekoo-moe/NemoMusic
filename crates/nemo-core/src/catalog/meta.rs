//! Header facts of an album or playlist page, as the source shows them (collection-sort-meta
//! spec). Every field is optional: a reader fills only what the page gives, and the UI hides what
//! is missing.

use crate::track::Thumbnail;

use super::Privacy;

/// What the header of an album or playlist page says beside its title.
///
/// Build it with `..CollectionMeta::default()` so new fields need no change at the call site.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CollectionMeta {
    /// The owner (playlist author or album artist), as shown in the header.
    pub owner: Option<String>,
    /// Picture sizes of the owner, when the header shows one.
    pub owner_avatar: Vec<Thumbnail>,
    /// Who can see the playlist, when the page says it.
    pub privacy: Option<Privacy>,
    /// Year shown in the header.
    pub year: Option<u16>,
    /// Number of views, when shown. A shortened source number (`437K`) is kept at the precision
    /// it was shown with (`437_000`).
    pub view_count: Option<u64>,
    /// Number of tracks the page says it holds (it can be more than the tracks read).
    pub track_count: Option<u32>,
    /// Total length as the source writes it (`36 minutes`, `13+ hours`); kept as text so a
    /// rounded or open-ended length is never shown more exactly than the source knows it.
    pub duration_text: Option<String>,
}

impl CollectionMeta {
    /// `true` when no field is known.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_empty_and_any_field_makes_it_not() {
        assert!(CollectionMeta::default().is_empty());
        let m = CollectionMeta {
            track_count: Some(3),
            ..CollectionMeta::default()
        };
        assert!(!m.is_empty());
    }
}
