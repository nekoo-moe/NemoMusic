//! The plain-text report of `Copy report`: every tab as text, from one snapshot.
//!
//! The probe redacts the text again before it reaches the clipboard (`DevProbe::redact`).

use std::fmt::Write as _;

use nemo_core::dev::{
    DevAccount, DevLenClass, DevLevel, DevPathKind, DevProvider, DevSnapshot, DevSolverState,
    DevTraceRow,
};

/// The most trace rows and log lines in a report.
const REPORT_ROWS: usize = 200;

/// The text a fact shows when nobody could read it.
pub const UNKNOWN: &str = "Unknown";

/// Facts that only the window knows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UiFacts {
    /// The frame-time readout, once frames have been measured.
    pub frames: Option<String>,
    /// The window size and scale.
    pub window: String,
    /// Reduce motion is on.
    pub reduce_motion: bool,
}

/// `HH:MM:SS.mmm` (UTC) of a time in milliseconds since the Unix epoch.
#[must_use]
pub fn clock(at_ms: u64) -> String {
    let secs = at_ms / 1000;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        (secs / 3600) % 24,
        (secs / 60) % 60,
        secs % 60,
        at_ms % 1000
    )
}

/// A yes, a no or `Unknown`.
#[must_use]
pub fn yes_no(fact: Option<bool>) -> &'static str {
    match fact {
        Some(true) => "yes",
        Some(false) => "no",
        None => UNKNOWN,
    }
}

/// The size class as words.
#[must_use]
pub fn len_class(class: DevLenClass) -> &'static str {
    match class {
        DevLenClass::Unknown => UNKNOWN,
        DevLenClass::Absent => "none stored",
        DevLenClass::Short => "short (under 32)",
        DevLenClass::Medium => "medium (32 to 255)",
        DevLenClass::Long => "long (256 to 2047)",
        DevLenClass::VeryLong => "very long (2048 or more)",
    }
}

/// Where `yt-dlp` comes from, as words.
#[must_use]
pub fn path_kind(kind: DevPathKind) -> &'static str {
    match kind {
        DevPathKind::Unknown => UNKNOWN,
        DevPathKind::Missing => "not found",
        DevPathKind::Managed => "managed copy",
        DevPathKind::User => "path set by the user",
        DevPathKind::Path => "found on PATH",
    }
}

/// The in-process solver, as words.
#[must_use]
pub fn solver(state: DevSolverState) -> &'static str {
    match state {
        DevSolverState::Unknown => UNKNOWN,
        DevSolverState::Disabled => "off in settings",
        DevSolverState::AutoOff => "switched itself off for a day",
        DevSolverState::Active => "in use",
    }
}

/// A number of milliseconds, or `-`.
#[must_use]
pub fn ms(value: Option<u64>) -> String {
    value.map_or_else(|| "-".to_owned(), |v| format!("{v} ms"))
}

/// A time of day, or `-`.
#[must_use]
pub fn when(at_ms: Option<u64>) -> String {
    at_ms.map_or_else(|| "-".to_owned(), clock)
}

/// The level as a short word.
#[must_use]
pub fn level_word(level: DevLevel) -> &'static str {
    match level {
        DevLevel::Error => "ERROR",
        DevLevel::Warn => "WARN",
        DevLevel::Info => "INFO",
        DevLevel::Debug => "DEBUG",
        DevLevel::Trace => "TRACE",
    }
}

/// One trace row as a line.
#[must_use]
pub fn trace_line(row: &DevTraceRow) -> String {
    let mut line = format!(
        "{} {} {} {} {}",
        clock(row.at_ms),
        row.provider,
        row.op,
        if row.host.is_empty() { "-" } else { &row.host },
        row.status
    );
    for (name, value) in [
        ("ms", row.ms),
        ("bytes", row.bytes),
        ("attempt", row.attempt),
        ("rest", row.rest_s),
    ] {
        if let Some(v) = value {
            let _ = write!(line, " {name}={v}");
        }
    }
    if let Some(tier) = &row.tier {
        let _ = write!(line, " tier={tier}");
    }
    line
}

