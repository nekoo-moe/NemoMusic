//! Centre zone of the capsule: shuffle, previous, play, next, repeat, and the seek row under them.

use iced::widget::{Space, column, container, row, stack};
use iced::{Alignment, Color, Element, Length};
use nemo_core::player::{PlayerCommand, RepeatMode};

use super::control::{next_button, prev_button};
use super::{play, seek_row};
use crate::gui::app::{App, Message};
use crate::gui::assets::Icon;
use crate::gui::style::{col, gap, icon_button, tip};
use crate::palette::Palette;
use crate::state::Event;
use crate::theme::{color, shape};

/// Target size of the shuffle and repeat buttons.
const SMALL: f32 = 36.0;
/// Target size of previous and next.
const MEDIUM: f32 = 40.0;
/// Diameter of the "on" dot under the repeat button.
const DOT: f32 = 4.0;

/// The zone: transport row over the seek row, `width` px wide.
pub(super) fn view(app: &App, accent: Palette, width: f32) -> Element<'_, Message> {
    let ink = col(color::on_surface());
    let transport = row![
        shuffle(app),
        prev_button(ink, MEDIUM),
        play::view(app, accent),
        next_button(ink, MEDIUM),
        repeat(app, accent),
    ]
    .spacing(gap::SM)
    .align_y(Alignment::Center);
    column![transport, seek_row(app, accent)]
        .spacing(gap::XS)
        .align_x(Alignment::Center)
        .width(Length::Fixed(width))
        .into()
}

/// Shuffle what plays next. Disabled with fewer than three songs queued.
fn shuffle(app: &App) -> Element<'_, Message> {
    tip(
        icon_button(
            Icon::Shuffle,
            20.0,
            SMALL,
            col(color::on_surface_variant()),
            Color::TRANSPARENT,
            (app.ui.snapshot.queue.len() > 2).then_some(Message::ShuffleQueue),
        ),
        "Shuffle what plays next",
    )
}

/// What the repeat button looks like and says for a mode: the glyph, whether it is on, and the
/// tooltip.
#[must_use]
pub(super) const fn repeat_look(mode: RepeatMode) -> (Icon, bool, &'static str) {
    match mode {
        RepeatMode::Off => (Icon::Repeat, false, "Repeat off"),
        RepeatMode::All => (Icon::Repeat, true, "Repeat all"),
        RepeatMode::One => (Icon::RepeatOne, true, "Repeat one song"),
    }
}

/// The repeat toggle: dim when off; in the song accent with a dot under it when on, and a "1"
/// in the glyph for repeat-one.
fn repeat(app: &App, accent: Palette) -> Element<'_, Message> {
    let mode = app.ui.snapshot.repeat;
    let (glyph, on, hint) = repeat_look(mode);
    let ink = if on {
        col(accent.primary)
    } else {
        col(color::on_surface_variant())
    };
    let button = icon_button(
        glyph,
        20.0,
        SMALL,
        ink,
        Color::TRANSPARENT,
        Some(Message::Ui(Event::Command(PlayerCommand::SetRepeat(
            mode.cycled(),
        )))),
    );
    let dot: Element<'_, Message> = if on {
        container(Space::new())
            .width(Length::Fixed(DOT))
            .height(Length::Fixed(DOT))
            .style(move |_| container::Style {
                background: Some(col(accent.primary).into()),
                border: iced::Border {
                    radius: shape::FULL.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            })
            .into()
    } else {
        Space::new().into()
    };
    tip(
        stack![
            button,
            container(dot)
                .width(Length::Fixed(SMALL))
                .height(Length::Fixed(SMALL))
                .align_x(Alignment::Center)
                .align_y(Alignment::End)
                .padding(iced::Padding {
                    bottom: 1.0,
                    ..iced::Padding::ZERO
                }),
        ]
        .width(Length::Fixed(SMALL))
        .height(Length::Fixed(SMALL)),
        hint,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeat_says_what_it_is_in_each_mode() {
        assert_eq!(
            repeat_look(RepeatMode::Off),
            (Icon::Repeat, false, "Repeat off")
        );
        assert_eq!(
            repeat_look(RepeatMode::All),
            (Icon::Repeat, true, "Repeat all")
        );
        assert_eq!(
            repeat_look(RepeatMode::One),
            (Icon::RepeatOne, true, "Repeat one song")
        );
    }
}
