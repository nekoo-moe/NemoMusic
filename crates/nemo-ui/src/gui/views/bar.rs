//! The floating player bar (Tier Low glass, `design.md` sections 6 and 7) and the transport
//! controls Now Playing shares.
//!
//! Tier Low: a pre-blurred cover texture, tint, edge highlight, shadow, radius 28. No live
//! backdrop sampling. If the tint cannot reach 4.5:1 even at 0.90 alpha, the bar falls back to an
//! opaque `surface-container-high`.

use iced::widget::{Space, button, column, container, image, row, slider, stack};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use super::super::app::{App, Message};
use super::super::assets::Icon;
use super::super::style::{
    button_style, col, fit, gap, glass_tint, icon, icon_button, label, shadow, slider_style,
    spinner,
};
use super::kit::art_placeholder;
use crate::format;
use crate::keys::KeyAction;
use crate::palette::Palette;
use crate::route::Route;
use crate::state::Event;
use crate::theme::{color, glass, layout as theme_layout, shape, type_scale};
use nemo_core::player::PlayerCommand;

/// Text for the quality badge, from the real signal path (`AAC 129 kbps · Lossy · Resample`).
/// `Unknown` when there is none; `Buffering` while the player waits for data.
#[must_use]
pub fn badge_text(app: &App) -> String {
    if app.ui.is_buffering() {
        return "Buffering\u{2026}".to_owned();
    }
    crate::signal::badge_label(app.ui.snapshot.signal_path.as_ref())
}

/// The bar, `bar_w` px wide and 88 px high.
pub fn view(app: &App, bar_w: u32) -> Element<'_, Message> {
    let accent = app.accent();
    let radius = glass::RADIUS;
    // The bar width is a small positive number.
    #[allow(clippy::cast_precision_loss)]
    let width = bar_w as f32;
    let side = if width < 900.0 {
        240.0
    } else if width < 1100.0 {
        280.0
    } else {
        320.0
    };

    let content = row![
        container(left(app, side)).width(Length::Fixed(side)),
        container(center(app, accent)).width(Length::Fill),
        container(right(app, side)).width(Length::Fixed(side)),
    ]
    .align_y(Alignment::Center)
    .padding(gap::MD)
    .height(Length::Fill);

    let glass_ok = if app.ui.settings.opaque_chrome() {
        None
    } else {
        app.cover.as_ref().and_then(|c| c.tint_alpha)
    };
    let mut layers: Vec<Element<'_, Message>> = vec![
        // Shadow `0 8 24 #00000052`.
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                shadow: shadow(glass::SHADOW_OFFSET_Y, glass::SHADOW_BLUR, glass::SHADOW),
                border: Border {
                    radius: radius.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .into(),
    ];
    match (app.cover.as_ref(), glass_ok) {
        (Some(cover), Some(alpha)) => {
            // Tier Low: the blurred cover as three colour samples, painted as a gradient (the
            // tiny-skia renderer cannot clip an image to rounded corners; quads it can).
            let [l, m, r] = cover.strip.map(col);
            let grad = iced::gradient::Linear::new(iced::Radians(std::f32::consts::FRAC_PI_2))
                .add_stop(0.0, l)
                .add_stop(0.5, m)
                .add_stop(1.0, r);
            layers.push(
                container(Space::new())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(move |_| container::Style {
                        background: Some(Background::Gradient(grad.into())),
                        border: Border {
                            radius: radius.into(),
                            ..Border::default()
                        },
                        ..container::Style::default()
                    })
                    .into(),
            );
            layers.push(tint_layer(glass_tint(alpha), radius));
        }
        // Tonal fallback: opaque, no glass.
        _ => layers.push(tint_layer(col(color::SURFACE_CONTAINER_HIGH), radius)),
    }
    layers.push(content.into());
    layers.push(edge_layer(radius, glass_ok.is_some()));
    container(stack(layers))
        .width(Length::Fixed(width))
        .height(Length::Fixed(f32::from(
            u16::try_from(crate::layout::PLAYER_BAR_HEIGHT).unwrap_or(88),
        )))
        .into()
}

