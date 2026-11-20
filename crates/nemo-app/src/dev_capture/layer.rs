//! The `tracing` layer that fills the developer ring, and its filter.

use std::fmt::Debug;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::field::{Field, Visit};
use tracing::{Event, Level, Metadata, Subscriber};
use tracing_subscriber::filter::{FilterFn, LevelFilter, filter_fn};
use tracing_subscriber::layer::{Context, Layer};

use super::redact::Redactor;
use super::ring::{CapturedEvent, DevRing};

/// The target of provider trace events (`nemo_core::dev::TRACE_TARGET`).
pub const TRACE_TARGET: &str = "nemo_trace";

/// The structured fields the ring keeps. Anything else is dropped at capture.
pub const ALLOWED_FIELDS: [&str; 16] = [
    "provider", "op", "host", "status", "ms", "bytes", "tier", "attempt", "rest", "kind", "stage",
    "outcome", "error", "reason", "count", "version",
];

/// The longest message kept, in characters.
const MESSAGE_MAX: usize = 300;

/// The longest field value kept, in characters.
const VALUE_MAX: usize = 160;

/// Milliseconds since the Unix epoch.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| u64::try_from(d.as_millis()).ok())
        .unwrap_or(0)
}

/// Which events the capture layer takes: provider trace events at `debug` or above, and every
/// other event up to `info`, or up to `log_level` when that is more verbose. The level hint
/// keeps `trace` callsites of other crates off.
#[must_use]
pub fn capture_filter(log_level: Level) -> FilterFn<impl Fn(&Metadata<'_>) -> bool> {
    let ceiling = if log_level > Level::INFO {
        log_level
    } else {
        Level::INFO
    };
    let hint = if ceiling > Level::DEBUG {
        ceiling
    } else {
        Level::DEBUG
    };
    filter_fn(move |meta: &Metadata<'_>| {
        if meta.target() == TRACE_TARGET {
            *meta.level() <= Level::DEBUG
        } else {
            *meta.level() <= ceiling
        }
    })
    .with_max_level_hint(LevelFilter::from_level(hint))
}

/// A `tracing` layer that redacts events and stores them in a [`DevRing`].
#[derive(Debug, Clone)]
pub struct CaptureLayer {
    ring: DevRing,
    redactor: Arc<Redactor>,
    clock: fn() -> u64,
}

impl CaptureLayer {
    /// A layer that fills `ring`, redacting with this user's home folder.
    #[must_use]
    pub fn new(ring: DevRing) -> Self {
        Self::with_parts(ring, Redactor::new(), now_ms)
    }

    /// A layer with its own redactor and clock (tests).
    #[must_use]
    pub fn with_parts(ring: DevRing, redactor: Redactor, clock: fn() -> u64) -> Self {
        Self {
            ring,
            redactor: Arc::new(redactor),
            clock,
        }
    }
}

impl<S: Subscriber> Layer<S> for CaptureLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let mut visitor = Collector::default();
        event.record(&mut visitor);
        let fields = visitor
            .fields
            .into_iter()
            .map(|(name, value)| (name, cut(&self.redactor.redact(&value), VALUE_MAX)))
            .collect();
        self.ring.push(CapturedEvent {
            at_ms: (self.clock)(),
            level: *meta.level(),
            target: meta.target().to_owned(),
            message: cut(&self.redactor.redact(&visitor.message), MESSAGE_MAX),
            fields,
        });
    }
}

/// Collects the message and the allow-listed fields of one event as text.
#[derive(Default)]
struct Collector {
    message: String,
    fields: Vec<(&'static str, String)>,
}

impl Collector {
    fn keep(&mut self, field: &Field, value: String) {
        if field.name() == "message" {
            self.message = value;
        } else if let Some(name) = ALLOWED_FIELDS.iter().find(|n| **n == field.name()) {
            self.fields.push((name, value));
        }
    }
}

impl Visit for Collector {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.keep(field, value.to_owned());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.keep(field, value.to_string());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.keep(field, value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.keep(field, value.to_string());
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.keep(field, format!("{value:.1}"));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        self.keep(field, format!("{value:?}"));
    }
}

/// Keep at most `max` characters of `text`, ending with `…` when it was cut.
fn cut(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut short: String = text.chars().take(max.saturating_sub(1)).collect();
    short.push('…');
    short
}

#[cfg(test)]
mod tests;
