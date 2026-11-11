//! Things that float above a screen: the track menu, the first-run notice, the signal-path
//! popover and the notice above the bar. All of them are opaque (`surface-container-high`):
//! the bar is the only glass layer, and a popover opened from glass is never glass itself.

use iced::widget::{Space, button, checkbox, column, container, mouse_area, opaque, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};
use nemo_core::track::Track;

use super::super::app::{App, ArtShape, Message};
use super::super::assets::{Icon, font};
use super::super::style::{
    animated, button_flat, col, fit, gap, icon, icon_button, label, shadow, spinner,
};
use super::kit::{art, paragraph, vspace};
use crate::copy;
use crate::format;
use crate::signal;
use crate::state::Event;
use crate::theme::{color, glass, shape, type_scale};

pub(super) fn card_style(radius: f32) -> impl Fn(&iced::Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(col(color::surface_container_high()))),
        border: Border {
            color: col(color::outline_variant()),
            width: 1.0,
            radius: radius.into(),
        },
        shadow: shadow(glass::SHADOW_OFFSET_Y, glass::SHADOW_BLUR, glass::SHADOW),
        ..container::Style::default()
    }
}

pub(super) fn scrim<'a>(alpha: f32, on_press: Option<Message>) -> Element<'a, Message> {
    let layer = container(Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(col(color::SCRIM.with_alpha(alpha)))),
            ..container::Style::default()
        });
    match on_press {
        Some(m) => mouse_area(layer).on_press(m).into(),
        None => opaque(layer),
    }
}

/// The track menu: a modal card with Play next, Add to queue, Go to album and Go to artist.
pub fn track_menu<'a>(app: &'a App, track: &'a Track) -> Element<'a, Message> {
    let dim = col(color::on_surface_variant());
    let remote_blocked =
        !app.ui.youtube_enabled() && track.origin == nemo_core::audio::Origin::YoutubeMusic;
    let entry = |glyph: Icon, text: &'static str, event: Option<Message>| -> Element<'a, Message> {
        let ink = if event.is_some() {
            col(color::on_surface())
        } else {
            col(color::disabled())
        };
        let b = button(
            row![
                icon(glyph, 20.0, ink),
                Space::new().width(gap::MD),
                label(text, type_scale::BODY_MD, false, ink),
            ]
            .align_y(Alignment::Center)
            .padding(Padding::from([0.0, gap::LG]))
            .height(Length::Fixed(48.0)),
        )
        .padding(0)
        .width(Length::Fill);
        match event {
            Some(e) => animated(b.on_press(e), Color::TRANSPARENT, shape::md()),
            None => b.style(button_flat(Color::TRANSPARENT, shape::md())).into(),
        }
    };
    let mut items = column![
        row![
            art(app, &track.artwork, 40, ArtShape::Row),
            Space::new().width(gap::MD),
            column![
                label(
                    fit(&track.title, 190.0, type_scale::BODY_MD.size),
                    type_scale::BODY_MD,
                    true,
                    col(color::on_surface())
                ),
                label(
                    fit(
                        &format::artists(&track.artists),
                        190.0,
                        type_scale::BODY_SM.size
                    ),
                    type_scale::BODY_SM,
                    false,
                    dim
                ),
            ]
            .width(Length::Fixed(190.0)),
            Space::new().width(Length::Fill),
            icon_button(
                Icon::Close,
                20.0,
                40.0,
                col(color::on_surface()),
                Color::TRANSPARENT,
                Some(Message::Ui(Event::CloseMenu)),
            ),
        ]
        .align_y(Alignment::Center)
        .padding(Padding::from([gap::SM, gap::LG])),
        vspace(gap::XS),
        entry(
            Icon::PlayNext,
            "Play next",
            Some(Message::Ui(Event::PlayNext(track.clone())))
        ),
        entry(
            Icon::Queue,
            "Add to queue",
            Some(Message::Ui(Event::Enqueue(track.clone())))
        ),
        entry(
            Icon::Radio,
            "Start radio",
            (!remote_blocked).then(|| Message::StartRadio(track.clone())),
        ),
    ];
    items = items.push(entry(
        Icon::Album,
        "Go to album",
        (track.album.is_some() && !remote_blocked)
            .then(|| Message::Ui(Event::GoToAlbum(track.clone()))),
    ));
    items = items.push(entry(
        Icon::Mic,
        "Go to artist",
        (!track.artists.is_empty() && !remote_blocked)
            .then(|| Message::Ui(Event::GoToArtist(track.clone()))),
    ));
    items = items.push(entry(
        Icon::Link,
        "Copy link",
        track
            .video_id
            .is_some()
            .then(|| Message::CopyLink(track.clone())),
    ));
    let liked = app.is_liked(track);
    items = items.push(entry(
        if liked { Icon::HeartFill } else { Icon::Heart },
        if liked {
            "Remove from liked songs"
        } else {
            "Like"
        },
        track
            .video_id
            .is_some()
            .then(|| Message::ToggleLike(track.clone())),
    ));
    let card = container(items.padding(Padding::from([gap::SM, gap::SM])))
        .width(Length::Fixed(320.0))
        .style(card_style(shape::lg()));
    iced::widget::stack![
        scrim(0.4, Some(Message::Ui(Event::CloseMenu))),
        container(opaque(card))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    ]
    .into()
}

