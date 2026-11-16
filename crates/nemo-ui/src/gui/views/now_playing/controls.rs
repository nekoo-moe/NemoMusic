//! The quiet controls of the full-screen Now Playing (lyrics-polish task 2, owner override
//! 2026-11-15): icon-only previous and next in `on-surface-variant` that light up to
//! `on-surface` under the pointer, one neutral filled play button, a thin progress bar with
//! tabular times, the quality badge as a quiet text label, and the lyric offset tools in a small
//! Timing popover. The bundled font's digits are tabular, so the times do not jitter.

use iced::font::Weight;
use iced::widget::{Space, button, column, container, mouse_area, opaque, row, slider, svg};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use super::super::super::app::{App, Message};
use super::super::super::assets::Icon;
use super::super::super::fx::fx;
use super::super::super::style::{button_flat, col, fit, gap, icon, label, label_w, tip};
use super::super::bar::badge_text;
use super::pill_button;
use crate::anim::play_radius;
use crate::format;
use crate::keys::{KeyAction, LYRICS_OFFSET_STEP_MS};
use crate::state::{Event, LyricsState};
use crate::theme::{color, shape, type_scale};
use nemo_core::lyrics::Granularity;
use nemo_core::player::PlayerCommand;

/// Glyph size of the quiet icon buttons, px (`design.md` section 13 icon sizes).
const GLYPH: f32 = 24.0;
/// Target size of the quiet icon buttons, px (>= 40 for primary controls).
const TARGET: f32 = 40.0;
/// Rail width of the thin progress bar, px.
const RAIL: f32 = 3.0;

/// An icon-only button: `on-surface-variant` at rest, `on-surface` under the pointer, with the
/// usual state layer and focus ring of [`fx`].
fn quiet_icon(glyph: Icon, hint: &str, message: Message) -> Element<'_, Message> {
    let rest = col(color::on_surface_variant());
    let hot = col(color::on_surface());
    let g = svg(glyph.handle())
        .width(Length::Fixed(GLYPH))
        .height(Length::Fixed(GLYPH))
        .style(move |_, status| svg::Style {
            color: Some(match status {
                svg::Status::Hovered => hot,
                svg::Status::Idle => rest,
            }),
        });
    let b = button(
        container(g)
            .width(Length::Fixed(TARGET))
            .height(Length::Fixed(TARGET))
            .center_x(Length::Fixed(TARGET))
            .center_y(Length::Fixed(TARGET)),
    )
    .padding(0)
    .on_press(message);
    tip(
        fx(b.style(button_flat(Color::TRANSPARENT, shape::FULL)))
            .layer(hot)
            .radius(TARGET / 2.0),
        hint,
    )
}

/// The play button in a neutral tone: an `on-surface` disc with a `surface` glyph, morphing
/// to a rounded square while a song plays (the only filled control).
fn neutral_play(app: &App, size: f32) -> Element<'_, Message> {
    let playing = app.ui.is_playing();
    let ink = col(color::surface());
    let glyph: Element<'_, Message> = if app.ui.is_buffering() {
        super::super::super::style::spinner(size / 2.0, ink, app.spin).into()
    } else {
        icon(
            if playing { Icon::Pause } else { Icon::Play },
            size / 2.0 + 2.0,
            ink,
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
        fx(b.style(button_flat(col(color::on_surface()), radius)))
            .layer(ink)
            .radius(radius),
        if playing { "Pause" } else { "Play" },
    )
}

/// Previous, play/pause, next.
pub fn transport(app: &App, play_size: f32) -> Element<'_, Message> {
    row![
        quiet_icon(
            Icon::Prev,
            "Previous",
            Message::Ui(Event::Command(PlayerCommand::Previous))
        ),
        neutral_play(app, play_size),
        quiet_icon(
            Icon::Next,
            "Next",
            Message::Ui(Event::Command(PlayerCommand::Next))
        ),
    ]
    .spacing(gap::LG)
    .align_y(Alignment::Center)
    .into()
}

