use std::io::Write;
use std::sync::{Arc, Mutex};

use tracing::Level;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::fmt::writer::BoxMakeWriter;

use super::{ALLOWED_FIELDS, CaptureLayer, TRACE_TARGET, cut};
use crate::dev_capture::{DevRing, Redactor};
use crate::logging::build;

/// A text sink that tests can read back.
#[derive(Clone, Default)]
struct Sink(Arc<Mutex<Vec<u8>>>);

impl Sink {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().expect("sink")).into_owned()
    }
}

impl Write for Sink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("sink").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Sink {
    type Writer = Sink;

    fn make_writer(&'a self) -> Sink {
        self.clone()
    }
}

fn fixed_clock() -> u64 {
    1_700_000_000_000
}

fn layer(ring: &DevRing) -> CaptureLayer {
    CaptureLayer::with_parts(
        ring.clone(),
        Redactor::with_home("/home/alice"),
        fixed_clock,
    )
}

/// One call site, used under several subscribers by the interest test.
fn emit_trace() {
    tracing::debug!(
        target: "nemo_trace",
        provider = "ytm",
        op = "search",
        host = "music.youtube.com",
        status = 200_u16,
        ms = 41_u64,
        tier = "native",
        "request"
    );
}

#[test]
fn trace_events_reach_the_ring_at_an_info_text_level_and_not_the_file() {
    let ring = DevRing::new(10);
    let sink = Sink::default();
    let subscriber = build(BoxMakeWriter::new(sink.clone()), Level::INFO, layer(&ring));
    tracing::subscriber::with_default(subscriber, || {
        emit_trace();
        tracing::info!("lifecycle line");
    });
    let events = ring.snapshot();
    let trace = events
        .iter()
        .find(|e| e.target == TRACE_TARGET)
        .expect("the trace event is in the ring");
    assert_eq!(trace.level, Level::DEBUG);
    assert_eq!(trace.at_ms, fixed_clock());
    assert_eq!(trace.message, "request");
    assert_eq!(trace.field("provider"), Some("ytm"));
    assert_eq!(trace.field("op"), Some("search"));
    assert_eq!(trace.field("host"), Some("music.youtube.com"));
    assert_eq!(trace.field("status"), Some("200"));
    assert_eq!(trace.field("ms"), Some("41"));
    assert_eq!(trace.field("tier"), Some("native"));
    assert!(events.iter().any(|e| e.message == "lifecycle line"));
    // The file keeps its `info` behaviour: the trace line is not in it.
    let file = sink.text();
    assert!(file.contains("lifecycle line"), "{file}");
    assert!(!file.contains("nemo_trace"), "{file}");
    assert!(!file.contains("music.youtube.com"), "{file}");
}

#[test]
fn a_call_site_first_seen_by_an_info_only_subscriber_is_not_hidden() {
    // Call site interest is cached per call site. Use the same call site first under a
    // subscriber that turns it off, then under the real one.
    let sink = Sink::default();
    let info_only = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_writer(sink.clone())
        .finish();
    tracing::subscriber::with_default(info_only, emit_trace);
    assert_eq!(sink.text(), "");

    let ring = DevRing::new(10);
    let subscriber = build(
        BoxMakeWriter::new(Sink::default()),
        Level::INFO,
        layer(&ring),
    );
    tracing::subscriber::with_default(subscriber, emit_trace);
    assert_eq!(ring.len(), 1, "the cached interest hid the trace event");
}

#[test]
fn other_debug_events_stay_out_at_info_and_come_in_at_debug() {
    for (level, expected) in [(Level::INFO, 0), (Level::DEBUG, 1)] {
        let ring = DevRing::new(10);
        let subscriber = build(BoxMakeWriter::new(Sink::default()), level, layer(&ring));
        tracing::subscriber::with_default(subscriber, || {
            tracing::debug!(target: "some_crate::module", "chatty");
            tracing::trace!(target: "some_crate::module", "very chatty");
        });
        assert_eq!(ring.len(), expected, "level {level}");
    }
}

#[test]
fn warnings_and_errors_are_always_captured() {
    let ring = DevRing::new(10);
    let subscriber = build(
        BoxMakeWriter::new(Sink::default()),
        Level::ERROR,
        layer(&ring),
    );
    tracing::subscriber::with_default(subscriber, || {
        tracing::warn!("careful");
        tracing::error!("broken");
    });
    let levels: Vec<Level> = ring.snapshot().iter().map(|e| e.level).collect();
    assert_eq!(levels, vec![Level::WARN, Level::ERROR]);
}

#[test]
fn fields_off_the_allow_list_are_dropped_and_values_are_redacted() {
    let ring = DevRing::new(10);
    let subscriber = build(
        BoxMakeWriter::new(Sink::default()),
        Level::INFO,
        layer(&ring),
    );
    tracing::subscriber::with_default(subscriber, || {
        tracing::warn!(
            track_title = "Secret Song",
            token = "ya29.abc",
            error = %"error sending request for url (https://api.example.com/v1/x?key=SECRET1): timed out",
            path = "/home/alice/Music",
            kind = "timeout",
            "call at https://u:p@host.example/path?q=SECRET2 for me@example.com failed"
        );
    });
    let events = ring.snapshot();
    let event = events.first().expect("one event");
    let names: Vec<&str> = event.fields.iter().map(|(n, _)| *n).collect();
    assert_eq!(names, vec!["error", "kind"]);
    assert!(names.iter().all(|n| ALLOWED_FIELDS.contains(n)));
    assert_eq!(
        event.field("error"),
        Some("error sending request for url (api.example.com): timed out")
    );
    assert_eq!(event.message, "call at host.example for [email] failed");
    let all = format!("{event:?}");
    for secret in ["Secret Song", "ya29", "SECRET1", "SECRET2", "alice", "u:p"] {
        assert!(!all.contains(secret), "{secret} in {all}");
    }
}

#[test]
fn long_texts_are_cut() {
    let ring = DevRing::new(10);
    let subscriber = build(
        BoxMakeWriter::new(Sink::default()),
        Level::INFO,
        layer(&ring),
    );
    let long = "word ".repeat(200);
    tracing::subscriber::with_default(subscriber, || {
        tracing::warn!(error = %long, "{long}");
    });
    let events = ring.snapshot();
    let event = events.first().expect("one event");
    assert_eq!(event.message.chars().count(), 300);
    assert!(event.message.ends_with('…'));
    assert_eq!(event.field("error").map(|v| v.chars().count()), Some(160));
}

#[test]
fn cut_keeps_short_text_and_counts_characters() {
    assert_eq!(cut("abc", 3), "abc");
    assert_eq!(cut("abcd", 3), "ab…");
    assert_eq!(cut("日本語日本語", 4), "日本語…");
}

#[test]
fn every_trace_field_name_is_on_the_allow_list() {
    for name in [
        "provider", "op", "host", "status", "ms", "bytes", "tier", "attempt", "rest",
    ] {
        assert!(ALLOWED_FIELDS.contains(&name), "{name}");
    }
}
