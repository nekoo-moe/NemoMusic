//! The contract of the listening-stats service: the local play log and its aggregates.
//!
//! No I/O here. `nemo-stats` implements [`StatsService`] over its own SQLite file; the UI reads
//! charts, the clock, the year in music, recaps and the receipt through it. [`NoStats`] stands
//! in while recording is off or the store could not open.

use std::path::Path;
use std::time::Duration;

use crate::BoxFuture;
use crate::track::Track;

/// The span of time a chart covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatsPeriod {
    /// The last 7 days.
    Week,
    /// The last 30 days.
    Month,
    /// The last 365 days.
    Year,
    /// Every play ever logged.
    AllTime,
    /// One calendar month in local time.
    MonthOf {
        /// The year, for example 2026.
        year: i32,
        /// The month, 1 to 12.
        month: u8,
    },
    /// One calendar year in local time.
    YearOf(i32),
}

/// One song of a chart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedSong {
    /// The song as last played.
    pub track: Track,
    /// Plays in the period.
    pub plays: u32,
    /// Time heard in the period.
    pub heard: Duration,
}

/// One artist or album of a chart.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RankedName {
    /// The name; `None` shows as "Unknown artist" or "Unknown album".
    pub name: Option<String>,
    /// Plays in the period.
    pub plays: u32,
}

/// Plays per hour of the day, in local time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct HourClock {
    /// Plays that started in each hour, index 0 is midnight to 1 am.
    pub plays: [u32; 24],
}

/// The year in music.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YearSummary {
    /// The calendar year.
    pub year: i32,
    /// Plays in the year.
    pub plays: u32,
    /// Time heard in the year.
    pub heard: Duration,
    /// The top songs.
    pub songs: Vec<RankedSong>,
    /// The top artists.
    pub artists: Vec<RankedName>,
    /// The top albums.
    pub albums: Vec<RankedName>,
    /// The month with the most plays (1 to 12) and its plays.
    pub busiest_month: (u8, u32),
    /// The hour with the most plays (0 to 23) and its plays.
    pub busiest_hour: (u8, u32),
}

/// A monthly recap: the top songs of one month.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recap {
    /// The year.
    pub year: i32,
    /// The month, 1 to 12.
    pub month: u8,
    /// The top songs of the month.
    pub songs: Vec<RankedSong>,
}

/// The file format of an export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExportFormat {
    /// Comma-separated values.
    Csv,
    /// A JSON array.
    Json,
}

/// A shareable receipt image of a chart, made in memory on demand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    /// The PNG bytes.
    pub png: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// A stats request failed. The texts hold no paths, titles or SQL.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StatsError {
    /// No stats store is running (recording was never set up, or the store could not open).
    #[error("listening stats are not available")]
    Unavailable,
    /// The database failed.
    #[error("stats database error: {0}")]
    Database(String),
    /// Writing an export or a receipt failed.
    #[error("stats file error: {0}")]
    Io(String),
    /// The period has no plays to export or draw.
    #[error("no plays in this period")]
    Empty,
}

/// The local play log and every aggregate over it. Every query runs off the UI thread.
pub trait StatsService: Send + Sync {
    /// Whether plays are being recorded.
    fn recording(&self) -> bool;

    /// Start or stop recording; the choice is stored.
    ///
    /// # Errors
    /// A [`StatsError`] when the choice cannot be stored.
    fn set_recording(&self, on: bool) -> BoxFuture<'_, Result<(), StatsError>>;

    /// The most played songs of `period`, at most `limit`.
    ///
    /// # Errors
    /// A [`StatsError`] when the store fails or is not available.
    fn top_songs(
        &self,
        period: StatsPeriod,
        limit: usize,
    ) -> BoxFuture<'_, Result<Vec<RankedSong>, StatsError>>;

    /// The most played artists of `period`, at most `limit`.
    ///
    /// # Errors
    /// A [`StatsError`] when the store fails or is not available.
    fn top_artists(
        &self,
        period: StatsPeriod,
        limit: usize,
    ) -> BoxFuture<'_, Result<Vec<RankedName>, StatsError>>;

    /// The most played albums of `period`, at most `limit`.
    ///
    /// # Errors
    /// A [`StatsError`] when the store fails or is not available.
    fn top_albums(
        &self,
        period: StatsPeriod,
        limit: usize,
    ) -> BoxFuture<'_, Result<Vec<RankedName>, StatsError>>;

    /// Plays per hour of the day in `period`.
    ///
    /// # Errors
    /// A [`StatsError`] when the store fails or is not available.
    fn clock(&self, period: StatsPeriod) -> BoxFuture<'_, Result<HourClock, StatsError>>;

    /// The months and years that have plays, for the period picker.
    ///
    /// # Errors
    /// A [`StatsError`] when the store fails or is not available.
    fn periods_with_plays(&self) -> BoxFuture<'_, Result<Vec<StatsPeriod>, StatsError>>;

    /// The year in music for `year`; `None` when the year has no plays.
    ///
    /// # Errors
    /// A [`StatsError`] when the store fails or is not available.
    fn year_in_music(&self, year: i32) -> BoxFuture<'_, Result<Option<YearSummary>, StatsError>>;

    /// Whether monthly recaps are made.
    fn monthly_recaps(&self) -> bool;

    /// Switch monthly recaps on or off; the choice is stored.
    ///
    /// # Errors
    /// A [`StatsError`] when the choice cannot be stored.
    fn set_monthly_recaps(&self, on: bool) -> BoxFuture<'_, Result<(), StatsError>>;

