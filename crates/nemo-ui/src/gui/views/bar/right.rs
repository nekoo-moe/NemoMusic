//! Right zone of the capsule: quality pill, lyrics, queue, tools, the speaker with its slider,
//! and full-screen. It folds in steps as the capsule narrows (`layout::bar::Step`).

use iced::widget::{Space, button, container, row, slider};
use iced::{Alignment, Color, Element, Length, Padding};
use nemo_core::player::PlayerCommand;

use super::badge_text;
use crate::gui::app::{App, Message, NpTab};
use crate::gui::assets::Icon;
use crate::gui::fx::fx;
use crate::gui::style::{button_flat, col, fit, gap, icon_button, label_w, slider_style, tip};
use crate::layout::bar::{Step, VOLUME_SLIDER, pill_max};
use crate::palette::Palette;
use crate::state::Event;
use crate::theme::{color, shape, type_scale};

/// Glyph size of the icon buttons.
const GLYPH: f32 = 20.0;
/// Target size of the icon buttons (`layout::bar::BUTTON`).
const TARGET: f32 = 36.0;
/// Pill padding, left and right.
const PILL_PAD: f32 = gap::MD;

/// Which of the right zone's parts show at `step`.
#[must_use]
pub(super) const fn parts(step: Step) -> (bool, bool, bool) {
    // (lyrics and tools buttons, volume slider, the single `More` button)
    (
        !matches!(step, Step::Folded),
        matches!(step, Step::Full),
        matches!(step, Step::Folded),
    )
}

/// The zone at `step`.
pub(super) fn view(app: &App, step: Step) -> Element<'_, Message> {
    let accent = app.accent();
    let (extras, slider_on, more) = parts(step);
    let mut r = row![Space::new().width(Length::Fill)]
        .spacing(gap::XS)
        .align_y(Alignment::Center)
        .push(pill(app, step));
    if extras {
        r = r.push(lyrics(app, accent));
    }
    r = r.push(queue(app, accent));
    if extras {
        r = r.push(tools(
            app,
            accent,
            Icon::Tune,
            "Sleep timer, speed and more (S)",
        ));
    }
    r = r.push(speaker(app));
    if slider_on {
        r = r.push(volume(app, accent));
    }
    if extras {
        r = r.push(tip(
            icon_button(
                Icon::FullScreen,
                GLYPH,
                TARGET,
                col(color::on_surface_variant()),
                Color::TRANSPARENT,
                Some(Message::ToggleFullScreen),
            ),
            "Full-screen Now Playing",
        ));
    }
    if more {
        r = r.push(tools(app, accent, Icon::More, "More controls"));
    }
    r.into()
}

/// The quality pill: the real format, quality word and processing, never a guess. Pressing it
/// opens the signal-path popover; lossy never gets a success colour.
fn pill(app: &App, step: Step) -> Element<'_, Message> {
    let text = fit(
        &badge_text(app),
        super::px(pill_max(step)) - 2.0 * PILL_PAD,
        type_scale::LABEL_SM.size,
    );
    let b = button(
        container(label_w(
            text,
            type_scale::LABEL_SM,
            iced::font::Weight::Medium,
            col(color::on_surface_variant()),
        ))
        .padding(Padding::from([gap::XS + 2.0, PILL_PAD])),
    )
    .padding(0)
    .on_press(Message::Ui(Event::ToggleSignalPath));
    tip(
        fx(b.style(button_flat(
            col(color::on_surface().with_alpha(0.08)),
            shape::FULL,
        )))
        .layer(col(color::on_surface()))
        .radius(shape::FULL),
        "Show signal path",
    )
}

/// A toggle button: in the song accent on a soft accent disc when on.
fn toggle<'a>(
    glyph: Icon,
    on: bool,
    accent: Palette,
    message: Message,
    hint: &'static str,
) -> Element<'a, Message> {
    let (ink, bg) = if on {
        (col(accent.primary), col(accent.primary.with_alpha(0.16)))
    } else {
        (col(color::on_surface_variant()), Color::TRANSPARENT)
    };
    tip(
        icon_button(glyph, GLYPH, TARGET, ink, bg, Some(message)),
        hint,
    )
}

/// Lyrics: the same action as pressing the cover.
fn lyrics(app: &App, accent: Palette) -> Element<'_, Message> {
    let on = app.pane_open && app.np_tab == NpTab::Lyrics;
    toggle(Icon::Lyrics, on, accent, Message::OpenNowPlaying, "Lyrics")
}

/// Queue: the queue tab of the right pane.
fn queue(app: &App, accent: Palette) -> Element<'_, Message> {
    let on = app.pane_open && app.np_tab == NpTab::Queue;
    toggle(
        Icon::Queue,
        on,
        accent,
        Message::Ui(Event::ToggleQueue),
        "Queue",
    )
}

/// Tools (sleep timer, speed, window modes): the sleep timer icon shows while one runs.
fn tools<'a>(
    app: &'a App,
    accent: Palette,
    glyph: Icon,
    hint: &'static str,
) -> Element<'a, Message> {
    let sleeping = app.sleep.is_some();
    toggle(
        if sleeping { Icon::Timer } else { glyph },
        app.tools_open || sleeping,
        accent,
        Message::Tools(!app.tools_open),
        hint,
    )
}

/// The speaker: shows the volume state and mutes or restores on a press.
fn speaker(app: &App) -> Element<'_, Message> {
    let muted = app.ui.snapshot.volume_percent == 0;
    tip(
        icon_button(
            if muted { Icon::VolumeOff } else { Icon::Volume },
            GLYPH,
            TARGET,
            col(color::on_surface_variant()),
            Color::TRANSPARENT,
            Some(Message::ToggleMute),
        ),
        if muted { "Unmute" } else { "Mute" },
    )
}

/// The inline volume slider.
fn volume(app: &App, accent: Palette) -> Element<'_, Message> {
    let w = super::px(VOLUME_SLIDER);
    slider(0..=100_u8, app.ui.snapshot.volume_percent, |v| {
        Message::Ui(Event::Command(PlayerCommand::SetVolume(v)))
    })
    .height(16.0)
    .width(Length::Fixed(w))
    .style(slider_style(
        col(accent.primary),
        col(color::on_surface().with_alpha(0.20)),
        col(accent.primary),
        6.0,
    ))
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_step_shows_less_than_the_one_before() {
        // (extras, slider, more)
        assert_eq!(parts(Step::Full), (true, true, false));
        assert_eq!(parts(Step::NoSlider), (true, false, false));
        assert_eq!(parts(Step::Folded), (false, false, true));
    }
}
