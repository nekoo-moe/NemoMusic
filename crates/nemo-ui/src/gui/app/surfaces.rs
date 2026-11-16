//! Enter and exit of the surfaces that float over a screen (app-motion spec, task 2): the toast,
//! the track menu, the signal-path and Timing popovers, the tools panel, the Create playlist and
//! shortcut sheets and the first-run notice. Each has a [`Presence`]; a surface that closed is
//! still drawn while it fades out, from a copy of what it showed (the toast and the menu clear
//! their state at once).

use nemo_core::track::Track;

use super::App;
use crate::motion::Presence;
use crate::state::Toast;

/// See the module docs.
#[derive(Debug, Clone)]
pub struct Surfaces {
    /// The notice above the bar.
    pub toast: Presence,
    /// The track menu.
    pub menu: Presence,
    /// The signal-path popover.
    pub signal: Presence,
    /// The Timing popover of the full-window lyrics.
    pub timing: Presence,
    /// The tools panel.
    pub tools: Presence,
    /// The Create playlist sheet.
    pub create: Presence,
    /// The shortcut sheet.
    pub shortcuts: Presence,
    /// The first-run notice.
    pub tos: Presence,
    /// The toast drawn now (it stays while the toast fades out).
    toast_shown: Option<Toast>,
    /// The track of the menu drawn now.
    menu_shown: Option<Track>,
}

impl Default for Surfaces {
    fn default() -> Self {
        Self {
            toast: Presence::popup(),
            menu: Presence::popup(),
            signal: Presence::popup(),
            timing: Presence::popup(),
            tools: Presence::popup(),
            create: Presence::popup(),
            shortcuts: Presence::popup(),
            tos: Presence::popup(),
            toast_shown: None,
            menu_shown: None,
        }
    }
}

impl Surfaces {
    fn all(&mut self) -> [&mut Presence; 8] {
        [
            &mut self.toast,
            &mut self.menu,
            &mut self.signal,
            &mut self.timing,
            &mut self.tools,
            &mut self.create,
            &mut self.shortcuts,
            &mut self.tos,
        ]
    }

    /// Advance every surface by `dt_ms`.
    pub fn advance(&mut self, dt_ms: f32) {
        for p in self.all() {
            p.advance(dt_ms);
        }
    }

    /// Jump every surface to its end (headless runs).
    pub fn finish(&mut self) {
        for p in self.all() {
            p.finish();
        }
    }

    /// `true` while no surface moves.
    #[must_use]
    pub fn settled(&self) -> bool {
        [
            &self.toast,
            &self.menu,
            &self.signal,
            &self.timing,
            &self.tools,
            &self.create,
            &self.shortcuts,
            &self.tos,
        ]
        .iter()
        .all(|p| p.is_settled())
    }

    /// The toast to draw (the live one, or the one fading out), or `None`.
    #[must_use]
    pub fn toast_shown(&self) -> Option<&Toast> {
        if self.toast.visible() {
            self.toast_shown.as_ref()
        } else {
            None
        }
    }

    /// The track of the menu to draw, or `None`.
    #[must_use]
    pub fn menu_shown(&self) -> Option<&Track> {
        if self.menu.visible() {
            self.menu_shown.as_ref()
        } else {
            None
        }
    }

    /// Follow a toast: a new text enters from the start; none fades the last one out. Copies
    /// only when the text changes, so a frame allocates nothing.
    fn follow_toast(&mut self, live: Option<&Toast>, reduced: bool) {
        match live {
            Some(t) => {
                let same = self
                    .toast_shown
                    .as_ref()
                    .is_some_and(|s| s.text == t.text && s.undo == t.undo);
                if !same {
                    self.toast_shown = Some(t.clone());
                    self.toast.restart(reduced);
                }
                self.toast.set(true, reduced);
            }
            None => self.toast.set(false, reduced),
        }
    }

    fn follow_menu(&mut self, live: Option<&Track>, reduced: bool) {
        match live {
            Some(t) => {
                let same = self.menu_shown.as_ref().is_some_and(|s| s.id == t.id);
                if !same {
                    self.menu_shown = Some(t.clone());
                    self.menu.restart(reduced);
                }
                self.menu.set(true, reduced);
            }
            None => self.menu.set(false, reduced),
        }
    }
}

impl App {
    /// Point every surface at what the state shows. Cheap; runs after every update and frame.
    pub(super) fn sync_surfaces(&mut self) {
        let r = self.ui.reduced_motion;
        let tos = self.ui.tos_sheet_visible();
        let s = &mut self.surfaces;
        s.follow_toast(self.ui.toast.as_ref(), r);
        s.follow_menu(self.ui.menu.as_ref(), r);
        s.signal.set(self.ui.signal_open, r);
        s.timing.set(self.ui.timing_open, r);
        s.tools.set(self.tools_open, r);
        s.create.set(self.create_open, r);
        s.shortcuts.set(self.shortcuts_open, r);
        s.tos.set(tos, r);
    }

    /// Rewind the page entrance and the enter of every open surface to `ms` after they started
    /// (headless snapshots of a mid-transition frame).
    pub fn freeze_mid_motion(&mut self, ms: f32) {
        self.anim.set_reduced(false);
        self.anim.restart_page();
        self.anim.step(ms);
        for p in self.surfaces.all() {
            if p.is_on() {
                p.restart(false);
                p.advance(ms);
            }
        }
    }
}

#[cfg(test)]
mod tests;
