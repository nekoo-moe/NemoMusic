//! The hidden developer panel (dev-tools spec): its state, the click counter that unlocks it and
//! its messages.
//!
//! Nothing here touches the file system, the network or the log: the panel reads one
//! [`DevSnapshot`] from the app's [`DevProbe`] and its only effects are the clipboard write of
//! `Copy report` and the probe's `clear`.
//!
//! [`DevProbe`]: nemo_core::dev::DevProbe

use std::time::Instant;

use nemo_core::dev::{DevLevel, DevLogLine, DevSnapshot, DevTraceRow};

use super::{App, Job};
use crate::motion::Spring;
use crate::state::Event;

pub mod report;

#[cfg(test)]
mod tests;

pub use report::UiFacts;

/// Clicks on the version text that open the panel.
pub const CLICKS_TO_UNLOCK: u8 = 10;

/// A click counts toward the same run only if it comes within this many ms of the last one.
pub const CLICK_RESET_MS: u64 = 3000;

/// The toast shown when the panel unlocks.
pub const UNLOCK_TOAST: &str = "Developer tools";

/// The toast shown after `Copy report`.
pub const COPIED_TOAST: &str = "Copied";

/// Counts the clicks on the version text. Pure: the caller gives the time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClickCounter {
    count: u8,
    last_ms: Option<u64>,
}

impl ClickCounter {
    /// Register a click at `now_ms`. Returns `true` on the click that completes a run of
    /// [`CLICKS_TO_UNLOCK`]; the count then starts again, so the next run needs ten more.
    pub fn click(&mut self, now_ms: u64) -> bool {
        let within = self
            .last_ms
            .is_some_and(|last| now_ms.saturating_sub(last) <= CLICK_RESET_MS);
        self.count = if within {
            self.count.saturating_add(1)
        } else {
            1
        };
        self.last_ms = Some(now_ms);
        if self.count >= CLICKS_TO_UNLOCK {
            *self = Self::default();
            return true;
        }
        false
    }

    /// The clicks counted so far in this run.
    #[must_use]
    pub const fn count(&self) -> u8 {
        self.count
    }
}

/// A tab of the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DevTab {
    /// Provider requests.
    #[default]
    Trace,
    /// Log lines.
    Log,
    /// One card per provider.
    Providers,
    /// Sign-in diagnostics.
    Account,
    /// Version, OS, config, hosts and output.
    System,
}

impl DevTab {
    /// Every tab, in the order of the chips.
    pub const ALL: [Self; 5] = [
        Self::Trace,
        Self::Log,
        Self::Providers,
        Self::Account,
        Self::System,
    ];

    /// The chip text.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Trace => "Trace",
            Self::Log => "Log",
            Self::Providers => "Providers",
            Self::Account => "Account",
            Self::System => "System",
        }
    }

    /// The labels of all tabs.
    #[must_use]
    pub fn labels() -> [&'static str; 5] {
        Self::ALL.map(Self::label)
    }

    /// The position of this tab among [`DevTab::ALL`].
    #[must_use]
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

/// The log levels the chips offer, from error to trace.
pub const LEVELS: [DevLevel; 5] = [
    DevLevel::Error,
    DevLevel::Warn,
    DevLevel::Info,
    DevLevel::Debug,
    DevLevel::Trace,
];

/// What a row of the trace is called, to find it again when new rows arrive.
pub type RowKey = (u64, String, String);

/// A message of the panel.
#[derive(Debug, Clone, PartialEq)]
pub enum DevMsg {
    /// A click on the version text of About.
    Click,
    /// `Ctrl+Shift+D`.
    Toggle,
    /// Close the panel.
    Close,
    /// Pick a tab by its chip index.
    Tab(usize),
    /// Show only this provider in the trace (`None`: all).
    Provider(Option<String>),
    /// Show only failed calls.
    ErrorsOnly(bool),
    /// Select a trace row (`None`: clear the selection).
    Select(Option<RowKey>),
    /// Show log lines up to this level.
    Level(DevLevel),
    /// The text filter of the log.
    LogFilter(String),
    /// Freeze or release the log list.
    Pause(bool),
    /// Copy the redacted report to the clipboard.
    Copy,
    /// Empty the ring.
    Clear,
}

