//! Per-widget animation state of an `Fx`: hover and press springs, press and focus flags.

use std::time::Instant;

use iced::advanced::widget::operation::Focusable;

use crate::anim;
use crate::motion::Spring;

pub(super) struct State {
    pub(super) hover: Spring,
    pub(super) press: Spring,
    pub(super) down: bool,
    pub(super) last: Option<Instant>,
    /// Keyboard focus (set by Tab through the focus operations); draws the ring.
    pub(super) focused: bool,
}

impl Focusable for State {
    fn is_focused(&self) -> bool {
        self.focused
    }

    fn focus(&mut self) {
        self.focused = true;
    }

    fn unfocus(&mut self) {
        self.focused = false;
    }
}

impl Default for State {
    fn default() -> Self {
        let (k, z) = anim::EFFECTS_FAST;
        Self {
            hover: Spring::tuned(0.0, k, z),
            press: Spring::tuned(0.0, k, z),
            down: false,
            last: None,
            focused: false,
        }
    }
}

impl State {
    pub(super) fn moving(&self) -> bool {
        !self.hover.is_settled() || !self.press.is_settled()
    }
}