fn tint_layer<'a>(tint: Color, radius: f32) -> Element<'a, Message> {
    container(Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(tint)),
            border: Border {
                radius: radius.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

/// Inner highlight (1 px, 0.22; top edge 0.32) when glass, plain `outline-variant` otherwise.
fn edge_layer<'a>(radius: f32, glass_on: bool) -> Element<'a, Message> {
    let edge = if glass_on {
        glass::EDGE.with_alpha(glass::EDGE_ALPHA)
    } else {
        color::OUTLINE_VARIANT
    };
    let ring = container(Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            border: Border {
                color: col(edge),
                width: 1.0,
                radius: radius.into(),
            },
            ..container::Style::default()
        });
    if !glass_on {
        return ring.into();
    }
    let top = row![
        Space::new().width(radius),
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fixed(1.0))
            .style(|_| container::Style {
                background: Some(col(glass::EDGE.with_alpha(glass::EDGE_ALPHA_TOP)).into()),
                ..container::Style::default()
            }),
        Space::new().width(radius),
    ];
    stack![ring, top].into()
}

/// Current cover at `size` px with radius `radius`, or the placeholder.
pub fn cover_art<'a>(app: &App, size: f32, radius: f32) -> Element<'a, Message> {
    match &app.cover {
        Some(c) => image(if size <= 96.0 {
            c.art_bar.clone()
        } else {
            c.art.clone()
        })
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .content_fit(iced::ContentFit::Fill)
        .into(),
        None => art_placeholder(size, radius, false),
    }
}

fn left(app: &App, side: f32) -> Element<'_, Message> {
    let Some(track) = &app.ui.snapshot.current else {
        return Space::new().into();
    };
    let text_w = side - 64.0 - gap::MD;
    let texts = column![
        label(
            fit(&track.title, text_w, type_scale::BODY_MD.size),
            type_scale::BODY_MD,
            true,
            col(color::ON_SURFACE)
        ),
        label(
            fit(
                &format::artists(&track.artists),
                text_w,
                type_scale::BODY_SM.size
            ),
            type_scale::BODY_SM,
            false,
            col(color::ON_SURFACE_VARIANT)
        ),
    ];
    button(
        row![
            cover_art(app, 64.0, shape::MD),
            Space::new().width(gap::MD),
            texts
        ]
        .align_y(Alignment::Center),
    )
    .padding(0)
    .style(button_style(Color::TRANSPARENT, shape::MD))
    .on_press(Message::Ui(Event::Go(Route::NowPlaying)))
    .into()
}

fn center(app: &App, accent: Palette) -> Element<'_, Message> {
    column![
        container(transport(app, accent, 40.0)).center_x(Length::Fill),
        seek_row(app, accent),
    ]
    .spacing(gap::XS)
    .width(Length::Fill)
    .into()
}

/// Previous, play/pause, next. The play button is the one place the accent fills a control.
pub fn transport(app: &App, accent: Palette, play_size: f32) -> Element<'_, Message> {
    let ink = col(color::ON_SURFACE);
    let playing = app.ui.is_playing();
    // While a stream starts (a YouTube track takes seconds) the glyph becomes a calm ring.
    let glyph: Element<'_, Message> = if app.ui.is_buffering() {
        spinner(play_size / 2.0 + 4.0, col(accent.on_primary), app.spin).into()
    } else {
        icon(
            if playing { Icon::Pause } else { Icon::Play },
            play_size / 2.0,
            col(accent.on_primary),
        )
        .into()
    };
    let play = button(
        container(glyph)
            .width(Length::Fixed(play_size))
            .height(Length::Fixed(play_size))
            .center_x(Length::Fixed(play_size))
            .center_y(Length::Fixed(play_size)),
    )
    .padding(0)
    .style(button_style(col(accent.primary), shape::FULL))
    .on_press(Message::Ui(Event::Key(KeyAction::TogglePlay)));
    row![
        icon_button(
            Icon::Prev,
            20.0,
            40.0,
            ink,
            Color::TRANSPARENT,
            Some(Message::Ui(Event::Command(PlayerCommand::Previous))),
        ),
        play,
        icon_button(
            Icon::Next,
            20.0,
            40.0,
            ink,
            Color::TRANSPARENT,
            Some(Message::Ui(Event::Command(PlayerCommand::Next))),
        ),
    ]
    .spacing(gap::SM)
    .align_y(Alignment::Center)
    .into()
}