/// The state of the panel.
pub struct DevState {
    /// Ten clicks happened in this run of the app.
    pub(crate) unlocked: bool,
    /// The panel is open.
    pub(crate) open: bool,
    /// The tab shown.
    pub(crate) tab: DevTab,
    /// The last snapshot read from the probe.
    pub(crate) snap: DevSnapshot,
    /// Trace filter: one provider.
    pub(crate) provider: Option<String>,
    /// Trace filter: failed calls only.
    pub(crate) errors_only: bool,
    /// The selected trace row.
    pub(crate) selected: Option<RowKey>,
    /// Log filter: the most verbose level shown.
    pub(crate) level: DevLevel,
    /// Log filter: text a line must contain (any case).
    pub(crate) filter: String,
    /// The log list is frozen.
    pub(crate) paused: bool,
    /// The sliding pill under the tabs: left edge and width.
    pub(crate) pill: (Spring, Spring),
    clicks: ClickCounter,
    epoch: Instant,
}

impl Default for DevState {
    fn default() -> Self {
        Self {
            unlocked: false,
            open: false,
            tab: DevTab::default(),
            snap: DevSnapshot::default(),
            provider: None,
            errors_only: false,
            selected: None,
            level: DevLevel::Info,
            filter: String::new(),
            paused: false,
            pill: (
                Spring::tuned(0.0, 1400.0, 1.0),
                Spring::tuned(0.0, 1400.0, 1.0),
            ),
            clicks: ClickCounter::default(),
            epoch: Instant::now(),
        }
    }
}

impl DevState {
    /// A click on the version text now. `true` when it unlocks the panel.
    pub(crate) fn click(&mut self) -> bool {
        let now = u64::try_from(self.epoch.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.clicks.click(now)
    }

    /// Take a new snapshot. While the log is paused its lines stay as they were.
    pub(crate) fn refresh(&mut self, fresh: DevSnapshot) {
        let frozen = self.paused.then(|| std::mem::take(&mut self.snap.log));
        self.snap = fresh;
        if let Some(log) = frozen {
            self.snap.log = log;
        }
    }

    /// The trace rows that pass the filters, newest first.
    #[must_use]
    pub fn trace_rows(&self) -> Vec<&DevTraceRow> {
        self.snap
            .trace
            .iter()
            .filter(|r| self.provider.as_deref().is_none_or(|p| p == r.provider))
            .filter(|r| !self.errors_only || r.failed)
            .collect()
    }

    /// The log lines that pass the filters, newest first.
    #[must_use]
    pub fn log_lines(&self) -> Vec<&DevLogLine> {
        let needle = self.filter.trim().to_lowercase();
        self.snap
            .log
            .iter()
            .filter(|l| l.level <= self.level)
            .filter(|l| {
                needle.is_empty()
                    || l.text.to_lowercase().contains(&needle)
                    || l.target.to_lowercase().contains(&needle)
            })
            .collect()
    }

    /// The selected trace row, if it is still in the snapshot.
    #[must_use]
    pub fn selected_row(&self) -> Option<&DevTraceRow> {
        let (at, provider, op) = self.selected.as_ref()?;
        self.snap
            .trace
            .iter()
            .find(|r| r.at_ms == *at && r.provider == *provider && r.op == *op)
    }

    /// `true` while the pill under the tabs moves.
    #[must_use]
    pub(crate) fn moving(&self) -> bool {
        self.open && (!self.pill.0.is_settled() || !self.pill.1.is_settled())
    }
}

/// Where the text of `Copy report` goes. The app gives it to the iced clipboard; tests give a
/// fake.
pub trait ClipboardSink {
    /// Put `text` on the clipboard.
    fn write(&mut self, text: String);
}

/// The sink of the running app: keeps the text until `update` turns it into a clipboard task.
#[derive(Debug, Default)]
pub struct PendingCopy(pub Option<String>);

impl ClipboardSink for PendingCopy {
    fn write(&mut self, text: String) {
        self.0 = Some(text);
    }
}

impl App {
    /// Handle one message of the panel.
    pub(super) fn dev_msg(&mut self, msg: DevMsg) -> Vec<Job> {
        match msg {
            DevMsg::Click => {
                if self.dev.click() {
                    self.dev.unlocked = true;
                    self.dev_set_open(true);
                    return self.dispatch(Event::Notice(UNLOCK_TOAST.to_owned()));
                }
            }
            DevMsg::Toggle => {
                if self.dev.unlocked {
                    self.dev_set_open(!self.dev.open);
                }
            }
            DevMsg::Close => self.dev_set_open(false),
            DevMsg::Tab(i) => {
                if let Some(tab) = DevTab::ALL.get(i) {
                    self.dev.tab = *tab;
                }
            }
            DevMsg::Provider(p) => self.dev.provider = p,
            DevMsg::ErrorsOnly(on) => self.dev.errors_only = on,
            DevMsg::Select(key) => self.dev.selected = key,
            DevMsg::Level(level) => self.dev.level = level,
            DevMsg::LogFilter(text) => self.dev.filter = text,
            DevMsg::Pause(on) => {
                self.dev.paused = on;
                if !on {
                    self.dev_refresh();
                }
            }
            DevMsg::Copy => {
                let mut sink = PendingCopy::default();
                self.dev_copy(&mut sink);
                self.dev_copied = sink.0;
                return self.dispatch(Event::Notice(COPIED_TOAST.to_owned()));
            }
            DevMsg::Clear => {
                if let Some(probe) = &self.backend.dev {
                    probe.clear();
                }
                self.dev_refresh();
            }
        }
        Vec::new()
    }