/// The first-run notice (ADR 0001): a calm full-window sheet, an unticked checkbox and Continue.
pub fn tos_sheet(app: &App) -> Element<'_, Message> {
    let accent = app.accent();
    let checked = app.ui.tos_checked;
    let mut body = column![
        label(
            "Before you start",
            type_scale::HEADLINE_MD,
            true,
            col(color::on_surface())
        ),
        vspace(gap::LG),
    ]
    .width(Length::Fixed(520.0));
    for p in copy::TOS_NOTICE {
        body = body
            .push(paragraph(
                p,
                type_scale::BODY_MD,
                col(color::on_surface()),
                520.0,
            ))
            .push(vspace(gap::MD));
    }
    let check = checkbox(checked)
        .label(copy::TOS_CHECKBOX)
        .on_toggle(|on| Message::Ui(Event::TosCheck(on)))
        .font(font(false))
        .text_size(f32::from(type_scale::BODY_MD.size))
        .size(24.0)
        .spacing(gap::MD)
        .style(move |_, status| {
            let on = matches!(
                status,
                checkbox::Status::Active { is_checked: true }
                    | checkbox::Status::Hovered { is_checked: true }
            );
            checkbox::Style {
                background: Background::Color(if on {
                    col(color::on_surface())
                } else {
                    Color::TRANSPARENT
                }),
                icon_color: col(color::surface()),
                border: Border {
                    color: col(color::on_surface_variant()),
                    width: 2.0,
                    radius: shape::xs().into(),
                },
                text_color: Some(col(color::on_surface())),
            }
        });
    let cont_ink = if checked {
        col(accent.on_primary)
    } else {
        col(color::disabled())
    };
    let cont = button(
        container(label("Continue", type_scale::BODY_MD, true, cont_ink))
            .padding(Padding::from([0.0, gap::XXL]))
            .height(Length::Fixed(48.0))
            .align_y(Alignment::Center),
    )
    .padding(0);
    let cont_idle = if checked {
        col(accent.primary)
    } else {
        col(color::on_surface().with_alpha(0.12))
    };
    let cont: Element<'_, Message> = if checked {
        animated(
            cont.on_press(Message::Ui(Event::TosContinue)),
            cont_idle,
            shape::FULL,
        )
    } else {
        cont.style(button_flat(cont_idle, shape::FULL)).into()
    };
    let later = button(
        container(label(
            "Not now",
            type_scale::BODY_MD,
            true,
            col(color::on_surface()),
        ))
        .padding(Padding::from([0.0, gap::XL]))
        .height(Length::Fixed(48.0))
        .align_y(Alignment::Center),
    )
    .padding(0)
    .on_press(Message::Ui(Event::TosLater));
    let later = animated(later, Color::TRANSPARENT, shape::FULL);
    body = body
        .push(check)
        .push(vspace(gap::XL))
        .push(row![cont, Space::new().width(gap::SM), later].align_y(Alignment::Center))
        .push(vspace(gap::MD))
        .push(label(
            "Your own music plays without this. YouTube Music waits until you continue.",
            type_scale::BODY_SM,
            false,
            col(color::on_surface_variant()),
        ));
    let sheet = container(body)
        .padding(gap::XXL)
        .style(card_style(shape::xl()));
    iced::widget::stack![
        scrim(0.72, None),
        container(opaque(sheet))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    ]
    .into()
}

/// The signal-path popover: Source, Decode, Process, Output, Result, from the player snapshot.
pub fn signal_popover(app: &App) -> Element<'_, Message> {
    let dim = col(color::on_surface_variant());
    let mut c = column![
        label(
            "Signal path",
            type_scale::TITLE_SM,
            true,
            col(color::on_surface())
        ),
        vspace(gap::MD),
    ]
    .width(Length::Fixed(400.0));
    match app.ui.snapshot.signal_path.as_ref() {
        Some(path) => {
            for line in signal::lines(path) {
                c = c
                    .push(label(line.label, type_scale::LABEL_SM, false, dim))
                    .push(paragraph(
                        line.value,
                        type_scale::BODY_MD,
                        col(color::on_surface()),
                        400.0,
                    ))
                    .push(vspace(gap::SM));
            }
        }
        None => {
            c = c.push(label(
                "Nothing is playing. The path appears when a song starts.",
                type_scale::BODY_MD,
                false,
                dim,
            ));
        }
    }
    opaque(container(c).padding(gap::LG).style(card_style(shape::lg())))
}

/// The notice above the bar: calm, no red. A SponsorBlock skip adds an Undo button.
pub fn toast(app: &App) -> Option<Element<'_, Message>> {
    let t = app.ui.toast.as_ref()?;
    let ink = col(color::on_surface());
    let mut r = row![
        icon(Icon::Info, 16.0, col(color::on_surface_variant())),
        Space::new().width(gap::SM),
        label(
            fit(&t.text, 420.0, type_scale::BODY_SM.size),
            type_scale::BODY_SM,
            false,
            ink
        ),
    ]
    .align_y(Alignment::Center);
    if t.undo {
        r = r.push(Space::new().width(gap::SM)).push(animated(
            button(
                container(label("Undo", type_scale::BODY_SM, true, ink))
                    .padding(Padding::from([0.0, gap::MD]))
                    .height(Length::Fixed(40.0))
                    .align_y(Alignment::Center),
            )
            .padding(0)
            .on_press(Message::Ui(Event::UndoSkip)),
            Color::TRANSPARENT,
            shape::FULL,
        ));
    }
    r = r.push(icon_button(
        Icon::Close,
        16.0,
        32.0,
        col(color::on_surface_variant()),
        Color::TRANSPARENT,
        Some(Message::Ui(Event::DismissToast)),
    ));
    Some(opaque(
        container(r.padding(Padding::from([gap::XS, gap::MD]))).style(card_style(shape::FULL)),
    ))
}

/// A small spinner glyph that turns with the app's frame clock (still when motion is reduced).
pub fn spinning<'a>(app: &App, size: f32, ink: Color) -> Element<'a, Message> {
    spinner(size, ink, app.spin).into()
}
