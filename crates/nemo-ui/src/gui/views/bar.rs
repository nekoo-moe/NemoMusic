//! The floating player capsule (Tier Low glass, `design.md` sections 6 and 7) and the transport
//! controls Now Playing shares.
//!
//! Tier Low: the cover's colours behind a tint, an edge highlight, a shadow, radius 36. No live
//! backdrop sampling. If the tint cannot reach 4.5:1 even at 0.90 alpha, the capsule falls back
//! to an opaque `surface-container-highest`. The play button is a circle that morphs into a
//! rounded square while a song plays (`anim::play_radius`).

use iced::font::Weight;
use iced::widget::{Space, button, column, container, image, row, slider, stack};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use super::super::app::{App, CoverView, Message};
use super::super::assets::Icon;
use super::super::fx::fx;
use super::super::style::{
    animated, button_flat, col, fit, gap, glass_tint, icon, icon_button, label, label_w, shadow,
    slider_style, spinner, tip,
};
use super::kit::art_placeholder;
use crate::anim::play_radius;
use crate::format;
use crate::keys::KeyAction;
use crate::palette::Palette;
use crate::state::Event;
use crate::theme::{color, glass, shape, type_scale};
use nemo_core::player::PlayerCommand;

/// Capsule height.
const HEIGHT: f32 = 72.0;
/// Capsule corner radius: half the height.
const RADIUS: f32 = HEIGHT / 2.0;

/// Text for the quality badge, from the real signal path (`AAC 129 kbps · Lossy · Resample`).
/// `Unknown` when there is none; `Buffering` while the player waits for data.
#[must_use]
pub fn badge_text(app: &App) -> String {
    if app.ui.is_buffering() {
        return "Buffering\u{2026}".to_owned();
    }
    crate::signal::badge_label(app.ui.snapshot.signal_path.as_ref())
}

/// The capsule, `width` px wide and 72 px high.
pub fn view(app: &App, width: f32) -> Element<'_, Message> {
    let accent = app.accent();
    let compact = width < 800.0;
    let roomy = width >= 1240.0;
    // Fixed parts: controls, track, right group; the progress takes the rest.
    let controls_w = if compact {
        3.0 * 48.0
    } else {
        5.0 * 44.0 + 12.0
    };
    let track_w = if compact { 168.0 } else { 232.0 };
    let right_w = if roomy {
        400.0
    } else if compact {
        150.0
    } else {
        280.0
    };

    let content = row![
        container(controls(app, accent, compact)).width(Length::Fixed(controls_w)),
        Space::new().width(gap::MD),
        container(track_info(app, track_w)).width(Length::Fixed(track_w)),
        Space::new().width(gap::MD),
        container(seek_row(app, accent)).width(Length::Fill),
        Space::new().width(gap::MD),
        container(right(app, right_w, roomy, compact)).width(Length::Fixed(right_w)),
    ]
    .align_y(Alignment::Center)
    .padding(Padding::from([0.0, gap::LG]))
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
                    radius: RADIUS.into(),
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
                            radius: RADIUS.into(),
                            ..Border::default()
                        },
                        ..container::Style::default()
                    })
                    .into(),
            );
            layers.push(tint_layer(glass_tint(alpha), RADIUS));
        }
        // Tonal fallback: opaque, no glass.
        _ => layers.push(tint_layer(col(color::SURFACE_CONTAINER_HIGHEST), RADIUS)),
    }
    layers.push(content.into());
    layers.push(edge_layer(RADIUS, glass_ok.is_some()));
    container(stack(layers))
        .width(Length::Fixed(width))
        .height(Length::Fixed(HEIGHT))
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

/// One cover handle for a slot: the 128 px bar bake up to 96 px, the 480 px bake above.
fn cover_handle(c: &CoverView, size: f32) -> image::Handle {
    if size <= 96.0 {
        c.art_bar.clone()
    } else {
        c.art.clone()
    }
}

/// Current cover at `size` px with radius `radius`, or the placeholder. When the track changed
/// the old cover stays under the new one while it fades in (400 ms).
pub fn cover_art<'a>(app: &App, size: f32, radius: f32) -> Element<'a, Message> {
    let Some(c) = &app.cover else {
        return art_placeholder(size, radius, false);
    };
    let fade = app.anim.cover();
    let draw = |handle: image::Handle, opacity: f32| {
        image(handle)
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .content_fit(iced::ContentFit::Fill)
            .opacity(opacity)
    };
    match &app.prev_cover {
        Some(prev) if fade < 0.999 => stack![
            draw(cover_handle(prev, size), 1.0),
            draw(cover_handle(c, size), fade)
        ]
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .into(),
        _ => draw(cover_handle(c, size), 1.0).into(),
    }
}

