//! Start-up stage timing at debug level (fast-start spec): where the time goes between a
//! Play command and the first audible sample. Run with `NEMO_LOG=debug` and read the
//! `nemo::start` lines.
//!
//! A line holds a fixed stage name and a duration in milliseconds. Never a URL, an id, a title
//! or any other text of the song. Nothing here runs on the audio thread.

use std::time::Instant;

/// Log that `stage` ended now, `since` it began.
pub(crate) fn stage(stage: &'static str, since: Instant) {
    tracing::debug!(
        target: "nemo::start",
        stage,
        ms = since.elapsed().as_millis(),
        "start stage"
    );
}
