//! Left zone of the capsule: the cover (opens Now Playing), title over artist, the like heart.

use iced::widget::{Space, button, column, row};
use iced::{Alignment, Color, Element};

use super::control::like_button;
use super::cover_art;
use crate::format;
use crate::gui::app::{App, Message};
use crate::gui::style::{animated, col, fit, gap, label, tip};
use crate::theme::{color, shape, type_scale};

/// Cover size.
pub const COVER: f32 = 56.0;
/// Target of the heart button.
const HEART: f32 = 36.0;

/// Width left for the title and artist in a zone `width` px wide: the zone less cover, the gap
/// after it, the gap before the heart and the heart.
#[must_use]
pub fn text_width(width: f32) -> f32 {
    (width - COVER - gap::MD - gap::XS - HEART).max(48.0)
}

/// The zone, `width` px wide.
pub(super) fn view(app: &App, width: f32) -> Element<'_, Message> {
    let Some(track) = &app.ui.snapshot.current else {
        return Space::new().into();
    };
    let text_w = text_width(width);
    // The title and artist fade in with a new song.
    let fade = app.title_fade();
    let texts = column![
        label(
            fit(&track.title, text_w, type_scale::TITLE_SM.size),
            type_scale::TITLE_SM,
            true,
            col(color::on_surface()).scale_alpha(fade)
        ),
        label(
            fit(
                &format::artists(&track.artists),
                text_w,
                type_scale::BODY_SM.size
            ),
            type_scale::BODY_SM,
            false,
            col(color::on_surface_variant()).scale_alpha(fade)
        ),
    ];
    let open = button(
        row![
            cover_art(app, COVER, shape::md()),
            Space::new().width(gap::MD),
            texts
        ]
        .align_y(Alignment::Center),
    )
    .padding(0)
    .on_press(Message::OpenNowPlaying);
    row![
        tip(
            animated(open, Color::TRANSPARENT, shape::md()),
            "Show Now Playing"
        ),
        Space::new().width(gap::XS),
        like_button(app, track, 20.0, HEART),
    ]
    .align_y(Alignment::Center)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_gets_what_the_cover_and_heart_leave() {
        assert!((text_width(264.0) - 156.0).abs() < f32::EPSILON);
        assert!((text_width(220.0) - 112.0).abs() < f32::EPSILON);
        assert!(text_width(10.0) >= 48.0);
    }
}
