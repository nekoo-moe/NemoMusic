//! Resolving and listing media by page URL (`yt-dlp` behind a trait), and the pure ranking that
//! picks one audio format from what a page offers.

use std::time::Duration;

use crate::BoxFuture;
use crate::audio::Codec;
use crate::stream::{ResolvedStream, StreamError};
use crate::track::{MediaUrl, Thumbnail};

/// A site a text search can run on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Site {
    /// SoundCloud (public pages only).
    SoundCloud,
    /// YouTube.
    Youtube,
}

/// What [`UrlResolver::list`] is asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaQuery {
    /// The entries behind one page URL (a playlist, a user page, a single track).
    Url(MediaUrl),
    /// A text search on one site.
    Search {
        /// The site to search.
        site: Site,
        /// The search text as the listener typed it.
        text: String,
        /// The most entries to return.
        limit: u8,
    },
}

/// What kind of thing a [`MediaEntry`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryKind {
    /// One playable track.
    Track,
    /// A playlist or album page.
    Playlist,
    /// A user or channel page.
    User,
}

/// One result of [`UrlResolver::list`]. Metadata only: no stream address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaEntry {
    /// What the entry is.
    pub kind: EntryKind,
    /// The page of the entry.
    pub url: MediaUrl,
    /// The site's own id for the entry.
    pub id: String,
    /// Title as the site shows it.
    pub title: String,
    /// Uploader name, if the site gives one.
    pub uploader: Option<String>,
    /// Page of the uploader, if the site gives one.
    pub uploader_url: Option<MediaUrl>,
    /// Length, if known. `None` for live streams and unknown lengths.
    pub duration: Option<Duration>,
    /// Cover images, any order.
    pub thumbnails: Vec<Thumbnail>,
    /// The entry is a live stream.
    pub live: bool,
}

/// What the player can use when a stream is chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatPolicy {
    /// Codecs the decoder can play now. A format in any other codec is never chosen.
    pub playable: Vec<Codec>,
    /// Whether the stream opener can read HLS playlists.
    pub allow_hls: bool,
}

/// One audio format a page offers, reduced to what the ranking needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatCandidate {
    /// Position of the format in the resolver's own list. [`rank_formats`] returns it.
    pub index: usize,
    /// Audio codec.
    pub codec: Codec,
    /// Average bitrate in kbit/s. `None` when the site does not say.
    pub bitrate_kbps: Option<u32>,
    /// The format is an HLS playlist, not one file.
    pub hls: bool,
    /// The format is encrypted. It is never chosen.
    pub encrypted: bool,
    /// The format is a short preview of the track, not the whole track.
    pub preview: bool,
}

/// Whether the resolver program is usable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolverState {
    /// The program runs.
    Ready {
        /// Version text the program reports.
        version: String,
    },
    /// The program is not installed.
    Missing,
    /// The program is installed but too old.
    TooOld,
}

/// Turns a page URL into a playable stream, and lists pages and searches. Implemented by
/// `nemo-ytm` with `yt-dlp`. Calls are human-paced: callers never resolve a whole queue at once.
pub trait UrlResolver: Send + Sync {
    /// Resolve one page to the best stream `policy` allows.
    ///
    /// # Errors
    /// [`StreamError::NotAvailable`] when no format is playable (encrypted only, codec not in
    /// `policy`), [`StreamError::ResolverMissing`], [`StreamError::Network`] or
    /// [`StreamError::Parse`].
    fn resolve<'a>(
        &'a self,
        url: &'a MediaUrl,
        policy: &'a FormatPolicy,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>>;

    /// List the entries behind a page, or the results of a search. Metadata only.
    ///
    /// # Errors
    /// The same errors as [`UrlResolver::resolve`], except `NotAvailable` for an empty answer:
    /// an empty list is `Ok`.
    fn list<'a>(
        &'a self,
        query: &'a MediaQuery,
    ) -> BoxFuture<'a, Result<Vec<MediaEntry>, StreamError>>;

    /// Whether the resolver program is usable. Must return at once.
    fn state(&self) -> ResolverState;

    /// Hint: `url` will probably be played soon (the next queue entry). A resolver may look the
    /// stream up in the background. Must return at once. Default: does nothing.
    fn warm(&self, _url: &MediaUrl) {}

    /// The stream last returned for `url` could not be opened (for example HTTP 403 on an
    /// address that has expired). A resolver may drop a cached answer. Must return at once.
    /// Default: does nothing.
    fn stream_failed(&self, _url: &MediaUrl) {}
}