fn track_info(app: &App, width: f32) -> Element<'_, Message> {
    let Some(track) = &app.ui.snapshot.current else {
        return Space::new().into();
    };
    let text_w = width - 48.0 - gap::MD;
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
    let b = button(
        row![
            cover_art(app, 48.0, shape::MD),
            Space::new().width(gap::MD),
            texts
        ]
        .align_y(Alignment::Center),
    )
    .padding(0)
    .on_press(Message::OpenNowPlaying);
    tip(
        animated(b, Color::TRANSPARENT, shape::LG),
        "Show Now Playing",
    )
}

/// Shuffle, previous, play, next, repeat.
fn controls(app: &App, accent: Palette, compact: bool) -> Element<'_, Message> {
    let ink = col(color::ON_SURFACE);
    let dim = col(color::ON_SURFACE_VARIANT);
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
        r = r.push(tip(
            icon_button(Icon::Repeat, 20.0, 40.0, dim, Color::TRANSPARENT, None),
            "Repeat is not available yet",
        ));
    }
    r.into()
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
    let ink = col(color::ON_SURFACE);
    row![
        prev_button(ink, 48.0),
        play_button(app, accent, play_size),
        next_button(ink, 48.0),
    ]
    .spacing(gap::MD)
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
        container(label(text, type_scale::LABEL_MD, false, dim))
            .width(Length::Fixed(40.0))
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
            col(color::ON_SURFACE.with_alpha(0.20)),
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
    let b = button(
        container(label_w(
            text,
            type_scale::LABEL_SM,
            Weight::Medium,
            col(color::ON_SURFACE_VARIANT),
        ))
        .padding(Padding::from([4.0, gap::MD])),
    )
    .padding(0)
    .on_press(Message::Ui(Event::ToggleSignalPath));
    fx(b.style(|_, _| button::Style {
        background: None,
        text_color: col(color::ON_SURFACE),
        border: Border {
            color: col(color::OUTLINE),
            width: 1.0,
            radius: shape::FULL.into(),
        },
        shadow: iced::Shadow::default(),
        snap: false,
    }))
    .radius(shape::FULL)
    .into()
}

fn right(app: &App, width: f32, roomy: bool, compact: bool) -> Element<'_, Message> {
    let ink = col(color::ON_SURFACE);
    let dim = col(color::ON_SURFACE_VARIANT);
    let volume_w = if compact { 72.0 } else { 88.0 };
    let volume = slider(0..=100_u8, app.ui.snapshot.volume_percent, |v| {
        Message::Ui(Event::Command(PlayerCommand::SetVolume(v)))
    })
    .height(16.0)
    .width(Length::Fixed(volume_w))
    .style(slider_style(
        col(color::ON_SURFACE),
        col(color::ON_SURFACE.with_alpha(0.20)),
        col(color::ON_SURFACE),
        5.0,
    ));
    let queue_on = app.pane_open && app.np_tab == super::super::app::NpTab::Queue;
    let queue = tip(
        icon_button(
            Icon::Queue,
            22.0,
            40.0,
            ink,
            if queue_on {
                col(color::ON_SURFACE.with_alpha(0.14))
            } else {
                Color::TRANSPARENT
            },
            Some(Message::Ui(Event::ToggleQueue)),
        ),
        "Queue",
    );
    let vol_icon = if app.ui.snapshot.volume_percent == 0 {
        Icon::VolumeOff
    } else {
        Icon::Volume
    };
    let mut r = row![Space::new().width(Length::Fill)]
        .spacing(gap::XS)
        .align_y(Alignment::Center);
    if roomy {
        r = r.push(badge(app, 190.0));
    }
    if !compact {
        r = r.push(tip(
            icon_button(Icon::Heart, 22.0, 40.0, dim, Color::TRANSPARENT, None),
            "Likes are not available yet",
        ));
    }
    r = r
        .push(queue)
        .push(icon(vol_icon, 22.0, dim))
        .push(Space::new().width(gap::XS))
        .push(volume);
    let _ = width;
    r.into()
}
