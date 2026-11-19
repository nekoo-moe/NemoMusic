//! Search box state: debounce, stale-answer handling, "Enter plays the top song" and the filter
//! chips (search-like-ytm). Pure.

use std::sync::Arc;

use nemo_core::catalog::{SearchKind, SearchResults};
use nemo_core::track::Track;

/// Quiet time after the last keystroke before a search runs.
pub const DEBOUNCE_MS: u64 = 300;
/// Most songs listed in the Songs group of the search results.
pub const SONGS_SHOWN: usize = 10;

/// What pressing Enter does.
#[derive(Debug, Clone, PartialEq)]
pub enum Submit {
    /// Nothing to do (empty box).
    Nothing,
    /// Run this search now; the top song plays when the answer arrives.
    Run(String),
    /// Play this list from its first song.
    Play(Vec<Track>),
}

/// State of the query box and its answer.
#[derive(Debug, Clone, Default)]
pub struct SearchState {
    /// Text in the box.
    pub input: String,
    seq: u64,
    /// The query the current `results` (or `error`) belong to.
    pub answered: Option<String>,
    /// Latest answer; kept while a newer search runs so the page does not flash empty.
    pub results: Option<Arc<SearchResults>>,
    /// The latest search failed (short message).
    pub error: Option<String>,
    /// A search is running.
    pub loading: bool,
    running: Option<String>,
    play_top: bool,
    /// The picked filter chip; `None` is "All".
    pub filter: Option<SearchKind>,
    /// The latest filtered answer.
    pub filtered: Option<Filtered>,
    filter_running: Option<(String, SearchKind)>,
}

/// The answer of one filtered search.
#[derive(Debug, Clone)]
pub struct Filtered {
    /// The query it belongs to.
    pub query: String,
    /// The filter it belongs to.
    pub kind: SearchKind,
    /// The answer or a short error.
    pub result: Result<Arc<SearchResults>, String>,
}

impl SearchState {
    /// The query the box asks for: trimmed text.
    #[must_use]
    pub fn query(&self) -> &str {
        self.input.trim()
    }

    /// `true` when the visible answer belongs to the current text.
    #[must_use]
    pub fn is_current(&self) -> bool {
        self.answered.as_deref() == Some(self.query())
    }

    /// The text changed. Returns the sequence number to debounce, or `None` when the box is empty
    /// (the page returns to its idle state).
    pub fn input(&mut self, text: String) -> Option<u64> {
        self.input = text;
        self.seq += 1;
        self.play_top = false;
        self.clear_filter();
        if self.query().is_empty() {
            self.results = None;
            self.answered = None;
            self.error = None;
            self.loading = false;
            self.running = None;
            return None;
        }
        Some(self.seq)
    }

    /// The debounce timer for `seq` fired. Returns the query to run, if it is still wanted.
    pub fn debounced(&mut self, seq: u64) -> Option<String> {
        if seq != self.seq {
            return None;
        }
        self.start()
    }

    /// Set the text from outside (a route) and run at once.
    pub fn set(&mut self, text: &str) -> Option<String> {
        if self.query() != text.trim() {
            self.clear_filter();
        }
        text.clone_into(&mut self.input);
        self.seq += 1;
        self.start()
    }

    /// Run the current text unless that answer is already here or on its way.
    fn start(&mut self) -> Option<String> {
        let q = self.query().to_owned();
        if q.is_empty()
            || self.answered.as_deref() == Some(&q)
            || self.running.as_deref() == Some(&q)
        {
            return None;
        }
        self.loading = true;
        self.error = None;
        self.running = Some(q.clone());
        Some(q)
    }

    /// Forget every answer, so the current text runs again from nothing (the source switch: an
    /// answer of one service must not stay on screen for another).
    pub fn forget_answers(&mut self) {
        self.clear_filter();
        self.results = None;
        self.answered = None;
        self.error = None;
        self.loading = false;
        self.running = None;
    }

    /// Re-run the current text after an error.
    pub fn retry(&mut self) -> Option<String> {
        self.answered = None;
        self.running = None;
        self.start()
    }