/// Score factor in hundredths: how much better a codec sounds than MP3 at the same bitrate.
/// First values from the `yt-dlp` issue that finds 64 kbit/s Opus better than 128 kbit/s MP3;
/// a listening test settles them.
const fn efficiency(codec: Codec) -> u64 {
    match codec {
        Codec::Opus => 200,
        Codec::Aac => 125,
        Codec::Vorbis => 115,
        _ => 100,
    }
}

/// Tie order: the newer codec wins (Opus, AAC, Vorbis, then the rest).
const fn recency(codec: Codec) -> u8 {
    match codec {
        Codec::Opus => 3,
        Codec::Aac => 2,
        Codec::Vorbis => 1,
        _ => 0,
    }
}

/// Choose one audio format. Returns the [`FormatCandidate::index`] of the winner, or `None` when
/// nothing can be played.
///
/// Dropped first: encrypted formats, codecs not in `policy.playable`, and HLS when
/// `policy.allow_hls` is false. A plain file beats HLS of the same codec. Of what is left, a full
/// stream beats a preview; then the higher score wins (bitrate times codec efficiency, a missing
/// bitrate scores 0 but stays playable); a tie goes to the newer codec, then to the earlier
/// candidate.
#[must_use]
pub fn rank_formats(candidates: &[FormatCandidate], policy: &FormatPolicy) -> Option<usize> {
    let usable = |c: &&FormatCandidate| {
        !c.encrypted && policy.playable.contains(&c.codec) && (policy.allow_hls || !c.hls)
    };
    let has_file = |codec: Codec| {
        candidates
            .iter()
            .filter(usable)
            .any(|c| c.codec == codec && !c.hls)
    };
    candidates
        .iter()
        .filter(usable)
        .filter(|c| !c.hls || !has_file(c.codec))
        .max_by_key(|c| {
            let score = u64::from(c.bitrate_kbps.unwrap_or(0)) * efficiency(c.codec);
            (
                !c.preview,
                score,
                recency(c.codec),
                std::cmp::Reverse(c.index),
            )
        })
        .map(|c| c.index)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn cand(index: usize, codec: Codec, kbps: Option<u32>) -> FormatCandidate {
        FormatCandidate {
            index,
            codec,
            bitrate_kbps: kbps,
            hls: false,
            encrypted: false,
            preview: false,
        }
    }

    fn policy(playable: &[Codec]) -> FormatPolicy {
        FormatPolicy {
            playable: playable.to_vec(),
            allow_hls: true,
        }
    }

    const ALL: [Codec; 4] = [Codec::Opus, Codec::Aac, Codec::Mp3, Codec::Vorbis];

    #[test]
    fn low_bitrate_opus_beats_higher_bitrate_mp3() {
        let list = [
            cand(0, Codec::Mp3, Some(128)),
            cand(1, Codec::Opus, Some(64)),
        ];
        // 64 x 2.0 = 128 ties with 128 x 1.0; the newer codec wins the tie.
        assert_eq!(rank_formats(&list, &policy(&ALL)), Some(1));
        let list = [
            cand(0, Codec::Mp3, Some(96)),
            cand(1, Codec::Opus, Some(64)),
        ];
        assert_eq!(rank_formats(&list, &policy(&ALL)), Some(1));
    }

    #[test]
    fn higher_score_wins_across_codecs() {
        let list = [
            cand(0, Codec::Aac, Some(256)),
            cand(1, Codec::Mp3, Some(320)),
        ];
        assert_eq!(rank_formats(&list, &policy(&ALL)), Some(0));
    }

    #[test]
    fn opus_is_dropped_when_not_playable() {
        let list = [
            cand(0, Codec::Opus, Some(160)),
            cand(1, Codec::Mp3, Some(128)),
        ];
        assert_eq!(
            rank_formats(&list, &policy(&[Codec::Aac, Codec::Mp3])),
            Some(1)
        );
    }

    #[test]
    fn encrypted_formats_are_dropped() {
        let mut best = cand(0, Codec::Aac, Some(256));
        best.encrypted = true;
        let list = [best, cand(1, Codec::Mp3, Some(64))];
        assert_eq!(rank_formats(&list, &policy(&ALL)), Some(1));
        assert_eq!(rank_formats(&list[..1], &policy(&ALL)), None);
    }

    #[test]
    fn a_preview_ranks_below_every_full_stream() {
        let mut preview = cand(0, Codec::Aac, Some(256));
        preview.preview = true;
        let list = [preview, cand(1, Codec::Mp3, Some(32))];
        assert_eq!(rank_formats(&list, &policy(&ALL)), Some(1));
        // A preview alone is still chosen.
        assert_eq!(rank_formats(&list[..1], &policy(&ALL)), Some(0));
    }

    #[test]
    fn a_missing_bitrate_is_playable_but_scores_zero() {
        let list = [cand(0, Codec::Aac, None), cand(1, Codec::Mp3, Some(1))];
        assert_eq!(rank_formats(&list, &policy(&ALL)), Some(1));
        assert_eq!(rank_formats(&list[..1], &policy(&ALL)), Some(0));
    }

    #[test]
    fn ties_go_to_the_newer_codec() {
        // 100 x 2.0 = 200 x 1.0.
        let list = [
            cand(0, Codec::Mp3, Some(200)),
            cand(1, Codec::Opus, Some(100)),
        ];
        assert_eq!(rank_formats(&list, &policy(&ALL)), Some(1));
        let list = [cand(0, Codec::Mp3, None), cand(1, Codec::Aac, None)];
        assert_eq!(rank_formats(&list, &policy(&ALL)), Some(1));
    }

    #[test]
    fn a_full_tie_goes_to_the_earlier_candidate() {
        let list = [
            cand(4, Codec::Mp3, Some(128)),
            cand(7, Codec::Mp3, Some(128)),
        ];
        assert_eq!(rank_formats(&list, &policy(&ALL)), Some(4));
    }

    #[test]
    fn a_file_beats_hls_of_the_same_codec() {
        let mut hls = cand(0, Codec::Aac, Some(256));
        hls.hls = true;
        let list = [hls, cand(1, Codec::Aac, Some(128))];
        assert_eq!(rank_formats(&list, &policy(&ALL)), Some(1));
        // HLS of another codec still competes on score.
        let mut hls = cand(0, Codec::Aac, Some(256));
        hls.hls = true;
        let list = [hls, cand(1, Codec::Mp3, Some(128))];
        assert_eq!(rank_formats(&list, &policy(&ALL)), Some(0));
    }

    #[test]
    fn hls_is_dropped_when_not_allowed() {
        let mut hls = cand(0, Codec::Aac, Some(256));
        hls.hls = true;
        let list = [hls, cand(1, Codec::Mp3, Some(64))];
        let no_hls = FormatPolicy {
            playable: ALL.to_vec(),
            allow_hls: false,
        };
        assert_eq!(rank_formats(&list, &no_hls), Some(1));
        assert_eq!(rank_formats(&list[..1], &no_hls), None);
    }

    #[test]
    fn nothing_playable_gives_none() {
        assert_eq!(rank_formats(&[], &policy(&ALL)), None);
        let list = [cand(0, Codec::Opus, Some(128))];
        assert_eq!(rank_formats(&list, &policy(&[])), None);
    }

    fn any_codec() -> impl Strategy<Value = Codec> {
        prop_oneof![
            Just(Codec::Flac),
            Just(Codec::Aac),
            Just(Codec::Opus),
            Just(Codec::Mp3),
            Just(Codec::Vorbis),
            Just(Codec::Unknown),
        ]
    }

    proptest! {
            #[test]
            fn the_result_index_is_always_in_range(
                specs in proptest::collection::vec(
                    (any_codec(), proptest::option::of(0u32..2000), any::<bool>(), any::<bool>(), any::<bool>()),
                    0..12,
                ),
                playable in proptest::collection::vec(any_codec(), 0..6),
                allow_hls in any::<bool>(),
            ) {
                let list: Vec<FormatCandidate> = specs
                    .into_iter()
                    .enumerate()
                    .map(|(index, (codec, bitrate_kbps, hls, encrypted, preview))| FormatCandidate {
                        index, codec, bitrate_kbps, hls, encrypted, preview,
                    })
                    .collect();
                let policy = FormatPolicy { playable, allow_hls };
                if let Some(i) = rank_formats(&list, &policy) {
                        prop_assert!(i < list.len());
                        let chosen = &list[i];
                        prop_assert!(!chosen.encrypted);
                        prop_assert!(policy.playable.contains(&chosen.codec));
                        prop_assert!(policy.allow_hls || !chosen.hls);
    } else {
                        let none_usable = list.iter().all(|c| {
                            c.encrypted
                                || !policy.playable.contains(&c.codec)
                                || (!policy.allow_hls && c.hls)
                        });
                        prop_assert!(none_usable);
                }
            }
        }
}
