//! The play button of the capsule: a 56 px accent disc. The glyph shrinks away and the other
//! one grows in, driven by the play spring (the one expressive overshoot, `design.md` section
//! 8); while a stream starts the glyph is the calm busy ring. Scale, not opacity: SVG tint alpha
//! is not honoured by the software renderer.

use iced::widget::{button, container};
use iced::{Alignment, Element, Length};
use nemo_core::player::PlayState;

use crate::anim::play_radius;
use crate::gui::app::{App, Message};
use crate::gui::assets::Icon;
use crate::gui::fx::fx;
use crate::gui::style::{button_flat, col, icon, spinner, tip};
use crate::keys::KeyAction;
use crate::palette::Palette;
use crate::state::Event;

/// Diameter of the disc.
pub const SIZE: f32 = 56.0;
/// Size of the glyph inside it.
const GLYPH: f32 = 28.0;

/// What the disc shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    /// Paused or stopped: pressing plays.
    Play,
    /// Playing: pressing pauses.
    Pause,
    /// A stream is starting: the busy ring; pressing still pauses.
    Busy,
}

impl Glyph {
    /// The glyph for a player state.
    #[must_use]
    pub const fn of(state: PlayState) -> Self {
        match state {
            PlayState::Playing => Self::Pause,
            PlayState::Buffering => Self::Busy,
            PlayState::Paused | PlayState::Stopped => Self::Play,
        }
    }

    /// Tooltip and accessible name: what pressing does.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Play => "Play",
            Self::Pause | Self::Busy => "Pause",
        }
    }
}

/// Smallest glyph scale, reached halfway through the morph, where the glyph swaps.
const SWAP_SCALE: f32 = 0.55;

/// The glyph to draw and its scale for a morph value (0 paused, 1 playing). The expressive
/// spring overshoots, so the value is clamped. Play shrinks to [`SWAP_SCALE`] at the middle,
/// where pause takes over and grows to full size.
#[must_use]
pub fn morph(value: f32) -> (Glyph, f32) {
    let m = value.clamp(0.0, 1.0);
    let glyph = if m < 0.5 { Glyph::Play } else { Glyph::Pause };
    (
        glyph,
        SWAP_SCALE + (1.0 - SWAP_SCALE) * (2.0 * m - 1.0).abs(),
    )
}

/// The play button.
pub fn view(app: &App, accent: Palette) -> Element<'_, Message> {
    let glyph = Glyph::of(app.ui.snapshot.state);
    let ink = col(accent.on_primary);
    let inner: Element<'_, Message> = if glyph == Glyph::Busy {
        spinner(GLYPH, ink, app.spin).into()
    } else {
        let (shown, scale) = morph(app.anim.play.value);
        let glyph_icon = if shown == Glyph::Pause {
            Icon::Pause
        } else {
            Icon::Play
        };
        icon(glyph_icon, GLYPH * scale, ink).into()
    };
    let radius = play_radius(SIZE, app.anim.play.value);
    let b = button(
        container(inner)
            .width(Length::Fixed(SIZE))
            .height(Length::Fixed(SIZE))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center),
    )
    .padding(0)
    .on_press(Message::Ui(Event::Key(KeyAction::TogglePlay)));
    tip(
        fx(b.style(button_flat(col(accent.primary), radius)))
            .layer(ink)
            .radius(radius),
        glyph.name(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_glyph_follows_the_player_state() {
        assert_eq!(Glyph::of(PlayState::Playing), Glyph::Pause);
        assert_eq!(Glyph::of(PlayState::Paused), Glyph::Play);
        assert_eq!(Glyph::of(PlayState::Stopped), Glyph::Play);
        assert_eq!(Glyph::of(PlayState::Buffering), Glyph::Busy);
    }

    #[test]
    fn pressing_a_busy_disc_pauses() {
        assert_eq!(Glyph::Busy.name(), "Pause");
        assert_eq!(Glyph::Play.name(), "Play");
        assert_eq!(Glyph::Pause.name(), "Pause");
    }

    #[test]
    fn the_glyph_swaps_in_the_middle_and_never_leaves_its_range() {
        let near = |a: f32, b: f32| (a - b).abs() < 1e-4;
        let (g, s) = morph(0.0);
        assert!(g == Glyph::Play && near(s, 1.0));
        let (g, s) = morph(1.0);
        assert!(g == Glyph::Pause && near(s, 1.0));
        // The spring overshoots both ways: clamped to the end states.
        let (g, s) = morph(1.12);
        assert!(g == Glyph::Pause && near(s, 1.0));
        let (g, s) = morph(-0.08);
        assert!(g == Glyph::Play && near(s, 1.0));
        let (g, s) = morph(0.49);
        assert!(g == Glyph::Play && (SWAP_SCALE..1.0).contains(&s));
        let (g, s) = morph(0.5);
        assert!(g == Glyph::Pause && near(s, SWAP_SCALE));
    }
}