    /// Enter was pressed.
    pub fn submit(&mut self) -> Submit {
        if self.query().is_empty() {
            return Submit::Nothing;
        }
        if self.is_current()
            && let Some(r) = &self.results
            && !r.songs.is_empty()
        {
            return Submit::Play(r.songs.clone());
        }
        self.play_top = true;
        match self.start() {
            Some(q) => Submit::Run(q),
            // Already on its way: the answer will play.
            None => Submit::Nothing,
        }
    }

    fn clear_filter(&mut self) {
        self.filter = None;
        self.filtered = None;
        self.filter_running = None;
    }

    /// The filtered answer of the current text and filter, if it is here.
    #[must_use]
    pub fn filtered_now(&self) -> Option<&Filtered> {
        let kind = self.filter?;
        self.filtered
            .as_ref()
            .filter(|f| f.kind == kind && f.query == self.query())
    }

    /// `true` while the filtered search of the current text and filter runs.
    #[must_use]
    pub fn filter_loading(&self) -> bool {
        self.filter
            .is_some_and(|k| self.filter_running.as_ref() == Some(&(self.query().to_owned(), k)))
    }

    /// The results on screen: the filtered answer when a chip is picked, else the main one.
    #[must_use]
    pub fn shown(&self) -> Option<&Arc<SearchResults>> {
        if self.filter.is_some() {
            return self.filtered_now().and_then(|f| f.result.as_ref().ok());
        }
        self.results.as_ref()
    }

    /// A chip was picked (`None`: "All"). Returns the filtered search to run, unless its
    /// answer is here or on its way. Picking a chip whose search failed runs it again.
    pub fn pick(&mut self, kind: Option<SearchKind>) -> Option<(String, SearchKind)> {
        self.filter = kind;
        let kind = kind?;
        let q = self.query().to_owned();
        if q.is_empty() || self.filter_running.as_ref() == Some(&(q.clone(), kind)) {
            return None;
        }
        if self.filtered_now().is_some_and(|f| f.result.is_ok()) {
            return None;
        }
        self.filter_running = Some((q.clone(), kind));
        Some((q, kind))
    }

    /// A filtered answer arrived. One for an older text or another chip is dropped.
    pub fn filter_done(
        &mut self,
        query: &str,
        kind: SearchKind,
        result: Result<Arc<SearchResults>, String>,
    ) {
        if self
            .filter_running
            .as_ref()
            .is_some_and(|(q, k)| q == query && *k == kind)
        {
            self.filter_running = None;
        }
        if query != self.query() || self.filter != Some(kind) {
            return;
        }
        self.filtered = Some(Filtered {
            query: query.to_owned(),
            kind,
            result,
        });
    }

