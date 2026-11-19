//! Window glass (appearance-glass spec): the app window itself is created see-through and its
//! backdrop is drawn at a chosen alpha, so the desktop shows through the gaps and edges. This
//! module holds the numbers and the state logic; the views only read them. No GUI dependency.
//!
//! Blur behind the window is the desktop compositor's job. NemoMusic never draws it.

/// Lowest window opacity the setting allows, in percent.
pub const OPACITY_MIN: u8 = 40;
/// Highest window opacity the setting allows, in percent.
pub const OPACITY_MAX: u8 = 95;
/// Window opacity of a new install, in percent.
pub const OPACITY_DEFAULT: u8 = 80;
/// Lowest alpha of a pane over the glass. At this alpha both text inks keep 4.5:1 over any
/// desktop (see the tests).
pub const PANE_FLOOR: f32 = 0.80;

/// The opacity setting limited to [`OPACITY_MIN`] to [`OPACITY_MAX`].
#[must_use]
pub const fn clamp_opacity(percent: u8) -> u8 {
    if percent < OPACITY_MIN {
        OPACITY_MIN
    } else if percent > OPACITY_MAX {
        OPACITY_MAX
    } else {
        percent
    }
}

/// What the running window does with the glass setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlassState {
    /// The setting is off: a solid window.
    Off,
    /// The window is see-through at the chosen opacity.
    Clear,
    /// The setting is on, and Reduce transparency keeps the window solid.
    Overridden,
    /// The setting is on, but the window was created solid: see-through after a restart.
    NeedsRestart,
}

/// The inputs that decide how the window draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowGlass {
    /// The `Window glass` switch.
    pub enabled: bool,
    /// Window opacity in percent; limited by [`clamp_opacity`] when read.
    pub opacity: u8,
    /// The `Reduce transparency` switch.
    pub reduce_transparency: bool,
    /// The window was created transparent (the switch was on when the app started).
    pub created_transparent: bool,
}

impl WindowGlass {
    /// The state of the running window. Reduce transparency is named before the restart: it is
    /// the reason the window is solid now.
    #[must_use]
    pub const fn state(self) -> GlassState {
        if !self.enabled {
            GlassState::Off
        } else if self.reduce_transparency {
            GlassState::Overridden
        } else if !self.created_transparent {
            GlassState::NeedsRestart
        } else {
            GlassState::Clear
        }
    }

    /// `true` when the window draws see-through now.
    #[must_use]
    pub const fn is_clear(self) -> bool {
        matches!(self.state(), GlassState::Clear)
    }

    /// `true` when the switch is on but only a restart makes the window see-through.
    #[must_use]
    pub const fn needs_restart(self) -> bool {
        matches!(self.state(), GlassState::NeedsRestart)
    }

    /// The window opacity as a fraction, 1 while the window draws solid.
    #[must_use]
    pub fn opacity_fraction(self) -> f32 {
        f32::from(clamp_opacity(self.opacity)) / 100.0
    }

    /// Alpha of the backdrop fills (flat colour, colour fields, vignette). The full-screen Now
    /// Playing is always solid: its lyrics sit straight on the backdrop with no pane.
    #[must_use]
    pub fn backdrop_alpha(self, full_screen: bool) -> f32 {
        if self.is_clear() && !full_screen {
            self.opacity_fraction()
        } else {
            1.0
        }
    }

    /// Alpha of a pane whose normal alpha is `base`: raised to [`PANE_FLOOR`] over the glass.
    #[must_use]
    pub fn pane_alpha(self, base: f32) -> f32 {
        if self.is_clear() {
            base.max(PANE_FLOOR)
        } else {
            base
        }
    }

    /// One plain sentence about the running window, for the Appearance page.
    #[must_use]
    pub fn status(self) -> String {
        match self.state() {
            GlassState::Off => "This window is solid.".to_owned(),
            GlassState::Clear => format!(
                "This window is see-through at {} percent.",
                clamp_opacity(self.opacity)
            ),
            GlassState::Overridden => {
                "This window is solid because Reduce transparency is on.".to_owned()
            }
            GlassState::NeedsRestart => {
                "This window is solid until you restart NemoMusic.".to_owned()
            }
        }
    }
}

/// What the parent needs to put a blur behind the see-through window (bar-material spec): the
/// look of the theme and the opacity chosen. NemoMusic draws no blur itself; on Windows the
/// parent asks the system for one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowLook {
    /// The theme is dark (dark and AMOLED), so the system material should be dark too.
    pub dark: bool,
    /// `surface` of the theme, the base of the tint.
    pub base: crate::theme::Rgba,
    /// The `Window glass` opacity in percent.
    pub opacity: u8,
}

impl WindowLook {
    /// The tint as red, green, blue and alpha: the base colour, as strong as the opacity setting
    /// (a lower opacity lets more of the blurred desktop through).
    #[must_use]
    pub fn tint(self) -> (u8, u8, u8, u8) {
        let alpha = self
            .base
            .with_alpha(f32::from(clamp_opacity(self.opacity)) / 100.0);
        (self.base.r, self.base.g, self.base.b, alpha.a)
    }
}

#[cfg(test)]
mod tests;