    /// The recap of one month; `None` when none was made.
    ///
    /// # Errors
    /// A [`StatsError`] when the store fails or is not available.
    fn recap(&self, year: i32, month: u8) -> BoxFuture<'_, Result<Option<Recap>, StatsError>>;

    /// Draw the receipt of the top `limit` songs of `period`.
    ///
    /// # Errors
    /// [`StatsError::Empty`] when the period has no plays, or another [`StatsError`] when the
    /// store or the drawing fails.
    fn receipt(
        &self,
        period: StatsPeriod,
        limit: usize,
    ) -> BoxFuture<'_, Result<Receipt, StatsError>>;

    /// Export every play to `dest` in `format`; returns the rows written.
    ///
    /// # Errors
    /// [`StatsError::Io`] when the file cannot be written, or another [`StatsError`] when the
    /// store fails.
    fn export<'a>(
        &'a self,
        format: ExportFormat,
        dest: &'a Path,
    ) -> BoxFuture<'a, Result<u64, StatsError>>;

    /// Delete every logged play and recap; returns the plays deleted.
    ///
    /// # Errors
    /// A [`StatsError`] when the store fails or is not available.
    fn clear(&self) -> BoxFuture<'_, Result<u64, StatsError>>;
}

/// A stats service with nothing behind it: recording is off and every request fails with
/// [`StatsError::Unavailable`].
#[derive(Debug, Clone, Copy, Default)]
pub struct NoStats;

fn unavailable<'a, T: Send + 'a>() -> BoxFuture<'a, Result<T, StatsError>> {
    Box::pin(async { Err(StatsError::Unavailable) })
}

impl StatsService for NoStats {
    fn recording(&self) -> bool {
        false
    }

    fn set_recording(&self, _on: bool) -> BoxFuture<'_, Result<(), StatsError>> {
        unavailable()
    }

    fn top_songs(
        &self,
        _period: StatsPeriod,
        _limit: usize,
    ) -> BoxFuture<'_, Result<Vec<RankedSong>, StatsError>> {
        unavailable()
    }

    fn top_artists(
        &self,
        _period: StatsPeriod,
        _limit: usize,
    ) -> BoxFuture<'_, Result<Vec<RankedName>, StatsError>> {
        unavailable()
    }

    fn top_albums(
        &self,
        _period: StatsPeriod,
        _limit: usize,
    ) -> BoxFuture<'_, Result<Vec<RankedName>, StatsError>> {
        unavailable()
    }

    fn clock(&self, _period: StatsPeriod) -> BoxFuture<'_, Result<HourClock, StatsError>> {
        unavailable()
    }

    fn periods_with_plays(&self) -> BoxFuture<'_, Result<Vec<StatsPeriod>, StatsError>> {
        unavailable()
    }

    fn year_in_music(&self, _year: i32) -> BoxFuture<'_, Result<Option<YearSummary>, StatsError>> {
        unavailable()
    }

    fn monthly_recaps(&self) -> bool {
        false
    }

    fn set_monthly_recaps(&self, _on: bool) -> BoxFuture<'_, Result<(), StatsError>> {
        unavailable()
    }

    fn recap(&self, _year: i32, _month: u8) -> BoxFuture<'_, Result<Option<Recap>, StatsError>> {
        unavailable()
    }

    fn receipt(
        &self,
        _period: StatsPeriod,
        _limit: usize,
    ) -> BoxFuture<'_, Result<Receipt, StatsError>> {
        unavailable()
    }

    fn export<'a>(
        &'a self,
        _format: ExportFormat,
        _dest: &'a Path,
    ) -> BoxFuture<'a, Result<u64, StatsError>> {
        unavailable()
    }

    fn clear(&self) -> BoxFuture<'_, Result<u64, StatsError>> {
        unavailable()
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::path::Path;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use super::{ExportFormat, NoStats, StatsError, StatsPeriod, StatsService};

    /// Poll a future that is ready at once.
    fn ready<T>(fut: impl Future<Output = T>) -> T {
        match pin!(fut).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(v) => v,
            Poll::Pending => panic!("NoStats answers at once"),
        }
    }

    fn refused<T>(fut: impl Future<Output = Result<T, StatsError>>) -> bool {
        matches!(ready(fut), Err(StatsError::Unavailable))
    }

    #[test]
    fn no_stats_records_nothing() {
        let s = NoStats;
        assert!(!s.recording());
        assert!(!s.monthly_recaps());
        assert!(refused(s.set_recording(true)));
        assert!(refused(s.set_monthly_recaps(true)));
        assert!(!s.recording() && !s.monthly_recaps());
    }

    #[test]
    fn no_stats_refuses_every_query() {
        let s = NoStats;
        let p = StatsPeriod::MonthOf {
            year: 2026,
            month: 11,
        };
        assert!(refused(s.top_songs(p, 10)));
        assert!(refused(s.top_artists(StatsPeriod::Week, 10)));
        assert!(refused(s.top_albums(StatsPeriod::AllTime, 10)));
        assert!(refused(s.clock(StatsPeriod::YearOf(2026))));
        assert!(refused(s.periods_with_plays()));
        assert!(refused(s.year_in_music(2026)));
        assert!(refused(s.recap(2026, 11)));
        assert!(refused(s.receipt(StatsPeriod::Month, 10)));
        assert!(refused(s.export(ExportFormat::Csv, Path::new("plays.csv"))));
        assert!(refused(s.clear()));
    }
}
