//! Fake lyrics for the mock backend: most tracks have synced lyrics from three sources (one
//! syllable-timed, one word-timed, one line-timed), every 7th is unsynced and every 11th has
//! none. All text is invented and nothing touches the network.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use nemo_core::BoxFuture;
use nemo_core::lyrics::{
    Granularity, LyricLine, LyricWord, LyricsCandidate, LyricsCredit, LyricsDoc, LyricsQuery,
    ProviderId,
};
use nemo_core::services::{LyricsLookup, LyricsProviderStatus, LyricsService};

use crate::mock::{LYRIC_PHRASES, LYRICS_PROVIDER, TRACKS, track};

/// Mock lyrics sources in chain order: name, risk note, host, finest timing.
static MOCK_SOURCES: [(&str, Option<&str>, &str, Granularity); 4] = [
    ("Mock Lyrics", None, "mock.invalid", Granularity::Line),
    ("Unison", None, "unison.invalid", Granularity::Word),
    (
        "Better Lyrics",
        None,
        "betterlyrics.invalid",
        Granularity::Syllable,
    ),
    (
        "Mock Sidecar",
        Some("Unofficial source; its terms may not allow this use."),
        "sidecar.invalid",
        Granularity::Line,
    ),
];

/// Time a synthetic line is sung for, ms (lines start every 4 s).
const SUNG_MS: u64 = 3_400;

/// The fake lyrics service.
#[derive(Default)]
pub struct MockLyrics {
    offsets: Mutex<HashMap<String, i64>>,
    chosen: Mutex<HashMap<String, ProviderId>>,
    overrides: Mutex<HashMap<&'static str, bool>>,
}

/// Index of a mock track from the lyrics query (matched by title and artist).
fn query_index(query: &LyricsQuery) -> Option<usize> {
    (0..TRACKS).find(|n| {
        let t = track(*n);
        t.title == query.title && t.artists == query.artists
    })
}

/// Synthetic line-timed lyrics for mock track `n`: one line every 4 s from 0:08.
#[must_use]
pub fn lyrics_for(n: usize, duration: Duration) -> Option<LyricsDoc> {
    if n % 11 == 5 {
        return None;
    }
    let unsynced = n % 7 == 3;
    let count = usize::try_from(duration.as_secs().saturating_sub(12) / 4).unwrap_or(0);
    let lines = (0..count)
        .map(|i| LyricLine {
            start_ms: (!unsynced).then(|| 8_000 + 4_000 * i as u64),
            end_ms: (!unsynced).then(|| 8_000 + 4_000 * i as u64 + SUNG_MS),
            text: LYRIC_PHRASES[(i + n) % LYRIC_PHRASES.len()].to_owned(),
            words: Vec::new(),
        })
        .collect();
    Some(LyricsDoc {
        lines,
        granularity: if unsynced {
            Granularity::Unsynced
        } else {
            Granularity::Line
        },
        provider: ProviderId::new(LYRICS_PROVIDER),
        matched: None,
        instrumental: false,
    })
}

/// Pieces of one word: words longer than five letters split in two syllables. The last piece
/// keeps the space after the word.
fn word_pieces(word: &str, syllables: bool, last: bool) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let mut out: Vec<String> = if syllables && chars.len() > 5 {
        let mid = chars.len() / 2;
        vec![chars[..mid].iter().collect(), chars[mid..].iter().collect()]
    } else {
        vec![word.to_owned()]
    };
    if !last && let Some(p) = out.last_mut() {
        p.push(' ');
    }
    out
}

/// `doc` with every line split into timed words or syllables, spread over the sung time by
/// their length.
fn timed(mut doc: LyricsDoc, level: Granularity) -> LyricsDoc {
    let syllables = level == Granularity::Syllable;
    for line in &mut doc.lines {
        let Some(start) = line.start_ms else { continue };
        let words: Vec<&str> = line.text.split(' ').collect();
        let pieces: Vec<String> = words
            .iter()
            .enumerate()
            .flat_map(|(i, w)| word_pieces(w, syllables, i + 1 == words.len()))
            .collect();
        let total: u64 = pieces
            .iter()
            .map(|p| p.chars().count() as u64)
            .sum::<u64>()
            .max(1);
        let mut at = start;
        line.words = pieces
            .into_iter()
            .map(|text| {
                let len = SUNG_MS * text.chars().count() as u64 / total;
                let w = LyricWord {
                    start_ms: at,
                    end_ms: Some(at + len),
                    text,
                };
                at += len;
                w
            })
            .collect();
    }
    doc.granularity = level;
    doc
}

