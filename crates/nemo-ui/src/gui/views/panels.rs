//! Panels that float above the window: the tools panel (sleep timer, playback speed, window
//! modes), the keyboard shortcut sheet, the "Create playlist" sheet, the mini player, and the
//! lyrics provider switcher. All are opaque (`surface-container-high`); the capsule stays the
//! only glass layer.

use std::time::Instant;

use iced::font::Weight;
use iced::widget::{Space, button, column, container, opaque, row, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};
use nemo_core::lyrics::Granularity;

use super::super::app::{App, CreateMsg, MINI_SIZE, Message};
use super::super::assets::{Icon, font};
use super::super::fx::fx;
use super::super::style::{
    animated, button_flat, button_outlined, col, fit, gap, icon, icon_button, label, label_w, tip,
};
use super::bar;
use super::kit::{paragraph, text_button, tonal_button, vspace};
use super::overlays::{card_style, scrim};
use crate::explore::{SHORTCUTS, SPEEDS, SleepChoice, timer_left};
use crate::theme::{color, shape, type_scale};

/// A thin section label.
fn heading(text: &str) -> Element<'_, Message> {
    label_w(
        text,
        type_scale::LABEL_MD,
        Weight::Medium,
        col(color::on_surface_variant()),
    )
    .into()
}

/// A full-width action row: glyph, text, optional right-hand hint.
fn action<'a>(
    glyph: Icon,
    text: &'a str,
    hint: Option<&'a str>,
    message: Message,
) -> Element<'a, Message> {
    let ink = col(color::on_surface());
    let mut r = row![
        icon(glyph, 20.0, ink),
        Space::new().width(gap::MD),
        label(text, type_scale::BODY_MD, false, ink),
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center)
    .padding(Padding::from([0.0, gap::MD]))
    .height(Length::Fixed(44.0));
    if let Some(h) = hint {
        r = r.push(label(
            h,
            type_scale::LABEL_MD,
            false,
            col(color::on_surface_variant()),
        ));
    }
    animated(
        button(r).padding(0).width(Length::Fill).on_press(message),
        Color::TRANSPARENT,
        shape::md(),
    )
}

/// The tools panel, opened from the capsule (or `S`): sleep timer, playback speed, shortcuts and
/// window modes.
pub fn tools_panel(app: &App) -> Element<'_, Message> {
    let accent = app.accent();
    let dim = col(color::on_surface_variant());
    let w = 360.0;

    // Sleep timer.
    let mut sleep_row = row![].spacing(gap::SM);
    let mut sleep_col = column![].spacing(gap::SM);
    for (i, c) in SleepChoice::ALL.iter().enumerate() {
        let on = app.sleep.as_ref().is_some_and(|s| s.choice == *c);
        let text = c.label();
        // Chips are owned strings; leak-free by building the label inline.
        sleep_row = sleep_row.push(owned_chip(text, on, accent, Message::Sleep(Some(*c))));
        if i == 2 {
            sleep_col = sleep_col.push(sleep_row);
            sleep_row = row![].spacing(gap::SM);
        }
    }
    sleep_col = sleep_col.push(sleep_row);
    let sleep_status: Element<'_, Message> = match &app.sleep {
        Some(s) => {
            let text = match s.deadline {
                Some(d) => format!(
                    "Pauses in {}",
                    timer_left(d.saturating_duration_since(Instant::now()))
                ),
                None => "Pauses when this song ends".to_owned(),
            };
            row![
                label(text, type_scale::BODY_SM, false, col(color::on_surface())),
                Space::new().width(Length::Fill),
                text_button("Turn off", Message::Sleep(None)),
            ]
            .align_y(Alignment::Center)
            .into()
        }
        None => label(
            "Off. Playback pauses by itself when it ends.",
            type_scale::BODY_SM,
            false,
            dim,
        )
        .into(),
    };

    // Playback speed: off by default; chips work only when the parent wired the engine.
    let wired = app.speed_wired();
    let mut speed_row = row![].spacing(gap::SM);
    for s in SPEEDS {
        let on = app.speed_percent == s;
        let text = if s == 100 {
            "Off".to_owned()
        } else {
            format!("{}x", f64::from(s) / 100.0)
        };
        speed_row = speed_row.push(if wired {
            owned_chip(text, on, accent, Message::Speed(s))
        } else {
            disabled_chip(text, on)
        });
    }
    let speed_note = if !wired {
        "Off. A different speed resamples the audio and ends bit-perfect output, so it needs engine support first."
    } else if app.speed_percent == 100 {
        "Off keeps the audio bit-perfect."
    } else {
        "Speed is on: the audio is resampled and is not bit-perfect."
    };

    let body = column![
        row![
            label(
                "Tools",
                type_scale::TITLE_SM,
                true,
                col(color::on_surface())
            ),
            Space::new().width(Length::Fill),
            icon_button(
                Icon::Close,
                18.0,
                32.0,
                dim,
                Color::TRANSPARENT,
                Some(Message::Tools(false)),
            ),
        ]
        .align_y(Alignment::Center),
        vspace(gap::SM),
        heading("Sleep timer"),
        vspace(gap::SM),
        sleep_col,
        vspace(gap::SM),
        sleep_status,
        vspace(gap::LG),
        heading("Playback speed"),
        vspace(gap::SM),
        speed_row,
        vspace(gap::SM),
        paragraph(speed_note, type_scale::BODY_SM, dim, w - 2.0 * gap::LG),
        vspace(gap::LG),
        action(
            Icon::FullScreen,
            "Full-screen Now Playing",
            Some("F"),
            Message::ToggleFullScreen
        ),
        action(Icon::Mini, "Mini player", Some("M"), Message::ToggleMini),
        action(
            Icon::Keyboard,
            "Keyboard shortcuts",
            Some("?"),
            Message::Shortcuts(true)
        ),
    ]
    .width(Length::Fixed(w - 2.0 * gap::LG));
    opaque(
        container(body)
            .padding(gap::LG)
            .width(Length::Fixed(w))
            .style(card_style(shape::xl())),
    )
}

