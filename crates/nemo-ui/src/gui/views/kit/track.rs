//! Track rows, Quick picks rows, track lists and the equaliser bars of the playing row.

use iced::widget::{Space, button, column, container, mouse_area, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};
use nemo_core::audio::Origin;
use nemo_core::track::Track;

use super::{QUICK_H, ROW_H, art, pane_bg, play_badge, text_button, vspace};
use crate::gui::app::{App, ArtShape, ListSource, Message};
use crate::gui::assets::Icon;
use crate::gui::fx::{BadgeAnchor, entrance, fx};
use crate::gui::style::{button_flat, col, fit, gap, icon_button, label, tip};
use crate::state::Event;
use crate::theme::{color, shape, type_scale};

/// Where a row sits and what it shows.
pub struct RowCtx {
    /// Row width in px.
    pub width: f32,
    /// The list the rows belong to.
    pub source: ListSource,
    /// Show the position number instead of the cover.
    pub numbered: bool,
    /// Show the album column.
    pub show_album: bool,
}

/// One track row, 56 px high: art or number, title and artist, album, badge, duration, menu.
pub fn track_row<'a>(
    app: &'a App,
    track: &Track,
    index: usize,
    ctx: &RowCtx,
) -> Element<'a, Message> {
    let snap = &app.ui.snapshot;
    let current = snap.current.as_ref().map(|t| &t.id) == Some(&track.id);
    let dim = col(color::on_surface_variant());
    let ink = col(color::on_surface());
    let width = ctx.width;
    let accent = app.accent();

    let lead: Element<'a, Message> = if ctx.numbered {
        let inner: Element<'a, Message> = if current {
            eq_bars(app, col(accent.primary), 16.0)
        } else {
            label(format!("{}", index + 1), type_scale::BODY_MD, false, dim).into()
        };
        container(inner)
            .width(Length::Fixed(44.0))
            .height(Length::Fixed(44.0))
            .center_x(Length::Fixed(44.0))
            .center_y(Length::Fixed(44.0))
            .into()
    } else {
        art(app, &track.artwork, 44, ArtShape::Row)
    };

    let show_badge = width >= 640.0;
    let show_album = ctx.show_album && width >= 760.0;
    let badge_w = if show_badge { 96.0 + gap::MD } else { 0.0 };
    // lead 44 + gap 12, duration 56, menu 40, row padding 2 x 8.
    let fixed = 44.0 + gap::MD + 56.0 + 40.0 + 2.0 * gap::SM + badge_w;
    let flex = (width - fixed).max(120.0);
    let (title_w, album_w) = if show_album {
        (flex * 0.55 - gap::MD, flex * 0.45)
    } else {
        (flex, 0.0)
    };

    let mut line = row![
        lead,
        Space::new().width(gap::MD),
        column![
            label(
                fit(&track.title, title_w, type_scale::BODY_MD.size),
                type_scale::BODY_MD,
                true,
                if current { col(accent.primary) } else { ink }
            ),
            label(
                fit(
                    &crate::format::artists(&track.artists),
                    title_w,
                    type_scale::BODY_SM.size
                ),
                type_scale::BODY_SM,
                false,
                dim
            ),
        ]
        .width(Length::Fixed(title_w)),
    ]
    .align_y(Alignment::Center);
    if show_album {
        line = line.push(Space::new().width(gap::MD)).push(
            container(label(
                fit(
                    track.album.as_deref().unwrap_or(""),
                    album_w,
                    type_scale::BODY_MD.size,
                ),
                type_scale::BODY_MD,
                false,
                dim,
            ))
            .width(Length::Fixed(album_w)),
        );
    }
    line = line.push(Space::new().width(Length::Fill));
    if current && !ctx.numbered {
        line = line
            .push(eq_bars(app, col(accent.primary), 16.0))
            .push(Space::new().width(gap::MD));
    }
    if show_badge {
        let text = if current {
            snap.signal_path
                .as_ref()
                .map_or_else(|| "Unknown".to_owned(), |p| p.source.badge())
        } else {
            match track.origin {
                Origin::Local => "Local".to_owned(),
                Origin::YoutubeMusic => "YouTube".to_owned(),
            }
        };
        line = line.push(
            container(label(
                fit(&text, 96.0, type_scale::LABEL_SM.size),
                type_scale::LABEL_SM,
                false,
                dim,
            ))
            .width(Length::Fixed(96.0 + gap::MD))
            .align_x(Alignment::End)
            .padding(Padding {
                right: gap::MD,
                ..Padding::ZERO
            }),
        );
    }
    line = line.push(
        container(label(
            track
                .duration
                .map_or_else(|| "-:--".to_owned(), crate::format::clock),
            type_scale::BODY_MD,
            false,
            dim,
        ))
        .width(Length::Fixed(56.0))
        .align_x(Alignment::End),
    );

    let idle = if current {
        col(color::surface_container_high())
    } else {
        Color::TRANSPARENT
    };
    let main = button(
        container(line)
            .height(Length::Fixed(ROW_H))
            .width(Length::Fill)
            .padding(Padding::from([0.0, gap::SM]))
            .align_y(Alignment::Center),
    )
    .padding(0)
    .width(Length::Fill)
    .on_press(Message::PlayAt {
        source: ctx.source.clone(),
        index,
    });
    let radius = shape::lg();
    let host = fx(main.style(button_flat(idle, radius)))
        .radius(radius)
        .badge(play_badge(
            app,
            32.0,
            BadgeAnchor::CenterLeft(if ctx.numbered { 6.0 } else { 14.0 }),
            Message::PlayAt {
                source: ctx.source.clone(),
                index,
            },
        ));
    let host = mouse_area(host).on_right_press(Message::Ui(Event::OpenMenu(track.clone())));
    row![
        host,
        tip(
            icon_button(
                Icon::More,
                20.0,
                40.0,
                dim,
                Color::TRANSPARENT,
                Some(Message::Ui(Event::OpenMenu(track.clone()))),
            ),
            "More",
        ),
    ]
    .align_y(Alignment::Center)
    .height(Length::Fixed(ROW_H))
    .into()
}