/// The credit a mock source asks for.
fn credit(name: &str) -> Option<LyricsCredit> {
    match name {
        "Unison" => Some(LyricsCredit {
            label: "Lyrics from Unison".to_owned(),
            url: None,
            by: Some("mira_k".to_owned()),
            by_url: None,
            note: None,
        }),
        "Better Lyrics" => Some(LyricsCredit {
            label: "Lyrics via Better Lyrics".to_owned(),
            url: Some("https://better-lyrics.boidu.dev/".to_owned()),
            by: None,
            by_url: None,
            note: None,
        }),
        _ => None,
    }
}

impl MockLyrics {
    fn enabled(&self, name: &str, risk: Option<&str>) -> bool {
        let on = self.overrides.lock().map(|d| d.clone()).unwrap_or_default();
        on.get(name).copied().unwrap_or(risk.is_none())
    }

    /// Every enabled source's answer, best first (level, then score, then chain order).
    fn candidates(&self, query: &LyricsQuery) -> Vec<LyricsCandidate> {
        let duration = query
            .duration_ms
            .map_or(Duration::from_secs(180), Duration::from_millis);
        let Some(base) = query_index(query).and_then(|n| lyrics_for(n, duration)) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (rank, (name, risk, _, best)) in MOCK_SOURCES.iter().enumerate() {
            // The sidecar never has lyrics; only the line source has unsynced text.
            if rank == 3 || !self.enabled(name, *risk) {
                continue;
            }
            if base.granularity == Granularity::Unsynced && *best != Granularity::Line {
                continue;
            }
            let mut d = if base.granularity == Granularity::Unsynced {
                base.clone()
            } else {
                timed(base.clone(), *best)
            };
            if *best == Granularity::Line {
                d.granularity = base.granularity;
            }
            d.provider = ProviderId::new(name);
            out.push(LyricsCandidate {
                provider: d.provider,
                granularity: d.granularity,
                score: if *name == LYRICS_PROVIDER { 100 } else { 90 },
                source_note: risk.map(str::to_owned),
                credit: credit(name),
                doc: d,
            });
        }
        out.sort_by(|a, b| {
            b.granularity
                .cmp(&a.granularity)
                .then(b.score.cmp(&a.score))
        });
        out
    }
}

impl LyricsService for MockLyrics {
    fn lookup<'a>(&'a self, query: &'a LyricsQuery) -> BoxFuture<'a, LyricsLookup> {
        let all = self.candidates(query);
        let key = query_index(query).map(|n| track(n).id.0);
        let pick = key.and_then(|k| self.chosen(&k));
        let doc = pick
            .and_then(|p| all.iter().find(|c| c.provider == p))
            .or_else(|| all.first())
            .map(|c| c.doc.clone());
        Box::pin(async move {
            LyricsLookup {
                doc,
                tried: vec![
                    ProviderId::new(LYRICS_PROVIDER),
                    ProviderId::new("Mock Sidecar"),
                ],
            }
        })
    }

    fn offset_ms(&self, track_key: &str) -> i64 {
        self.offsets
            .lock()
            .ok()
            .and_then(|m| m.get(track_key).copied())
            .unwrap_or(0)
    }

    fn set_offset_ms(&self, track_key: &str, offset_ms: i64) {
        if let Ok(mut m) = self.offsets.lock() {
            m.insert(track_key.to_owned(), offset_ms);
        }
    }

    fn lookup_all<'a>(&'a self, query: &'a LyricsQuery) -> BoxFuture<'a, Vec<LyricsCandidate>> {
        let out = self.candidates(query);
        Box::pin(async move { out })
    }

    fn choose(&self, track_key: &str, provider: ProviderId) {
        if let Ok(mut m) = self.chosen.lock() {
            m.insert(track_key.to_owned(), provider);
        }
    }

    fn chosen(&self, track_key: &str) -> Option<ProviderId> {
        self.chosen.lock().ok()?.get(track_key).copied()
    }

    fn clear_choice(&self, track_key: &str) {
        if let Ok(mut m) = self.chosen.lock() {
            m.remove(track_key);
        }
    }

    fn credit(&self, _track_key: &str, provider: ProviderId) -> Option<LyricsCredit> {
        credit(provider.as_str())
    }