fn owned_chip<'a>(
    text: String,
    on: bool,
    accent: crate::palette::Palette,
    message: Message,
) -> Element<'a, Message> {
    let (fill, ink, outline) = if on {
        (
            col(accent.secondary_container),
            col(accent.on_secondary_container),
            None,
        )
    } else {
        (
            Color::TRANSPARENT,
            col(color::on_surface()),
            Some(col(color::outline())),
        )
    };
    let b = button(
        container(label_w(text, type_scale::LABEL_MD, Weight::Medium, ink))
            .padding(Padding::from([0.0, gap::MD]))
            .height(Length::Fixed(32.0))
            .align_y(Alignment::Center),
    )
    .padding(0)
    .on_press(message);
    fx(b.style(button_outlined(fill, outline, shape::FULL)))
        .layer(ink)
        .radius(shape::FULL)
        .into()
}

fn disabled_chip<'a>(text: String, on: bool) -> Element<'a, Message> {
    let ink = col(color::on_surface().with_alpha(0.38));
    let outline = col(color::outline_variant());
    let fill = if on {
        col(color::on_surface().with_alpha(0.08))
    } else {
        Color::TRANSPARENT
    };
    let b = button(
        container(label_w(text, type_scale::LABEL_MD, Weight::Medium, ink))
            .padding(Padding::from([0.0, gap::MD]))
            .height(Length::Fixed(32.0))
            .align_y(Alignment::Center),
    )
    .padding(0);
    fx(b.style(button_outlined(fill, Some(outline), shape::FULL)))
        .passive()
        .radius(shape::FULL)
        .into()
}

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

/// A lyrics provider switcher: one chip per provider that has lyrics for the song, the timing
/// granularity under its name, and `on_pick` carrying the index.
pub fn lyrics_switcher(app: &App, on_pick: fn(usize) -> Message) -> Element<'_, Message> {
    let accent = app.accent();
    let selected = app.lyric_provider;
    let mut r = row![].spacing(gap::SM).align_y(Alignment::Center);
    for (i, cand) in app.lyric_candidates.iter().enumerate() {
        let name = cand.provider.as_str();
        let on = i == selected;
        let kind = match cand.granularity {
            Granularity::Line => "line",
            Granularity::Word => "word",
            Granularity::Syllable => "syllable",
            Granularity::Unsynced => "text",
        };
        let ink = if on {
            col(accent.on_secondary_container)
        } else {
            col(color::on_surface())
        };
        let b = button(
            container(
                row![
                    label_w(
                        fit(name, 96.0, type_scale::LABEL_MD.size),
                        type_scale::LABEL_MD,
                        Weight::Medium,
                        ink
                    ),
                    Space::new().width(gap::XS),
                    label(kind, type_scale::LABEL_SM, false, ink.scale_alpha(0.72)),
                ]
                .align_y(Alignment::Center),
            )
            .padding(Padding::from([0.0, gap::MD]))
            .height(Length::Fixed(32.0))
            .align_y(Alignment::Center),
        )
        .padding(0)
        .on_press(on_pick(i));
        let (fill, outline) = if on {
            (col(accent.secondary_container), None)
        } else {
            (Color::TRANSPARENT, Some(col(color::outline())))
        };
        r = r.push(tip(
            fx(b.style(button_outlined(fill, outline, shape::FULL)))
                .layer(ink)
                .radius(shape::FULL),
            "Lyrics source",
        ));
    }
    r.into()
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