    /// An answer arrived. Returns the songs to play when Enter asked for the top song.
    pub fn done(
        &mut self,
        query: &str,
        result: Result<Arc<SearchResults>, String>,
    ) -> Option<Vec<Track>> {
        if self.running.as_deref() == Some(query) {
            self.running = None;
        }
        if query != self.query() {
            // Stale: the text moved on. The newer search (if any) keeps `loading`.
            return None;
        }
        self.loading = false;
        self.answered = Some(query.to_owned());
        match result {
            Ok(r) => {
                self.error = None;
                self.results = Some(Arc::clone(&r));
                if std::mem::take(&mut self.play_top) && !r.songs.is_empty() {
                    return Some(r.songs.clone());
                }
            }
            Err(e) => {
                self.error = Some(e);
                self.play_top = false;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock;

    fn results(n: usize) -> Arc<SearchResults> {
        Arc::new(SearchResults {
            songs: (0..n).map(mock::track).collect(),
            ..SearchResults::default()
        })
    }

    #[test]
    fn only_the_last_keystroke_runs_a_search() {
        let mut s = SearchState::default();
        let a = s.input("lo".into()).expect("seq");
        let b = s.input("lofi".into()).expect("seq");
        assert_eq!(s.debounced(a), None, "an older timer is ignored");
        assert_eq!(s.debounced(b), Some("lofi".to_owned()));
        assert!(s.loading);
        assert_eq!(s.debounced(b), None, "already running");
    }

    #[test]
    fn clearing_the_box_returns_to_idle() {
        let mut s = SearchState::default();
        let a = s.input("x".into()).expect("seq");
        s.debounced(a);
        s.done("x", Ok(results(2)));
        assert!(s.results.is_some());
        assert_eq!(s.input("  ".into()), None);
        assert!(s.results.is_none() && !s.loading && s.answered.is_none());
    }

    #[test]
    fn stale_answers_are_dropped() {
        let mut s = SearchState::default();
        let a = s.input("ab".into()).expect("seq");
        s.debounced(a);
        let b = s.input("abc".into()).expect("seq");
        s.debounced(b);
        assert_eq!(s.done("ab", Ok(results(3))), None);
        assert!(
            s.results.is_none() && s.loading,
            "newer search still running"
        );
        s.done("abc", Ok(results(1)));
        assert!(s.is_current() && !s.loading);
    }

    #[test]
    fn enter_plays_the_top_song_after_the_answer() {
        let mut s = SearchState::default();
        s.input("jazz".into());
        assert_eq!(s.submit(), Submit::Run("jazz".to_owned()));
        let songs = s.done("jazz", Ok(results(3))).expect("play");
        assert_eq!(songs.len(), 3);
        // A second Enter plays straight from the answer.
        assert!(matches!(s.submit(), Submit::Play(v) if v.len() == 3));
    }

    #[test]
    fn enter_with_no_songs_or_an_error_plays_nothing() {
        let mut s = SearchState::default();
        assert_eq!(s.submit(), Submit::Nothing);
        s.input("zzz".into());
        s.submit();
        assert_eq!(s.done("zzz", Ok(results(0))), None);
        s.input("yyy".into());
        s.submit();
        assert_eq!(s.done("yyy", Err("offline".into())), None);
        assert_eq!(s.error.as_deref(), Some("offline"));
        assert_eq!(s.retry(), Some("yyy".to_owned()));
        assert!(s.error.is_none() && s.loading);
    }

    #[test]
    fn a_chip_runs_its_search_once_and_typing_returns_to_all() {
        let mut s = SearchState::default();
        s.set("hvl");
        s.done("hvl", Ok(results(2)));
        assert_eq!(
            s.pick(Some(SearchKind::Songs)),
            Some(("hvl".to_owned(), SearchKind::Songs))
        );
        assert!(s.filter_loading() && s.shown().is_none());
        assert_eq!(s.pick(Some(SearchKind::Songs)), None, "already running");
        s.filter_done("hvl", SearchKind::Songs, Ok(results(5)));
        assert_eq!(s.shown().map(|r| r.songs.len()), Some(5));
        assert_eq!(s.pick(Some(SearchKind::Songs)), None, "answer is here");
        assert_eq!(s.pick(None), None);
        assert_eq!(
            s.shown().map(|r| r.songs.len()),
            Some(2),
            "All shows the main answer"
        );
        s.input("hvl x".into());
        assert_eq!(s.filter, None);
        assert!(s.filtered.is_none());
    }

    #[test]
    fn a_stale_or_failed_filtered_answer_is_handled() {
        let mut s = SearchState::default();
        s.set("a");
        s.pick(Some(SearchKind::Albums));
        s.pick(Some(SearchKind::Artists));
        s.filter_done("a", SearchKind::Albums, Ok(results(1)));
        assert!(s.filtered.is_none(), "another chip is picked now");
        s.filter_done("a", SearchKind::Artists, Err("offline".into()));
        assert!(s.filtered_now().is_some_and(|f| f.result.is_err()));
        assert_eq!(
            s.pick(Some(SearchKind::Artists)),
            Some(("a".to_owned(), SearchKind::Artists)),
            "picking again retries"
        );
    }

    #[test]
    fn set_from_a_route_runs_at_once() {
        let mut s = SearchState::default();
        assert_eq!(s.set("Nova Lane"), Some("Nova Lane".to_owned()));
        assert_eq!(s.set("Nova Lane"), None);
    }
}