/// The thin progress bar: a 3 px neutral rail, a small handle that grows under the pointer,
/// and both times in `label-sm`.
pub fn seek(app: &App) -> Element<'_, Message> {
    let duration = app.ui.snapshot.duration;
    let frac = match (app.seek_drag, duration) {
        (Some(f), _) => f,
        (None, Some(d)) if !d.is_zero() => {
            (app.ui.position.as_secs_f32() / d.as_secs_f32()).clamp(0.0, 1.0)
        }
        _ => 0.0,
    };
    let shown = match (app.seek_drag, duration) {
        (Some(f), Some(d)) => d.mul_f32(f.clamp(0.0, 1.0)),
        _ => app.ui.position,
    };
    let dim = col(color::on_surface_variant());
    let fill = col(color::on_surface());
    let rail = col(color::on_surface().with_alpha(0.18));
    let bar = slider(0.0..=1.0, frac, Message::SeekDrag)
        .step(0.001_f32)
        .on_release(Message::SeekRelease)
        .height(12.0)
        .style(move |_, status| slider::Style {
            rail: slider::Rail {
                backgrounds: (Background::Color(fill), Background::Color(rail)),
                width: RAIL,
                border: Border {
                    radius: shape::FULL.into(),
                    ..Border::default()
                },
            },
            handle: slider::Handle {
                shape: slider::HandleShape::Circle {
                    radius: match status {
                        slider::Status::Active => 4.0,
                        slider::Status::Hovered | slider::Status::Dragged => 6.0,
                    },
                },
                background: Background::Color(fill),
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
            },
        });
    let time = |text: String| label(text, type_scale::LABEL_SM, false, dim);
    column![
        bar,
        row![
            time(format::clock(shown)),
            Space::new().width(Length::Fill),
            time(duration.map_or_else(|| "-:--".to_owned(), format::clock)),
        ],
    ]
    .spacing(gap::XS)
    .into()
}

/// The quality badge as a quiet text label; it still opens the signal path.
pub fn quality_label(app: &App, max_w: f32) -> Element<'_, Message> {
    let text = fit(&badge_text(app), max_w, type_scale::LABEL_SM.size);
    let b = button(
        container(label_w(
            text,
            type_scale::LABEL_SM,
            Weight::Medium,
            col(color::on_surface_variant()),
        ))
        .padding(Padding::from([gap::XS, gap::SM])),
    )
    .padding(0)
    .on_press(Message::Ui(Event::ToggleSignalPath));
    fx(b.style(button_flat(Color::TRANSPARENT, shape::FULL)))
        .radius(shape::FULL)
        .into()
}

/// `true` when the lyrics on screen are synced (the offset tools apply).
fn synced(app: &App) -> bool {
    matches!(&app.ui.lyrics, LyricsState::Ready(d) if d.granularity != Granularity::Unsynced)
}

/// The icon that opens the Timing popover; nothing for unsynced lyrics.
pub fn timing_button(app: &App) -> Element<'_, Message> {
    if !synced(app) {
        return Space::new().into();
    }
    quiet_icon(
        Icon::Timer,
        "Lyrics timing",
        Message::Ui(Event::ToggleLyricsTiming),
    )
}

/// The Timing popover (opaque `surface-container-high`, `design.md` section 6): `-50 ms`, the
/// offset, `+50 ms` and `Reset`, with the `[` and `]` keys named. A press outside closes it.
pub fn timing_layer(app: &App, top: f32) -> Option<Element<'_, Message>> {
    if !app.surfaces.timing.visible() || !synced(app) {
        return None;
    }
    let ink = col(color::on_surface());
    let dim = col(color::on_surface_variant());
    let step = |sign: i64| {
        Message::Ui(Event::Key(KeyAction::LyricsOffsetBy(
            sign * LYRICS_OFFSET_STEP_MS,
        )))
    };
    let tools = row![
        pill_button("-50\u{2009}ms", step(-1)),
        container(label(
            format::offset_seconds(app.ui.lyrics_offset_ms),
            type_scale::LABEL_MD,
            false,
            ink,
        ))
        .width(Length::Fixed(64.0))
        .center_x(Length::Fixed(64.0)),
        pill_button("+50\u{2009}ms", step(1)),
    ]
    .spacing(gap::XS)
    .align_y(Alignment::Center);
    let card = column![
        label("Lyrics timing", type_scale::TITLE_SM, true, ink),
        tools,
        row![
            label("Keys [ and ]", type_scale::LABEL_SM, false, dim),
            Space::new().width(Length::Fill),
            pill_button("Reset", Message::Ui(Event::LyricsOffsetReset)),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(gap::MD)
    .width(Length::Fixed(280.0));
    let card = container(card)
        .padding(gap::LG)
        .style(|_| container::Style {
            background: Some(Background::Color(col(color::surface_container_high()))),
            border: Border {
                color: col(color::outline_variant()),
                width: 1.0,
                radius: shape::lg().into(),
            },
            ..container::Style::default()
        });
    let away = mouse_area(
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(Message::Ui(Event::ToggleLyricsTiming));
    let card = super::super::overlays::appear(
        card,
        app.surfaces.timing.value(),
        shape::lg(),
        super::super::overlays::POPUP_RISE,
    );
    let placed = container(opaque(card))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::End)
        .padding(Padding {
            top,
            right: gap::XL,
            ..Padding::ZERO
        });
    Some(iced::widget::stack![away, placed].into())
}