/// `0:42 ---o--- 3:12`. Times use tabular figures and a fixed width, so nothing shifts as they
/// tick.
pub fn seek_row(app: &App, accent: Palette) -> Element<'_, Message> {
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
    let dim = col(color::ON_SURFACE_VARIANT);
    let time = |text: String, right: bool| {
        container(label(text, type_scale::LABEL_SM, false, dim))
            .width(Length::Fixed(44.0))
            .align_x(if right {
                Alignment::Start
            } else {
                Alignment::End
            })
    };
    let bar = slider(0.0..=1.0, frac, Message::SeekDrag)
        .step(0.001_f32)
        .on_release(Message::SeekRelease)
        .height(16.0)
        .style(slider_style(
            col(accent.primary),
            col(color::ON_SURFACE.with_alpha(0.24)),
            col(accent.primary),
            6.0,
        ));
    row![
        time(format::clock(shown), false),
        Space::new().width(gap::SM),
        bar,
        Space::new().width(gap::SM),
        time(
            duration.map_or_else(|| "-:--".to_owned(), format::clock),
            true
        ),
    ]
    .align_y(Alignment::Center)
    .into()
}

/// Quality badge pill: the real format, quality word and processing, never a guess. Pressing it
/// opens the signal-path popover; lossy never gets a success colour.
pub fn badge(app: &App, max_w: f32) -> Element<'_, Message> {
    let text = fit(&badge_text(app), max_w, type_scale::LABEL_SM.size);
    button(
        container(label(
            text,
            type_scale::LABEL_SM,
            false,
            col(color::ON_SURFACE_VARIANT),
        ))
        .padding(Padding::from([2.0, gap::SM])),
    )
    .padding(0)
    .style(|_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: hovered.then(|| col(color::ON_SURFACE.with_alpha(0.08)).into()),
            text_color: col(color::ON_SURFACE),
            border: Border {
                color: col(color::OUTLINE_VARIANT),
                width: 1.0,
                radius: shape::FULL.into(),
            },
            shadow: iced::Shadow::default(),
            snap: false,
        }
    })
    .on_press(Message::Ui(Event::ToggleSignalPath))
    .into()
}
fn right(app: &App, side: f32) -> Element<'_, Message> {
    let ink = col(color::ON_SURFACE);
    let volume = slider(0..=100_u8, app.ui.snapshot.volume_percent, |v| {
        Message::Ui(Event::Command(PlayerCommand::SetVolume(v)))
    })
    .height(16.0)
    .width(Length::Fixed(
        side - 40.0 - 20.0 - 2.0 * gap::SM - 2.0 * gap::MD - 40.0,
    ))
    .style(slider_style(
        col(color::ON_SURFACE),
        col(color::ON_SURFACE.with_alpha(0.24)),
        col(color::ON_SURFACE),
        5.0,
    ));
    let queue = icon_button(
        Icon::Queue,
        20.0,
        40.0,
        ink,
        if app.ui.queue_open {
            col(color::ON_SURFACE.with_alpha(0.12))
        } else {
            Color::TRANSPARENT
        },
        Some(Message::Ui(Event::ToggleQueue)),
    );
    column![
        container(badge(app, side - 2.0 * gap::MD))
            .align_x(Alignment::End)
            .width(Length::Fill),
        row![
            icon(Icon::Volume, 20.0, col(color::ON_SURFACE_VARIANT)),
            Space::new().width(gap::SM),
            volume,
            Space::new().width(Length::Fill),
            queue
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(gap::XS)
    .width(Length::Fill)
    .into()
}

/// Layout constant reference so the compiler checks the bar height token is the one used.
pub const BAR_HEIGHT: f32 = theme_layout::PLAYER_BAR_HEIGHT;
