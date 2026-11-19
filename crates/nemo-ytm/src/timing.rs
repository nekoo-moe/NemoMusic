//! Start-up stage timing at debug level (fast-start spec). Read the `nemo::start` lines with
//! `NEMO_LOG=debug`. A line holds a fixed stage name, a duration in milliseconds and whether
//! the stage worked. Never a URL, an id or an error text.

use std::time::Instant;

/// Log that `stage` ended now, `since` it began, and whether it gave a stream.
pub(crate) fn stage(stage: &'static str, since: Instant, ok: bool) {
    tracing::debug!(
        target: "nemo::start",
        stage,
        ms = since.elapsed().as_millis(),
        ok,
        "start stage"
    );
}

/// Collects the tracing events of a closure as text, for tests that check what is logged.
///
/// One process-wide subscriber (installed on first use) keeps every event with the id of the
/// thread that logged it. A scoped subscriber would miss events whose call site another test
/// thread registered first.
#[cfg(test)]
pub(crate) mod capture {
    use std::fmt::Write as _;
    use std::sync::{Mutex, Once, PoisonError};
    use std::thread::{self, ThreadId};

    use tracing::field::{Field, Visit};
    use tracing::span::{Attributes, Id, Record};
    use tracing::{Event, Metadata, Subscriber};

    static EVENTS: Mutex<Vec<(ThreadId, String)>> = Mutex::new(Vec::new());
    static INSTALL: Once = Once::new();

    struct Everything;

    struct Text(String);

    impl Visit for Text {
        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            let _ = write!(self.0, " {}={value:?}", field.name());
        }
    }

    impl Subscriber for Everything {
        fn enabled(&self, _: &Metadata<'_>) -> bool {
            true
        }
        fn new_span(&self, _: &Attributes<'_>) -> Id {
            Id::from_u64(1)
        }
        fn record(&self, _: &Id, _: &Record<'_>) {}
        fn record_follows_from(&self, _: &Id, _: &Id) {}
        fn event(&self, event: &Event<'_>) {
            let mut text = Text(event.metadata().target().to_owned());
            event.record(&mut text);
            EVENTS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push((thread::current().id(), text.0));
        }
        fn enter(&self, _: &Id) {}
        fn exit(&self, _: &Id) {}
    }

    /// Run `run` and return every tracing event (any level) it logged on this thread, one
    /// text each.
    pub(crate) fn lines_of(run: impl FnOnce()) -> Vec<String> {
        INSTALL.call_once(|| {
            // Another global subscriber would be a different test setup; ignore the error.
            let _ = tracing::subscriber::set_global_default(Everything);
        });
        let me = thread::current().id();
        EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|(thread, _)| *thread != me);
        run();
        let events = EVENTS.lock().unwrap_or_else(PoisonError::into_inner);
        events
            .iter()
            .filter(|(thread, _)| *thread == me)
            .map(|(_, line)| line.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn a_stage_line_holds_only_the_stage_the_time_and_the_outcome() {
        let lines = capture::lines_of(|| stage("resolve native", Instant::now(), true));
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert!(line.starts_with("nemo::start"), "{line}");
        for field in ["stage=", "ms=", "ok=", "message="] {
            assert!(line.contains(field), "{line}");
        }
        assert_eq!(line.matches('=').count(), 4, "no other field: {line}");
    }
}
