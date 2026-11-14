//! Panels that float above the window: the tools panel (sleep timer, playback speed, window
//! modes, and the lyrics source), the keyboard shortcut sheet, the "Create playlist" sheet and
//! the mini player. All are opaque (`surface-container-high`); the capsule stays the only glass
//! layer.

use iced::font::Weight;
use iced::widget::{Space, button, column, container, opaque, row, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use super::super::app::{App, CreateMsg, MINI_SIZE, Message};
use super::super::assets::{Icon, font};
use super::super::fx::fx;
use super::super::style::{button_flat, col, fit, gap, icon_button, label, label_w, tip};
use super::bar;
use super::kit::{text_button, tonal_button, vspace};
use super::overlays::{card_style, scrim};
use crate::explore::SHORTCUTS;
use crate::theme::{color, shape, type_scale};

mod tools;

pub use tools::tools_panel;

/// The keyboard shortcut sheet.
pub fn shortcuts_sheet(app: &App) -> Element<'_, Message> {
    let _ = app;
    let dim = col(color::on_surface_variant());
    let mut list = column![].spacing(gap::XS);
    for (keys, what) in SHORTCUTS {
        list = list.push(
            row![
                container(label_w(
                    keys,
                    type_scale::LABEL_MD,
                    Weight::Medium,
                    col(color::on_surface())
                ))
                .padding(Padding::from([gap::XS, gap::SM]))
                .width(Length::Fixed(132.0))
                .style(|_| container::Style {
                    background: Some(col(color::surface_container_highest()).into()),
                    border: Border {
                        radius: shape::sm().into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                }),
                Space::new().width(gap::MD),
                label(what, type_scale::BODY_MD, false, col(color::on_surface())),
            ]
            .align_y(Alignment::Center)
            .height(Length::Fixed(32.0)),
        );
    }
    let body = column![
        row![
            label(
                "Keyboard shortcuts",
                type_scale::TITLE_MD,
                true,
                col(color::on_surface())
            ),
            Space::new().width(Length::Fill),
            icon_button(
                Icon::Close,
                20.0,
                40.0,
                dim,
                Color::TRANSPARENT,
                Some(Message::Shortcuts(false)),
            ),
        ]
        .align_y(Alignment::Center),
        vspace(gap::MD),
        list,
        vspace(gap::MD),
        label(
            "Single letters are off while you type in a text field.",
            type_scale::BODY_SM,
            false,
            dim
        ),
    ]
    .width(Length::Fixed(520.0));
    iced::widget::stack![
        scrim(0.5, Some(Message::Shortcuts(false))),
        container(opaque(
            container(body)
                .padding(gap::XL)
                .style(card_style(shape::xl()))
        ))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill),
    ]
    .into()
}

/// The "Create playlist" sheet: a name field and Create.
pub fn create_sheet(app: &App) -> Element<'_, Message> {
    let accent = app.accent();
    let dim = col(color::on_surface_variant());
    let name = text_input("Playlist name", &app.create_name)
        .on_input(|s| Message::Create(CreateMsg::Name(s)))
        .on_submit(Message::Create(CreateMsg::Confirm))
        .font(font(false))
        .size(f32::from(type_scale::BODY_MD.size))
        .padding(Padding::from([gap::MD, gap::LG]))
        .style(move |_, status| {
            let focused = matches!(status, text_input::Status::Focused { .. });
            text_input::Style {
                background: Background::Color(col(color::surface_container_highest())),
                border: Border {
                    radius: shape::md().into(),
                    width: if focused { 2.0 } else { 1.0 },
                    color: if focused {
                        col(accent.primary)
                    } else {
                        col(color::outline())
                    },
                },
                icon: dim,
                placeholder: dim,
                value: col(color::on_surface()),
                selection: col(accent.primary.with_alpha(0.35)),
            }
        });
    let ready = !app.create_name.trim().is_empty();
    let create: Element<'_, Message> = if ready {
        let b = button(
            container(label_w(
                "Create",
                type_scale::BODY_MD,
                Weight::Medium,
                col(accent.on_primary),
            ))
            .padding(Padding::from([0.0, gap::XL]))
            .height(Length::Fixed(44.0))
            .align_y(Alignment::Center),
        )
        .padding(0)
        .on_press(Message::Create(CreateMsg::Confirm));
        fx(b.style(button_flat(col(accent.primary), shape::FULL)))
            .layer(col(accent.on_primary))
            .radius(shape::FULL)
            .into()
    } else {
        tonal_button("Create", None, None)
    };
    let body = column![
        label(
            "Create playlist",
            type_scale::TITLE_MD,
            true,
            col(color::on_surface())
        ),
        vspace(gap::MD),
        name,
        vspace(gap::SM),
        label(
            "It is created in your YouTube Music account when signed in.",
            type_scale::BODY_SM,
            false,
            dim
        ),
        vspace(gap::LG),
        row![
            create,
            Space::new().width(gap::SM),
            text_button("Cancel", Message::Create(CreateMsg::Close)),
        ]
        .align_y(Alignment::Center),
    ]
    .width(Length::Fixed(420.0));
    iced::widget::stack![
        scrim(0.5, Some(Message::Create(CreateMsg::Close))),
        container(opaque(
            container(body)
                .padding(gap::XL)
                .style(card_style(shape::xl()))
        ))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill),
    ]
    .into()
}

