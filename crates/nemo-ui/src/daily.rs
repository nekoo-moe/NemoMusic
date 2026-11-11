//! Daily recommendation sections for Home, built from catalog data the app already has. Pure and
//! deterministic: the choices are seeded with the local date, so they are stable within a day and
//! change the next day. Personal sections need a signed-in account; signed out, only sections that
//! make no claim about the listener are built.

use nemo_core::catalog::{AlbumSummary, ArtistSummary, FeedShelf, MoodChip, PodcastSummary};
use nemo_core::track::Track;

use crate::explore::MOODS;

/// Everything the account and the catalog gave for the daily sections.
#[derive(Debug, Clone, Default)]
pub struct DailyData {
    /// Liked songs, most recent first.
    pub liked: Vec<Track>,
    /// Listening history, most recent first.
    pub history: Vec<Track>,
    /// Followed artists.
    pub followed: Vec<ArtistSummary>,
    /// New releases (Explore).
    pub new_releases: Vec<AlbumSummary>,
    /// Mood chips (home feed or Explore).
    pub moods: Vec<MoodChip>,
    /// Music videos of the home feed.
    pub videos: Vec<Track>,
    /// Podcast shows of the home feed.
    pub podcasts: Vec<PodcastSummary>,
    /// The other shelves of the home feed, as the catalog gave them.
    pub feed_shelves: Vec<FeedShelf>,
}

/// The local day: a seed that changes at local midnight, and the hour for the greeting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayContext {
    /// Days since 1970-01-01 in local time.
    pub day: u64,
    /// Hour of the day, 0 to 23, local time.
    pub hour: u32,
}

impl DayContext {
    /// From unix seconds and the local offset to UTC in minutes (the app knows the time zone;
    /// `nemo-ui` has no time crate).
    #[must_use]
    pub fn from_unix(secs: u64, utc_offset_min: i32) -> Self {
        let local = i64::try_from(secs).unwrap_or(0) + i64::from(utc_offset_min) * 60;
        let local = u64::try_from(local.max(0)).unwrap_or(0);
        Self {
            day: local / 86_400,
            // The remainder is below 24, so it fits.
            #[allow(clippy::cast_possible_truncation)]
            hour: (local % 86_400 / 3_600) as u32,
        }
    }

    /// The date seed.
    #[must_use]
    pub fn seed(self) -> u64 {
        self.day.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1
    }

    /// "Good morning", "Good afternoon", "Good evening" or "Good night".
    #[must_use]
    pub fn greeting(self) -> &'static str {
        match self.hour {
            5..=11 => "Good morning",
            12..=17 => "Good afternoon",
            18..=21 => "Good evening",
            _ => "Good night",
        }
    }
}

/// Which section this is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DailyKind {
    /// Songs from likes, history and recent plays, shuffled by the date.
    DailyMix,
    /// More songs of an artist the listener played; carries the name.
    Because(String),
    /// Liked songs that were not played lately.
    Rediscover,
    /// One mood picked by the date; carries its label.
    MoodOfDay(String),
    /// New releases by followed artists.
    NewForYou,
    /// New releases of the catalog, not filtered by the listener.
    NewReleases,
}

/// One built section.
#[derive(Debug, Clone, PartialEq)]
pub struct DailySection {
    /// Kind.
    pub kind: DailyKind,
    /// Title.
    pub title: String,
    /// Honest source line: what the section is made from.
    pub tag: &'static str,
    /// Songs (mixes, rediscover, because).
    pub tracks: Vec<Track>,
    /// Albums (new releases).
    pub albums: Vec<AlbumSummary>,
}

