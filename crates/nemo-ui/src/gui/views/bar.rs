//! The docked bottom horizontal player bar and transport controls.

use iced::font::Weight;
use iced::widget::{Space, button, column, container, row, slider, stack};
use iced::{Alignment, Border, Color, Element, Length, Padding};

use super::super::app::{App, Message};
use super::super::assets::Icon;
use super::super::fx::fx;
use super::super::style::{
    animated, col, fit, gap, icon, icon_button, label, label_w, slider_style, tip,
};
use crate::format;
use crate::palette::Palette;
use crate::state::Event;
use crate::theme::{color, shape, type_scale};
use nemo_core::player::PlayerCommand;

mod control;
mod cover;

pub use control::{like_button, play_button, repeat_button, transport};
use control::{next_button, prev_button};
pub use cover::cover_art;

/// Docked bar height.
const HEIGHT: f32 = 76.0;

/// Text for the quality badge, from the real signal path (`AAC 129 kbps · Lossy · Resample`).
/// `Unknown` when there is none; `Buffering` while the player waits for data.
#[must_use]
pub fn badge_text(app: &App) -> String {
    if app.ui.is_buffering() {
        return "Buffering\u{2026}".to_owned();
    }
    crate::signal::badge_label(app.ui.snapshot.signal_path.as_ref())
}

/// The docked bottom player bar, `width` px wide and 76 px high.
pub fn view(app: &App, width: f32) -> Element<'_, Message> {
    let accent = app.accent();
    let compact = width < 800.0;
    let roomy = width >= 1240.0;

    let track_w = if compact {
        180.0
    } else if roomy {
        300.0
    } else {
        240.0
    };
    let right_w = if roomy {
        360.0
    } else if compact {
        150.0
    } else {
        260.0
    };

    let content = row![
        container(track_info(app, track_w))
            .width(Length::Fixed(track_w))
            .align_y(Alignment::Center),
        Space::new().width(gap::MD),
        container(center_controls(app, accent, compact))
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center),
        Space::new().width(gap::MD),
        container(right(app, right_w, roomy, compact))
            .width(Length::Fixed(right_w))
            .align_y(Alignment::Center),
    ]
    .align_y(Alignment::Center)
    .padding(Padding::from([0.0, gap::LG]))
    .height(Length::Fill)
    .width(Length::Fill);

    let top_border = container(Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(|_| container::Style {
            background: Some(col(color::outline_variant()).into()),
            ..container::Style::default()
        });

    let bar_body = container(content)
        .width(Length::Fill)
        .height(Length::Fixed(HEIGHT))
        .style(|_| container::Style {
            background: Some(col(color::surface_container_lowest()).into()),
            ..container::Style::default()
        });

    stack![bar_body, top_border]
        .width(Length::Fixed(width))
        .height(Length::Fixed(HEIGHT))
        .into()
}

fn center_controls(app: &App, accent: Palette, compact: bool) -> Element<'_, Message> {
    let ink = col(color::on_surface());
    let transport_row = row![
        prev_button(ink, 36.0),
        play_button(app, accent, 44.0),
        next_button(ink, 36.0),
    ]
    .spacing(gap::SM)
    .align_y(Alignment::Center);

    if compact {
        return transport_row.into();
    }

    column![transport_row, seek_row(app, accent),]
        .spacing(1.0)
        .align_x(Alignment::Center)
        .width(Length::Fill)
        .into()
}

fn track_info(app: &App, width: f32) -> Element<'_, Message> {
    let Some(track) = &app.ui.snapshot.current else {
        return Space::new().into();
    };
    let text_w = (width - 48.0 - gap::MD - 44.0).max(60.0);
    // The title and artist fade in with a new song.
    let fade = app.title_fade();
    let texts = column![
        label(
            fit(&track.title, text_w, type_scale::BODY_MD.size),
            type_scale::BODY_MD,
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
    let b = button(
        row![
            cover_art(app, 48.0, shape::md()),
            Space::new().width(gap::MD),
            texts
        ]
        .align_y(Alignment::Center),
    )
    .padding(0)
    .on_press(Message::OpenNowPlaying);
    let track_btn = tip(
        animated(b, Color::TRANSPARENT, shape::lg()),
        "Show Now Playing",
    );
    row![
        track_btn,
        Space::new().width(gap::XS),
        like_button(app, track, 20.0, 36.0),
    ]
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
    let dim = col(color::on_surface_variant());
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
            col(color::on_surface().with_alpha(0.20)),
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
            col(color::on_surface_variant()),
        ))
        .padding(Padding::from([4.0, gap::MD])),
    )
    .padding(0)
    .on_press(Message::Ui(Event::ToggleSignalPath));
    fx(b.style(|_, _| button::Style {
        background: None,
        text_color: col(color::on_surface()),
        border: Border {
            color: col(color::outline()),
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
    let ink = col(color::on_surface());
    let dim = col(color::on_surface_variant());
    let volume_w = if compact { 72.0 } else { 88.0 };
    let volume = slider(0..=100_u8, app.ui.snapshot.volume_percent, |v| {
        Message::Ui(Event::Command(PlayerCommand::SetVolume(v)))
    })
    .height(16.0)
    .width(Length::Fixed(volume_w))
    .style(slider_style(
        col(color::on_surface()),
        col(color::on_surface().with_alpha(0.20)),
        col(color::on_surface()),
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
                col(color::on_surface().with_alpha(0.14))
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
    if !compact {
        r = r.push(tip(
            icon_button(
                Icon::Shuffle,
                20.0,
                36.0,
                dim,
                Color::TRANSPARENT,
                (app.ui.snapshot.queue.len() > 2).then_some(Message::ShuffleQueue),
            ),
            "Shuffle what plays next",
        ));
        r = r.push(repeat_button(app, 36.0));
    }
    if roomy {
        r = r.push(badge(app, 180.0));
    }
    r = r.push(queue);
    if !compact {
        let sleeping = app.sleep.is_some();
        r = r.push(tip(
            icon_button(
                if sleeping { Icon::Timer } else { Icon::Tune },
                20.0,
                36.0,
                if sleeping {
                    col(app.accent().primary)
                } else {
                    ink
                },
                if app.tools_open {
                    col(color::on_surface().with_alpha(0.14))
                } else {
                    Color::TRANSPARENT
                },
                Some(Message::Tools(!app.tools_open)),
            ),
            "Sleep timer, speed and more (S)",
        ));
    }
    r = r
        .push(icon(vol_icon, 20.0, dim))
        .push(Space::new().width(gap::XS))
        .push(volume);
    let _ = width;
    r.into()
}