    fn providers(&self) -> Vec<LyricsProviderStatus> {
        MOCK_SOURCES
            .iter()
            .map(|(name, risk, host, best)| LyricsProviderStatus {
                id: ProviderId::new(name),
                enabled: self.enabled(name, *risk),
                default_enabled: risk.is_none(),
                risk_note: *risk,
                hosts: std::slice::from_ref(host),
                best: *best,
            })
            .collect()
    }

    fn set_provider_enabled(&self, provider: ProviderId, enabled: bool) -> bool {
        let Some((name, ..)) = MOCK_SOURCES.iter().find(|s| s.0 == provider.as_str()) else {
            return false;
        };
        if let Ok(mut d) = self.overrides.lock() {
            d.insert(name, enabled);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    fn ready<T>(fut: impl Future<Output = T>) -> T {
        let mut fut = pin!(fut);
        let mut cx = Context::from_waker(Waker::noop());
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => v,
            Poll::Pending => unreachable!("mock futures complete at once"),
        }
    }

    #[test]
    fn sources_come_best_first_with_their_credits() {
        let l = MockLyrics::default();
        let q = LyricsQuery::from_track(&track(0));
        let all = ready(l.lookup_all(&q));
        let names: Vec<_> = all.iter().map(|c| c.provider.as_str()).collect();
        assert_eq!(names, ["Better Lyrics", "Unison", "Mock Lyrics"]);
        assert_eq!(all[0].granularity, Granularity::Syllable);
        assert_eq!(all[1].granularity, Granularity::Word);
        assert_eq!(
            all[1].credit.as_ref().map(|c| c.label.as_str()),
            Some("Lyrics from Unison")
        );
        assert!(l.credit("x", ProviderId::new("Better Lyrics")).is_some());
        assert!(l.credit("x", ProviderId::new("Mock Lyrics")).is_none());
        // The best source wins until the user picks another; clearing the pick restores it.
        assert_eq!(
            ready(l.lookup(&q)).doc.map(|d| d.granularity),
            Some(Granularity::Syllable)
        );
        let key = track(0).id.0;
        l.choose(&key, ProviderId::new("Mock Lyrics"));
        assert_eq!(
            ready(l.lookup(&q)).doc.map(|d| d.granularity),
            Some(Granularity::Line)
        );
        l.clear_choice(&key);
        assert_eq!(l.chosen(&key), None);
    }

    #[test]
    fn opt_in_sources_start_off_and_unknown_ids_are_refused() {
        let l = MockLyrics::default();
        let sidecar = ProviderId::new("Mock Sidecar");
        let on = |l: &MockLyrics| l.providers().iter().any(|p| p.id == sidecar && p.enabled);
        assert!(!on(&l));
        assert!(l.set_provider_enabled(sidecar, true));
        assert!(on(&l));
        assert!(!l.set_provider_enabled(ProviderId::new("Nope"), true));
        assert!(l.set_provider_enabled(ProviderId::new("Better Lyrics"), false));
        let q = LyricsQuery::from_track(&track(0));
        assert_eq!(ready(l.lookup_all(&q)).len(), 2);
    }

    #[test]
    fn syllable_pieces_join_back_into_the_line_and_are_timed_in_order() {
        let l = MockLyrics::default();
        let doc = ready(l.lookup(&LyricsQuery::from_track(&track(1))))
            .doc
            .expect("synced");
        // Track 1, line 8: "nothing here is copyrighted" has long words.
        let line = &doc.lines[8];
        let joined: String = line.words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(joined, line.text);
        assert!(
            line.words.len() > line.text.split(' ').count(),
            "words were split"
        );
        assert!(
            line.words
                .windows(2)
                .all(|w| w[0].start_ms <= w[1].start_ms)
        );
    }

    #[test]
    fn lyrics_are_synced_unsynced_or_missing() {
        let lyrics = MockLyrics::default();
        let q = |n| LyricsQuery::from_track(&track(n));
        let synced = ready(lyrics.lookup(&q(0))).doc.expect("synced");
        assert!(synced.lines.len() > 20 && synced.lines[0].start_ms == Some(8_000));
        let unsynced = ready(lyrics.lookup(&q(3))).doc.expect("unsynced");
        assert_eq!(unsynced.granularity, Granularity::Unsynced);
        assert!(unsynced.lines.iter().all(|l| l.start_ms.is_none()));
        let none = ready(lyrics.lookup(&q(5)));
        assert!(none.doc.is_none() && !none.tried.is_empty());
        assert_eq!(lyrics.offset_ms("k"), 0);
        lyrics.set_offset_ms("k", -300);
        assert_eq!(lyrics.offset_ms("k"), -300);
    }
}