fn next(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// Shuffle `v` with a generator seeded by `seed` (the same seed gives the same order).
pub fn shuffle_seeded<T>(v: &mut [T], seed: u64) {
    let mut s = seed | 1;
    for i in (1..v.len()).rev() {
        // The modulus is at most the length, so it fits usize.
        #[allow(clippy::cast_possible_truncation)]
        let j = (next(&mut s) % (i as u64 + 1)) as usize;
        v.swap(i, j);
    }
}

/// Pick an index below `n` from the seed.
fn pick(seed: u64, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    let mut s = seed | 1;
    // The modulus is at most `n`, so it fits usize.
    #[allow(clippy::cast_possible_truncation)]
    let i = (next(&mut s) % n as u64) as usize;
    i
}

fn dedupe(tracks: Vec<Track>) -> Vec<Track> {
    let mut seen = std::collections::HashSet::new();
    tracks
        .into_iter()
        .filter(|t| seen.insert(t.id.clone()))
        .collect()
}

/// Build the sections for the day. `recent` is what this session played; `pool` is any other
/// songs the app knows (the songs of the Home shelves).
#[must_use]
#[allow(clippy::too_many_lines)] // one pass over the section kinds, each a short block
pub fn build(
    data: &DailyData,
    recent: &[Track],
    pool: &[Track],
    signed_in: bool,
    ctx: DayContext,
) -> Vec<DailySection> {
    let seed = ctx.seed();
    let mut out = Vec::new();

    if signed_in {
        // Daily mix: likes, history, recent plays.
        let mut mix: Vec<Track> = dedupe(
            data.liked
                .iter()
                .chain(&data.history)
                .chain(recent)
                .cloned()
                .collect(),
        );
        if mix.len() >= 4 {
            shuffle_seeded(&mut mix, seed ^ 0xA1);
            mix.truncate(20);
            out.push(DailySection {
                kind: DailyKind::DailyMix,
                title: "Your daily mix".to_owned(),
                tag: "Made from your likes and history",
                tracks: mix,
                albums: Vec::new(),
            });
        }

        // Because you listened to <artist>.
        let known: Vec<&Track> = data
            .history
            .iter()
            .chain(recent)
            .chain(&data.liked)
            .chain(pool)
            .collect();
        let mut artists: Vec<String> = Vec::new();
        for t in data.history.iter().chain(recent) {
            if let Some(a) = t.artists.first()
                && !artists.contains(a)
            {
                artists.push(a.clone());
            }
        }
        // Start from a date-seeded artist and take the first one that has enough songs.
        if !artists.is_empty() {
            let start = pick(seed ^ 0xB2, artists.len());
            for k in 0..artists.len() {
                let name = &artists[(start + k) % artists.len()];
                let mut songs: Vec<Track> = dedupe(
                    known
                        .iter()
                        .filter(|t| t.artists.first() == Some(name))
                        .map(|t| (*t).clone())
                        .collect(),
                );
                if songs.len() >= 3 {
                    shuffle_seeded(&mut songs, seed ^ 0xB3);
                    songs.truncate(12);
                    out.push(DailySection {
                        kind: DailyKind::Because(name.clone()),
                        title: format!("Because you listened to {name}"),
                        tag: "Songs by this artist from your history",
                        tracks: songs,
                        albums: Vec::new(),
                    });
                    break;
                }
            }
        }

        // Rediscover: likes that are not in the history or the recent plays.
        let played: std::collections::HashSet<_> = data
            .history
            .iter()
            .chain(recent)
            .map(|t| t.id.clone())
            .collect();
        let mut old: Vec<Track> = data
            .liked
            .iter()
            .filter(|t| !played.contains(&t.id))
            .cloned()
            .collect();
        if old.len() >= 3 {
            // Older likes come last in the list: keep the older half.
            let half = old.len() / 2;
            let mut older = old.split_off(half.min(old.len().saturating_sub(3)));
            shuffle_seeded(&mut older, seed ^ 0xC4);
            older.truncate(12);
            out.push(DailySection {
                kind: DailyKind::Rediscover,
                title: "Rediscover".to_owned(),
                tag: "Liked songs you have not played lately",
                tracks: older,
                albums: Vec::new(),
            });
        }
    }

    // Mood of the day: needs no account.
    let labels: Vec<String> = if data.moods.is_empty() {
        MOODS.iter().map(|m| m.label.to_owned()).collect()
    } else {
        data.moods.iter().map(|m| m.title.clone()).collect()
    };
    let label = labels[pick(seed ^ 0xD5, labels.len())].clone();
    out.push(DailySection {
        kind: DailyKind::MoodOfDay(label.clone()),
        title: "Mood of the day".to_owned(),
        tag: "Picked by the date, the same for everyone",
        tracks: Vec::new(),
        albums: Vec::new(),
    });

    // New releases: filtered by followed artists when signed in and something matches.
    if signed_in {
        let followed: Vec<String> = data
            .followed
            .iter()
            .map(|a| a.name.to_lowercase())
            .collect();
        let mine: Vec<AlbumSummary> = data
            .new_releases
            .iter()
            .filter(|al| {
                al.artists
                    .iter()
                    .any(|a| followed.contains(&a.to_lowercase()))
            })
            .cloned()
            .collect();
        if !mine.is_empty() {
            out.push(DailySection {
                kind: DailyKind::NewForYou,
                title: "New for you".to_owned(),
                tag: "New releases by artists you follow",
                tracks: Vec::new(),
                albums: mine,
            });
            return out;
        }
    }
    if !data.new_releases.is_empty() {
        out.push(DailySection {
            kind: DailyKind::NewReleases,
            title: "New releases".to_owned(),
            tag: "Not personalised",
            tracks: Vec::new(),
            albums: data.new_releases.iter().take(12).cloned().collect(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock;

    fn data() -> DailyData {
        DailyData {
            liked: (0..30).map(mock::track).collect(),
            history: (2..10).map(mock::track).collect(),
            followed: vec![ArtistSummary {
                id: nemo_core::catalog::BrowseId("a".into()),
                name: mock::track(0).artists[0].clone(),
                thumbnails: Vec::new(),
            }],
            new_releases: vec![AlbumSummary {
                id: nemo_core::catalog::BrowseId("al".into()),
                title: "Fresh".into(),
                artists: vec![mock::track(0).artists[0].clone()],
                year: None,
                thumbnails: Vec::new(),
            }],
            ..DailyData::default()
        }
    }

    fn titles(s: &[DailySection]) -> Vec<String> {
        s.iter().map(|s| s.title.clone()).collect()
    }

    #[test]
    fn the_same_day_gives_the_same_sections_and_the_next_day_differs() {
        let d = data();
        let day = DayContext::from_unix(1_800_000_000, 0);
        let a = build(&d, &[], &[], true, day);
        let b = build(&d, &[], &[], true, DayContext { hour: 23, ..day });
        assert_eq!(a, b, "the hour must not change the choices");
        let next_day = DayContext {
            day: day.day + 1,
            ..day
        };
        let c = build(&d, &[], &[], true, next_day);
        assert_ne!(a, c, "the next day picks differently");
    }

    #[test]
    fn signed_out_has_no_personal_sections() {
        let d = data();
        let s = build(&d, &[], &[], false, DayContext::from_unix(1_800_000_000, 0));
        for sec in &s {
            assert!(
                !sec.title.contains("for you") && !sec.title.contains("Your daily"),
                "{}",
                sec.title
            );
            assert!(!matches!(
                sec.kind,
                DailyKind::DailyMix | DailyKind::Rediscover | DailyKind::NewForYou
            ));
        }
        assert!(s.iter().any(|s| matches!(s.kind, DailyKind::MoodOfDay(_))));
    }

    #[test]
    fn signed_in_builds_the_personal_sections() {
        let d = data();
        let s = build(&d, &[], &[], true, DayContext::from_unix(1_800_000_000, 0));
        let t = titles(&s);
        assert!(t.contains(&"Your daily mix".to_owned()), "{t:?}");
        assert!(t.contains(&"Rediscover".to_owned()), "{t:?}");
        assert!(t.contains(&"New for you".to_owned()), "{t:?}");
        assert!(
            t.iter().any(|x| x.starts_with("Because you listened to")),
            "{t:?}"
        );
    }

    #[test]
    fn day_context_follows_the_offset() {
        // 2027-01-15 23:30 UTC is already the next day in UTC+7.
        let secs = 1_800_000_000_u64;
        let utc = DayContext::from_unix(secs, 0);
        let plus = DayContext::from_unix(secs + 3_600 * 3, 7 * 60);
        assert!(plus.day >= utc.day);
        assert_eq!(DayContext { day: 0, hour: 8 }.greeting(), "Good morning");
        assert_eq!(DayContext { day: 0, hour: 14 }.greeting(), "Good afternoon");
        assert_eq!(DayContext { day: 0, hour: 19 }.greeting(), "Good evening");
        assert_eq!(DayContext { day: 0, hour: 2 }.greeting(), "Good night");
    }

    #[test]
    fn rediscover_excludes_what_was_played() {
        let d = data();
        let s = build(&d, &[], &[], true, DayContext::from_unix(1_800_000_000, 0));
        let r = s
            .iter()
            .find(|s| s.kind == DailyKind::Rediscover)
            .expect("rediscover");
        for t in &r.tracks {
            assert!(!d.history.iter().any(|h| h.id == t.id));
        }
    }
}
