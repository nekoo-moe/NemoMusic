//! Transport controls of the player capsule: shuffle, previous, play, next, repeat and like.

use iced::widget::{button, container, row};
use iced::{Alignment, Color, Element, Length};

use crate::anim::play_radius;
use crate::gui::app::{App, Message};
use crate::gui::assets::Icon;
use crate::gui::fx::fx;
use crate::gui::style::{button_flat, col, gap, icon, icon_button, spinner, tip};
use crate::keys::KeyAction;
use crate::palette::Palette;
use crate::state::Event;
use crate::theme::color;
use nemo_core::player::{PlayerCommand, RepeatMode};
use nemo_core::track::Track;

/// Shuffle, previous, play, next, repeat.
pub(super) fn controls(app: &App, accent: Palette, compact: bool) -> Element<'_, Message> {
    let ink = col(color::on_surface());
    let dim = col(color::on_surface_variant());
    let mut r = row![].spacing(gap::XS).align_y(Alignment::Center);
    if !compact {
        r = r.push(tip(
            icon_button(
                Icon::Shuffle,
                20.0,
                40.0,
                dim,
                Color::TRANSPARENT,
                (app.ui.snapshot.queue.len() > 2).then_some(Message::ShuffleQueue),
            ),
            "Shuffle what plays next",
        ));
    }
    r = r.push(prev_button(ink, 40.0));
    r = r.push(play_button(app, accent, 48.0));
    r = r.push(next_button(ink, 40.0));
    if !compact {
        r = r.push(repeat_button(app, 40.0));
    }
    r.into()
}

/// The repeat toggle: Off, All, One. When on it is a tonal circle in the cover's colours and
/// the glyph says which mode (a "1" for One).
pub fn repeat_button(app: &App, target: f32) -> Element<'_, Message> {
    let accent = app.accent();
    let mode = app.ui.snapshot.repeat;
    let (glyph, ink, bg, hint) = match mode {
        RepeatMode::Off => (
            Icon::Repeat,
            col(color::on_surface_variant()),
            Color::TRANSPARENT,
            "Repeat off",
        ),
        RepeatMode::All => (
            Icon::Repeat,
            col(accent.on_secondary_container),
            col(accent.secondary_container),
            "Repeat all",
        ),
        RepeatMode::One => (
            Icon::RepeatOne,
            col(accent.on_secondary_container),
            col(accent.secondary_container),
            "Repeat one song",
        ),
    };
    tip(
        icon_button(
            glyph,
            20.0,
            target,
            ink,
            bg,
            Some(Message::Ui(Event::Command(PlayerCommand::SetRepeat(
                mode.cycled(),
            )))),
        ),
        hint,
    )
}

/// The like heart of a song: filled in the accent when liked. Signed out it opens the sign-in
/// sheet; a local file cannot be liked, so there it is drawn disabled.
pub fn like_button<'a>(
    app: &App,
    track: &Track,
    glyph_size: f32,
    target: f32,
) -> Element<'a, Message> {
    let dim = col(color::on_surface_variant());
    if track.video_id.is_none() {
        return tip(
            icon_button(
                Icon::Heart,
                glyph_size,
                target,
                dim,
                Color::TRANSPARENT,
                None,
            ),
            "Only YouTube Music songs can be liked",
        );
    }
    let message = Some(Message::ToggleLike(track.clone()));
    if !app.signed_in() {
        return tip(
            icon_button(
                Icon::Heart,
                glyph_size,
                target,
                dim,
                Color::TRANSPARENT,
                message,
            ),
            "Sign in to like songs",
        );
    }
    if app.is_liked(track) {
        tip(
            icon_button(
                Icon::HeartFill,
                glyph_size,
                target,
                col(app.accent().primary),
                Color::TRANSPARENT,
                message,
            ),
            "Remove from liked songs",
        )
    } else {
        tip(
            icon_button(
                Icon::Heart,
                glyph_size,
                target,
                col(color::on_surface()),
                Color::TRANSPARENT,
                message,
            ),
            "Like this song",
        )
    }
}

fn prev_button<'a>(ink: Color, target: f32) -> Element<'a, Message> {
    tip(
        icon_button(
            Icon::Prev,
            24.0,
            target,
            ink,
            Color::TRANSPARENT,
            Some(Message::Ui(Event::Command(PlayerCommand::Previous))),
        ),
        "Previous",
    )
}

fn next_button<'a>(ink: Color, target: f32) -> Element<'a, Message> {
    tip(
        icon_button(
            Icon::Next,
            24.0,
            target,
            ink,
            Color::TRANSPARENT,
            Some(Message::Ui(Event::Command(PlayerCommand::Next))),
        ),
        "Next",
    )
}

/// The play button: a circle that morphs into a rounded square while a song plays. While a
/// stream starts the glyph becomes a calm ring.
pub fn play_button(app: &App, accent: Palette, size: f32) -> Element<'_, Message> {
    let playing = app.ui.is_playing();
    let glyph: Element<'_, Message> = if app.ui.is_buffering() {
        spinner(size / 2.0 + 2.0, col(accent.on_primary), app.spin).into()
    } else {
        icon(
            if playing { Icon::Pause } else { Icon::Play },
            size / 2.0 + 4.0,
            col(accent.on_primary),
        )
        .into()
    };
    let radius = play_radius(size, app.anim.play.value);
    let b = button(
        container(glyph)
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .center_x(Length::Fixed(size))
            .center_y(Length::Fixed(size)),
    )
    .padding(0)
    .on_press(Message::Ui(Event::Key(KeyAction::TogglePlay)));
    tip(
        fx(b.style(button_flat(col(accent.primary), radius)))
            .layer(col(accent.on_primary))
            .radius(radius),
        if playing { "Pause" } else { "Play" },
    )
}

/// Previous, play/pause, next, for the full-window Now Playing.
pub fn transport(app: &App, accent: Palette, play_size: f32) -> Element<'_, Message> {
    let ink = col(color::on_surface());
    row![
        prev_button(ink, 48.0),
        play_button(app, accent, play_size),
        next_button(ink, 48.0),
    ]
    .spacing(gap::MD)
    .align_y(Alignment::Center)
    .into()
}