fn provider_line(p: &DevProvider) -> String {
    let mut line = format!(
        "{}: calls {}, errors {}, last ok {}, last error {}, p50 {}, p95 {}",
        p.name,
        p.calls,
        p.errors,
        when(p.last_ok_ms),
        p.last_error_kind.as_deref().unwrap_or("-"),
        ms(p.p50_ms),
        ms(p.p95_ms)
    );
    if let Some(until) = p.resting_until_ms {
        let _ = write!(line, ", resting until {}", clock(until));
    }
    line
}

fn account_line(a: &DevAccount) -> String {
    let cookies = if a.cookie_names.is_empty() {
        "-".to_owned()
    } else {
        a.cookie_names.join(", ")
    };
    format!(
        "{}: signed in {}, token {}, size {}, cookies {}, last stage {} {} at {}",
        a.site,
        yes_no(a.signed_in),
        yes_no(a.token_present),
        len_class(a.token_len),
        cookies,
        a.last_stage.as_deref().unwrap_or("-"),
        a.last_outcome.as_deref().unwrap_or("-"),
        when(a.last_at_ms)
    )
}

/// The whole report.
#[must_use]
pub fn text(snap: &DevSnapshot, ui: &UiFacts) -> String {
    let sys = &snap.system;
    let mut out = String::new();
    let _ = writeln!(out, "NemoMusic developer report");
    let _ = writeln!(
        out,
        "version {}, {}, {} build",
        sys.version, sys.os, sys.profile
    );
    let _ = writeln!(
        out,
        "taken {} UTC, up {} s",
        clock(snap.taken_ms),
        sys.uptime_s
    );
    let _ = writeln!(
        out,
        "window {}, reduce motion {}",
        ui.window,
        if ui.reduce_motion { "on" } else { "off" }
    );
    let _ = writeln!(
        out,
        "frames {}",
        ui.frames.as_deref().unwrap_or("measuring")
    );
    let _ = writeln!(out, "\n== Providers ==");
    for p in &snap.providers {
        let _ = writeln!(out, "{}", provider_line(p));
    }
    let r = &snap.resolver;
    let _ = writeln!(out, "\n== Resolver ==");
    for t in &r.tiers {
        let _ = writeln!(out, "tier {}: ok {}, failed {}", t.name, t.ok, t.failed);
    }
    let _ = writeln!(
        out,
        "cache: memory hits {}, disk hits {}, misses {}, size {}",
        r.cache_hits,
        r.cache_disk_hits,
        r.cache_misses,
        r.cache_size
            .map_or_else(|| UNKNOWN.to_owned(), |n| n.to_string())
    );
    let _ = writeln!(out, "in-process solver: {}", solver(r.solver));
    let _ = writeln!(
        out,
        "yt-dlp: {}, build {}, version {}",
        path_kind(r.ytdlp_path),
        r.ytdlp_build.as_deref().unwrap_or(UNKNOWN),
        r.ytdlp_version.as_deref().unwrap_or(UNKNOWN)
    );
    let _ = writeln!(out, "\n== Account ==");
    for a in &snap.accounts {
        let _ = writeln!(out, "{}", account_line(a));
    }
    let _ = writeln!(out, "\n== System ==");
    let _ = writeln!(out, "audio: {}", sys.audio.as_deref().unwrap_or(UNKNOWN));
    for c in &sys.config {
        let _ = writeln!(
            out,
            "config {} = {}",
            c.key,
            c.value.as_deref().unwrap_or("(set)")
        );
    }
    for e in &sys.egress {
        let _ = writeln!(out, "host {} x{}", e.host, e.count);
    }
    let _ = writeln!(out, "\n== Trace (newest first) ==");
    for row in snap.trace.iter().take(REPORT_ROWS) {
        let _ = writeln!(out, "{}", trace_line(row));
    }
    let _ = writeln!(out, "\n== Log (newest first) ==");
    for line in snap.log.iter().take(REPORT_ROWS) {
        let _ = writeln!(
            out,
            "{} {} {} {}",
            clock(line.at_ms),
            level_word(line.level),
            line.target,
            line.text
        );
    }
    out
}