/// The mini player: cover, title, artist, previous, play, next and a way back.
pub fn mini_player(app: &App) -> Element<'_, Message> {
    let accent = app.accent();
    let ink = col(color::on_surface());
    let dim = col(color::on_surface_variant());
    let art_px = 96.0;
    let text_w = MINI_SIZE.0 - art_px - 2.0 * gap::LG - gap::LG - 8.0;
    let info: Element<'_, Message> = match &app.ui.snapshot.current {
        Some(t) => column![
            label(
                fit(&t.title, text_w, type_scale::TITLE_SM.size),
                type_scale::TITLE_SM,
                true,
                ink
            ),
            label(
                fit(
                    &crate::format::artists(&t.artists),
                    text_w,
                    type_scale::BODY_SM.size
                ),
                type_scale::BODY_SM,
                false,
                dim
            ),
        ]
        .into(),
        None => label("Nothing playing", type_scale::TITLE_SM, true, dim).into(),
    };
    let controls = row![
        icon_button(
            Icon::Prev,
            22.0,
            40.0,
            ink,
            Color::TRANSPARENT,
            Some(Message::Ui(crate::state::Event::Command(
                nemo_core::player::PlayerCommand::Previous
            ))),
        ),
        bar::play_button(app, accent, 44.0),
        icon_button(
            Icon::Next,
            22.0,
            40.0,
            ink,
            Color::TRANSPARENT,
            Some(Message::Ui(crate::state::Event::Command(
                nemo_core::player::PlayerCommand::Next
            ))),
        ),
        Space::new().width(Length::Fill),
        tip(
            icon_button(
                Icon::FullScreenExit,
                20.0,
                36.0,
                dim,
                Color::TRANSPARENT,
                Some(Message::ToggleMini),
            ),
            "Back to the full window",
        ),
    ]
    .spacing(gap::XS)
    .align_y(Alignment::Center);
    let right = column![info, vspace(gap::SM), bar::seek_row(app, accent), controls]
        .width(Length::Fixed(text_w + 8.0));
    container(
        row![
            bar::cover_art(app, art_px, shape::lg()),
            Space::new().width(gap::MD),
            right,
        ]
        .align_y(Alignment::Center)
        .padding(gap::LG),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_y(Alignment::Center)
    .style(|_| container::Style {
        background: Some(col(color::surface_container()).into()),
        ..container::Style::default()
    })
    .into()
}

/// Header chip row of Home: All, Music, Videos, Podcasts, with a sliding selection pill.
pub fn content_chips(app: &App) -> Element<'_, Message> {
    let labels: Vec<&str> = crate::explore::ContentFilter::ALL
        .iter()
        .map(|f| f.label())
        .collect();
    let selected = crate::explore::ContentFilter::ALL
        .iter()
        .position(|f| *f == app.content_filter)
        .unwrap_or(0);
    super::kit::sliding_chips(app, &labels, selected, 0, |i| {
        Message::ContentFilter(crate::explore::ContentFilter::ALL[i])
    })
}
