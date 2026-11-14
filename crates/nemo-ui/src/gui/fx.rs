//! `Fx`: one wrapper widget that gives its child the Material motion of this app.
//!
//! * Animated **state layers**: a rounded overlay that fades to 8% on hover and 10% on press with
//!   the `effects.fast` spring.
//! * A hover **lift**: the child rises a few px with a soft shadow (`spatial.fast`).
//! * An **entrance**: a fade and a slide up, driven from outside by a 0 to 1 progress.
//!
//! The widget owns its hover and press springs. It asks for a redraw only while a spring still
//! moves, so an idle window draws no frames. With reduced motion every value jumps to its target.
//! The child keeps its own layout and events; `Fx` only watches the cursor.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use iced::{Color, Element};

use crate::anim;

static REDUCED_MOTION: AtomicBool = AtomicBool::new(false);

static FOCUS_RGBA: AtomicU32 = AtomicU32::new(0x8A_B4_F8_FF);

/// Colour of the keyboard focus ring (`design.md` section 5: 2 px `primary`, 2 px offset). The
/// app sets it to the cover's accent before each frame.
pub fn set_focus_color(c: Color) {
    FOCUS_RGBA.store(u32::from_be_bytes(c.into_rgba8()), Ordering::Relaxed);
}

fn focus_color() -> Color {
    let [red, green, blue, alpha] = FOCUS_RGBA.load(Ordering::Relaxed).to_be_bytes();
    Color::from_rgba8(red, green, blue, f32::from(alpha) / 255.0)
}

/// Tell every [`Fx`] whether motion is reduced (the app sets it before each frame is built).
pub fn set_reduced_motion(on: bool) {
    REDUCED_MOTION.store(on, Ordering::Relaxed);
}

/// `true` when motion is reduced.
#[must_use]
pub fn reduced_motion() -> bool {
    REDUCED_MOTION.load(Ordering::Relaxed)
}

/// State layer opacity on hover. M3 says 8%; iced blends in linear light, which makes a white
/// layer read about twice as strong, so the drawn value is 0.045 (measured: the result sits at
/// about 8% in sRGB terms).
pub const HOVER: f32 = 0.045;
/// State layer opacity on press (M3: 10%, same correction).
pub const PRESS: f32 = 0.06;

mod badge;
mod state;
mod widget;

pub use badge::{Badge, BadgeAnchor};

/// Wrap `content` in an [`Fx`].
pub fn fx<'a, M: 'a>(content: impl Into<Element<'a, M>>) -> Fx<'a, M> {
    Fx::new(content)
}

/// See the module docs.
pub struct Fx<'a, M> {
    content: Element<'a, M>,
    layer: Color,
    radius: f32,
    lift: f32,
    interactive: bool,
    progress: f32,
    slide: f32,
    fade_to: Color,
    badge: Option<Badge<'a, M>>,
    scale: f32,
    zoom: f32,
    pop: f32,
}

impl<'a, M: 'a> Fx<'a, M> {
    /// A wrapper with state layers on `on-surface`, radius 0.
    pub fn new(content: impl Into<Element<'a, M>>) -> Self {
        Self {
            content: content.into(),
            layer: Color::from_rgb8(0xF2, 0xF3, 0xF5),
            radius: 0.0,
            lift: 0.0,
            interactive: true,
            progress: 1.0,
            slide: 0.0,
            fade_to: Color::TRANSPARENT,
            badge: None,
            scale: 1.0,
            zoom: 0.0,
            pop: 0.0,
        }
    }

    /// A fixed scale around the centre of the child (the cover breathing with play and pause).
    #[must_use]
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// Grow by this fraction on hover (0.03 is 3%).
    #[must_use]
    pub fn zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom;
        self
    }

    /// Start the entrance this fraction smaller (0.02: it grows from 98% to 100%).
    #[must_use]
    pub fn pop(mut self, pop: f32) -> Self {
        self.pop = pop;
        self
    }

    /// A play button that fades in on hover and sends its own message.
    #[must_use]
    pub fn badge(mut self, badge: Badge<'a, M>) -> Self {
        self.badge = Some(badge);
        self
    }

    /// Corner radius of the state layer (match the child's).
    #[must_use]
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Colour of the state layer.
    #[must_use]
    pub fn layer(mut self, layer: Color) -> Self {
        self.layer = layer;
        self
    }

    /// Rise by `px` on hover, with a shadow.
    #[must_use]
    pub fn lift(mut self, px: f32) -> Self {
        self.lift = px;
        self
    }

    /// Entrance progress (1 = settled): the child slides up `slide` px and fades from `fade_to`,
    /// the colour of whatever is behind it.
    #[must_use]
    pub fn entrance(mut self, progress: f32, slide: f32, fade_to: Color) -> Self {
        self.progress = progress.clamp(0.0, 1.0);
        self.slide = slide;
        self.fade_to = fade_to;
        self
    }

    /// No hover and press layers (an entrance-only wrapper).
    #[must_use]
    pub fn passive(mut self) -> Self {
        self.interactive = false;
        self
    }
}

/// Entrance-only wrapper: `progress` from [`anim::Anim::reveal`], sliding [`anim::SLIDE_PX`].
pub fn entrance<'a, M: 'a>(
    content: impl Into<Element<'a, M>>,
    progress: f32,
    fade_to: Color,
) -> Element<'a, M> {
    if progress >= 0.999 {
        return content.into();
    }
    Fx::new(content)
        .passive()
        .entrance(progress, anim::SLIDE_PX, fade_to)
        .pop(anim::POP)
        .into()
}

impl<'a, M: 'a> From<Fx<'a, M>> for Element<'a, M> {
    fn from(fx: Fx<'a, M>) -> Self {
        Element::new(fx)
    }
}
