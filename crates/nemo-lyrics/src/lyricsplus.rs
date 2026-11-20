//! `LyricsPlus` provider: the community lyrics relay behind the `YouLy+` extension (opt-in,
//! default off).
//!
//! Facts read on 2026-11-15 from <https://github.com/ibratabian17/LyricsPlus> (Apache-2.0,
//! `docs/endpoints.md`, `internal/domain/v2.go`, `internal/orchestrator`) and from five
//! recorded answers:
//! - `GET /v2/lyrics/get?title=&artist=&album=&duration=&source=` needs no key, cookie or
//!   account. `duration` is in seconds. The server looks in its own cache first, then asks
//!   Apple Music, its user-submitted database, Deezer and QQ Music with its own accounts and
//!   returns the best timing it found.
//! - The answer is JSON: `type` (`Word`, `Line`, `None`), `lyrics[]` lines with `time` and
//!   `duration` in ms, `syllabus[]` timed pieces whose `text` keeps its trailing space,
//!   `isBackground` pieces for background vocals, `element.singer`, optional
//!   `transliteration`, and `processingTime.selectedSongMetadata` naming the matched song.
//! - A piece marked `synthetic` has timing the server spread by letter count, not timing from
//!   the source. Such a line keeps only its line timing here: no timing is made up (ADR 0002).
//! - 404 is a miss. When some source was skipped or timed out (`sourcesStatus` other than
//!   `BAD`), the miss is temporary and is reported as an error, so it is cached for minutes, not
//!   days. 429 and 503 carry `Retry-After`.
//! - The README says the lyrics are distributed unofficially and that the API grants no right
//!   to reproduce them. There are no terms for other apps. So it stays off until the user turns
//!   it on, with a risk note, and every doc names `LyricsPlus` and its upstream source.

use nemo_core::BoxFuture;
use nemo_core::lyrics::{
    CreditedDoc, DetailedDoc, Granularity, LyricsDoc, LyricsError, LyricsProvider, LyricsQuery,
    ProviderId, ProviderInfo,
};

use crate::http::{build_client, duration_secs, send_any, url_with};

mod parse;

pub use parse::{credit, miss_is_temporary, parse_response, upstream_name};

/// Provider id shown in the UI.
pub const LYRICSPLUS_ID: ProviderId = ProviderId::new("LyricsPlus");

/// Page the credit links to (no request is made to it).
pub const CREDIT_URL: &str = "https://github.com/ibratabian17/LyricsPlus";

/// Upstream sources asked, in order. This is the server's own default order, sent explicitly
/// so a server change cannot widen it.
pub const SOURCES: &str = "apple,lyricsplus,deezer,qq";

const URL: &str = "https://lyricsplus.prjktla.my.id/v2/lyrics/get";
const HOSTS: &[&str] = &["lyricsplus.prjktla.my.id"];
const RISK: &str = "Unofficial relay run by volunteers: it fetches Apple Music, Deezer and QQ Music lyrics with its own accounts, has no terms for other apps, and can stop or block without notice.";

/// The query parameters, or `None` when title or artist is unknown. Album and duration are
/// sent when known.
#[must_use]
pub fn query_params(query: &LyricsQuery) -> Option<Vec<(&'static str, String)>> {
    let title = query.title.trim();
    let artist = query.artists.first().map(|a| a.trim())?;
    if title.is_empty() || artist.is_empty() {
        return None;
    }
    let mut p = vec![("title", title.to_owned()), ("artist", artist.to_owned())];
    if let Some(album) = query
        .album
        .as_deref()
        .map(str::trim)
        .filter(|a| !a.is_empty())
    {
        p.push(("album", album.to_owned()));
    }
    if let Some(secs) = duration_secs(query.duration_ms) {
        p.push(("duration", secs.to_string()));
    }
    p.push(("source", SOURCES.to_owned()));
    Some(p)
}

/// Lyrics from the `LyricsPlus` relay.
#[derive(Debug, Clone)]
pub struct LyricsPlusProvider {
    client: reqwest::Client,
}

impl LyricsPlusProvider {
    /// Build the provider.
    ///
    /// # Errors
    /// [`LyricsError::Unavailable`] if the HTTP client cannot be built.
    pub fn new() -> Result<Self, LyricsError> {
        Ok(Self {
            client: build_client()?,
        })
    }

    async fn lookup(&self, query: &LyricsQuery) -> Result<Option<DetailedDoc>, LyricsError> {
        let Some(params) = query_params(query) else {
            return Ok(None);
        };
        let pairs: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let (status, body) =
            send_any("lyrics:lyricsplus", self.client.get(url_with(URL, &pairs)?)).await?;
        match status {
            200..=299 => parse_response(&body),
            404 if miss_is_temporary(&body) => Err(LyricsError::Unavailable(
                "LyricsPlus could not ask all its sources".to_owned(),
            )),
            // A miss, or a query the server refuses (too long, bad parameters).
            400 | 403 | 404 | 414 => Ok(None),
            other => Err(LyricsError::Network(format!("HTTP status {other}"))),
        }
    }
}

impl LyricsProvider for LyricsPlusProvider {
    fn id(&self) -> ProviderId {
        LYRICSPLUS_ID
    }

    fn hosts(&self) -> &'static [&'static str] {
        HOSTS
    }

    fn fetch<'a>(
        &'a self,
        query: &'a LyricsQuery,
    ) -> BoxFuture<'a, Result<Option<LyricsDoc>, LyricsError>> {
        Box::pin(async move { Ok(self.lookup(query).await?.map(|(doc, ..)| doc)) })
    }

    fn fetch_credited<'a>(
        &'a self,
        query: &'a LyricsQuery,
    ) -> BoxFuture<'a, Result<Option<CreditedDoc>, LyricsError>> {
        Box::pin(async move {
            let found = self.lookup(query).await?;
            Ok(found.map(|(doc, credit, _)| (doc, credit)))
        })
    }

    fn fetch_detailed<'a>(
        &'a self,
        query: &'a LyricsQuery,
    ) -> BoxFuture<'a, Result<Option<DetailedDoc>, LyricsError>> {
        Box::pin(self.lookup(query))
    }

    fn info(&self) -> ProviderInfo {
        ProviderInfo::opt_in(RISK)
    }

    /// Apple Music answers carry syllable pieces.
    fn best_granularity(&self) -> Granularity {
        Granularity::Syllable
    }
}

#[cfg(test)]
mod tests;