    fn dev_set_open(&mut self, open: bool) {
        self.dev.open = open;
        if open {
            self.dev_refresh();
        }
    }

    /// Open the panel on `tab`, as if it had been unlocked (headless snapshots).
    pub fn open_dev_for_snapshot(&mut self, tab: DevTab) {
        self.dev.unlocked = true;
        self.dev.tab = tab;
        self.dev_set_open(true);
        self.sync_dev();
    }

    /// Read a fresh snapshot, if the app has a probe.
    pub(super) fn dev_refresh(&mut self) {
        if let Some(probe) = &self.backend.dev {
            let fresh = probe.snapshot();
            self.dev.refresh(fresh);
        }
    }

    /// The once-a-second tick while the panel is open.
    pub(super) fn dev_tick(&mut self) {
        if self.dev.open {
            self.dev_refresh();
        }
    }

    /// Build the report and hand it to `sink`: one redacted plain-text report of all tabs.
    pub fn dev_copy(&mut self, sink: &mut dyn ClipboardSink) {
        self.dev_refresh();
        let facts = UiFacts {
            frames: self.frame_stats.readout().map(|r| r.text()),
            window: format!(
                "{} x {} px, scale {}%",
                self.window.width, self.window.height, self.scale_pct
            ),
            reduce_motion: self.ui.reduced_motion,
        };
        let text = report::text(&self.dev.snap, &facts);
        let text = match &self.backend.dev {
            Some(probe) => probe.redact(&text),
            None => text,
        };
        sink.write(text);
    }

    /// Point the pill under the tabs at the open tab.
    pub(super) fn sync_dev(&mut self) {
        use super::super::views::kit::chip_geometry;
        let (x, w) = chip_geometry(&DevTab::labels(), self.dev.tab.index());
        let (sx, sw) = &mut self.dev.pill;
        sx.target = x;
        sw.target = w;
        if !self.dev.open || self.ui.reduced_motion {
            sx.snap(x);
            sw.snap(w);
        }
    }

    /// Step the pill under the tabs by `dt_ms`.
    pub(super) fn step_dev(&mut self, dt_ms: f32) {
        let dt = dt_ms.min(50.0) / 1000.0;
        self.dev.pill.0.step(dt);
        self.dev.pill.1.step(dt);
    }
}