/// A "Quick picks" row: art 56, title and artist; the playing song is tinted.
pub fn quick_pick<'a>(
    app: &'a App,
    track: &'a Track,
    source: ListSource,
    index: usize,
    width: f32,
    stagger: usize,
) -> Element<'a, Message> {
    let current = app.ui.snapshot.current.as_ref().map(|t| &t.id) == Some(&track.id);
    let accent = app.accent();
    let text_w = (width - 56.0 - gap::MD - 2.0 * gap::SM - gap::SM).max(80.0);
    let line = row![
        art(app, &track.artwork, 56, ArtShape::Thumb),
        Space::new().width(gap::MD),
        column![
            label(
                fit(&track.title, text_w, type_scale::BODY_MD.size),
                type_scale::BODY_MD,
                true,
                if current {
                    col(accent.primary)
                } else {
                    col(color::on_surface())
                }
            ),
            label(
                fit(
                    &crate::format::artists(&track.artists),
                    text_w,
                    type_scale::BODY_SM.size
                ),
                type_scale::BODY_SM,
                false,
                col(color::on_surface_variant())
            ),
        ]
        .width(Length::Fixed(text_w)),
    ]
    .align_y(Alignment::Center);
    let b = button(
        container(line)
            .width(Length::Fixed(width))
            .height(Length::Fixed(QUICK_H))
            .padding(Padding::from([0.0, gap::SM]))
            .align_y(Alignment::Center),
    )
    .padding(0)
    .on_press(Message::PlayAt {
        source: source.clone(),
        index,
    });
    let source2 = source;
    let idle = if current {
        col(color::surface_container_high())
    } else {
        Color::TRANSPARENT
    };
    let radius = shape::lg();
    let host = fx(b.style(button_flat(idle, radius)))
        .radius(radius)
        .badge(play_badge(
            app,
            36.0,
            BadgeAnchor::CenterRight(12.0),
            Message::PlayAt {
                source: source2,
                index,
            },
        ));
    let host = mouse_area(host).on_right_press(Message::Ui(Event::OpenMenu(track.clone())));
    entrance(host, app.anim.reveal(stagger), pane_bg())
}

/// Rows of `tracks`, at most `limit` of them, then a "Show more" button when more exist.
pub fn track_list<'a>(
    app: &'a App,
    tracks: &[Track],
    limit: usize,
    ctx: &RowCtx,
) -> Element<'a, Message> {
    let mut c = column![];
    for (i, t) in tracks.iter().enumerate().take(limit) {
        c = c.push(track_row(app, t, i, ctx));
    }
    if tracks.len() > limit {
        c = c
            .push(vspace(gap::SM))
            .push(text_button("Show more", Message::ShowMore));
    }
    c.into()
}

/// Three equaliser bars on the playing row. They move while a song plays and rest at fixed
/// heights when paused. Purely a state indicator: it is not audio analysis.
pub fn eq_bars<'a>(app: &App, ink: Color, h: f32) -> Element<'a, Message> {
    let moving = app.ui.is_playing() && !app.ui.is_buffering() && !app.ui.reduced_motion;
    let mut r = row![].spacing(2.0).align_y(Alignment::End);
    for i in 0..3_u8 {
        let k = f32::from(i);
        let level = if moving {
            0.30 + 0.70 * (app.eq_phase * (1.0 + 0.41 * k) + k * 1.9).sin().abs()
        } else {
            [0.40, 0.70, 0.50][usize::from(i)]
        };
        r = r.push(
            container(Space::new())
                .width(Length::Fixed(3.0))
                .height(Length::Fixed((h * level).max(3.0)))
                .style(move |_| container::Style {
                    background: Some(Background::Color(ink)),
                    border: Border {
                        radius: 1.5.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                }),
        );
    }
    container(r)
        .width(Length::Fixed(13.0))
        .height(Length::Fixed(h))
        .align_y(Alignment::End)
        .into()
}
