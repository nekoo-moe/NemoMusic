//! The right Now Playing pane: the cover fills the width, then title, artist, actions, the live
//! lyric line and the tabs Lyrics, Queue and Info. The background takes the cover's colour.

use iced::font::Weight;
use iced::widget::{Space, button, column, container, mouse_area, row, stack};
use iced::{Alignment, Color, Element, Length, Padding, mouse};
use nemo_core::lyrics::Granularity;

use super::super::app::{App, Message, NpTab};
use super::super::assets::Icon;
use super::super::fx::{Fx, fx};
use super::super::style::{
    button_flat, col, fit, gap, icon, icon_button, label, label_w, scroll, tip, vertical_gradient,
};
use super::super::widgets::Offset;
use super::bar::{badge, cover_art};
use super::kit::{paragraph, vspace};
use super::now_playing::{footer, lyric_ink, lyrics_status, pill_button, status_text};
use super::queue;
use super::shell::pane_surface;
use crate::format;
use crate::layout;
use crate::lyrics_view::{LINE_GAP, LineRole};
use crate::route::Route;
use crate::signal;
use crate::state::{Event, LyricsState};
use crate::theme::{color, shape, type_scale};

/// The pane, `full_w` px wide when open; `slot` is its current width while it animates (the
/// content keeps `full_w` and is clipped).
pub fn view(app: &App, full_w: f32, slot: Length) -> Element<'_, Message> {
    let accent = app.accent();
    let base = color::SURFACE_CONTAINER;
    let top = accent.primary_container.with_alpha(0.50).over(base);
    let bg = vertical_gradient(col(top), col(base));
    let body: Element<'_, Message> = if app.ui.snapshot.current.is_some() {
        playing(app, full_w)
    } else {
        empty(app, full_w)
    };
    pane_surface(
        container(body)
            .width(Length::Fixed(full_w))
            .height(Length::Fill),
        slot,
        bg,
    )
    .into()
}

fn empty(app: &App, width: f32) -> Element<'_, Message> {
    let accent = app.accent();
    let mark = container(icon(Icon::Note, 40.0, col(accent.on_primary_container)))
        .width(Length::Fixed(96.0))
        .height(Length::Fixed(96.0))
        .center_x(Length::Fixed(96.0))
        .center_y(Length::Fixed(96.0))
        .style(move |_| container::Style {
            background: Some(col(accent.primary_container).into()),
            border: iced::Border {
                radius: 32.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });
    container(
        column![
            mark,
            vspace(gap::LG),
            label(
                "Nothing playing",
                type_scale::TITLE_MD,
                true,
                col(color::ON_SURFACE)
            ),
            vspace(gap::XS),
            paragraph(
                "Pick a song and its cover, lyrics and queue show up here.",
                type_scale::BODY_MD,
                col(color::ON_SURFACE_VARIANT),
                width - 64.0
            ),
        ]
        .align_x(Alignment::Center),
    )
    .width(Length::Fixed(width))
    .height(Length::Fill)
    .center_x(Length::Fixed(width))
    .center_y(Length::Fill)
    .into()
}

fn playing(app: &App, width: f32) -> Element<'_, Message> {
    let Some(track) = &app.ui.snapshot.current else {
        return Space::new().into();
    };
    let accent = app.accent();
    let pad = gap::MD;
    let inner = width - 2.0 * pad;
    let ink = col(color::ON_SURFACE);
    let dim = col(color::ON_SURFACE_VARIANT);

    let actions = row![
        tip(
            icon_button(Icon::Heart, 22.0, 40.0, dim, Color::TRANSPARENT, None),
            "Likes are not available yet",
        ),
        tip(
            icon_button(
                Icon::More,
                22.0,
                40.0,
                ink,
                Color::TRANSPARENT,
                Some(Message::Ui(Event::OpenMenu(track.clone()))),
            ),
            "More",
        ),
        tip(
            icon_button(
                Icon::Expand,
                20.0,
                40.0,
                ink,
                Color::TRANSPARENT,
                Some(Message::Ui(Event::Go(Route::NowPlaying))),
            ),
            "Full window",
        ),
    ];
    let artist_w = inner - 3.0 * 40.0 - gap::SM;
    let titles = column![
        label(
            fit(&track.title, inner, type_scale::TITLE_LG.size),
            type_scale::TITLE_LG,
            true,
            ink
        ),
        row![
            label(
                fit(
                    &format::artists(&track.artists),
                    artist_w,
                    type_scale::BODY_MD.size
                ),
                type_scale::BODY_MD,
                false,
                dim
            ),
            Space::new().width(Length::Fill),
            actions,
        ]
        .align_y(Alignment::Center)
        .width(Length::Fixed(inner)),
    ]
    .width(Length::Fixed(inner));

    let (w, h) = app.window_px();
    let tab_h = layout::pane_lyrics_height(w, h);
    let tab_body: Element<'_, Message> = match app.np_tab {
        NpTab::Lyrics => lyrics_tab(app, inner, tab_h),
        NpTab::Queue => queue::tab(app, width),
        NpTab::Info => info_tab(app, inner),
    };

    column![
        container(cover_art(app, inner, 18.0)).padding(Padding {
            top: pad,
            right: pad,
            bottom: 0.0,
            left: pad,
        }),
        container(titles).padding(Padding {
            top: gap::MD,
            right: pad,
            bottom: 0.0,
            left: pad,
        }),
        container(live_line(app, inner)).padding(Padding {
            top: gap::XS,
            right: pad,
            bottom: gap::SM,
            left: pad,
        }),
        container(tabs(
            app,
            accent.secondary_container,
            accent.on_secondary_container,
            inner
        ))
        .padding(Padding::from([0.0, pad])),
        container(tab_body)
            .padding(Padding {
                top: gap::SM,
                right: if app.np_tab == NpTab::Queue { 0.0 } else { pad },
                bottom: 0.0,
                left: if app.np_tab == NpTab::Queue { 0.0 } else { pad },
            })
            .width(Length::Fixed(width))
            .height(Length::Fill)
            .clip(true),
    ]
    .width(Length::Fixed(width))
    .into()
}

/// The live lyric line: the current line of synced lyrics, or why there is none.
fn live_line(app: &App, width: f32) -> Element<'_, Message> {
    let st = type_scale::TITLE_SM;
    let (text, live) = match &app.ui.lyrics {
        LyricsState::Ready(doc) if doc.instrumental => ("Instrumental".to_owned(), false),
        LyricsState::Ready(doc) if doc.granularity == Granularity::Unsynced => {
            ("These lyrics are not synced".to_owned(), false)
        }
        LyricsState::Ready(doc) => match app.ui.active_lyric().and_then(|i| doc.lines.get(i)) {
            Some(l) => (l.text.clone(), true),
            None => ("\u{266a}".to_owned(), false),
        },
        LyricsState::Missing(_) | LyricsState::Loading | LyricsState::Idle => {
            (String::new(), false)
        }
    };
    let ink = if live {
        col(app.accent().primary)
    } else {
        col(color::ON_SURFACE_VARIANT)
    };
    let lyric = label_w(fit(&text, width, st.size), st, Weight::Semibold, ink);
    let slot = container(Fx::new(lyric).passive().entrance(
        app.anim.line(),
        8.0,
        col(color::SURFACE_CONTAINER.with_alpha(0.0)),
    ))
    .height(Length::Fixed(f32::from(st.line)));
    slot.into()
}

/// The segmented tab switch.
fn tabs(
    app: &App,
    on_fill: crate::theme::Rgba,
    on_ink: crate::theme::Rgba,
    width: f32,
) -> Element<'_, Message> {
    let tab_w = (width - 2.0 * gap::XS) / 3.0;
    let mut r = row![].spacing(0);
    for t in NpTab::ALL {
        let on = app.np_tab == t;
        let ink = if on {
            col(on_ink)
        } else {
            col(color::ON_SURFACE_VARIANT)
        };
        let b = button(
            container(label_w(t.label(), type_scale::BODY_MD, Weight::Medium, ink))
                .width(Length::Fixed(tab_w))
                .height(Length::Fixed(36.0))
                .center_x(Length::Fixed(tab_w))
                .center_y(Length::Fixed(36.0)),
        )
        .padding(0)
        .on_press(Message::NpTab(t));
        let idle = if on { col(on_fill) } else { Color::TRANSPARENT };
        r = r.push(
            fx(b.style(button_flat(idle, shape::FULL)))
                .layer(ink)
                .radius(shape::FULL),
        );
    }
    container(r.padding(gap::XS))
        .width(Length::Fixed(width))
        .style(|_| container::Style {
            background: Some(col(color::SURFACE_CONTAINER_LOWEST.with_alpha(0.55)).into()),
            border: iced::Border {
                radius: shape::FULL.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

/// Lyrics in the dense sizes with the spring glide, the provider footer and the offset pills.
fn lyrics_tab(app: &App, width: f32, viewport_h: f32) -> Element<'_, Message> {
    let body: Element<'_, Message> = match &app.ui.lyrics {
        LyricsState::Ready(doc) if doc.instrumental => status_text("Instrumental", None),
        LyricsState::Ready(doc) if doc.granularity == Granularity::Unsynced => {
            let mut c = column![].spacing(gap::SM).width(Length::Fill);
            for line in &doc.lines {
                c = c.push(
                    label(
                        line.text.clone(),
                        type_scale::BODY_MD,
                        false,
                        col(color::ON_SURFACE_VARIANT),
                    )
                    .wrapping(iced::widget::text::Wrapping::Word),
                );
            }
            scroll(c).height(Length::Fill).into()
        }
        LyricsState::Ready(doc) => {
            let active = app.ui.active_lyric();
            let mut c = column![].width(Length::Fill);
            for (i, line) in doc.lines.iter().enumerate() {
                let role = LineRole::of(i, active);
                let st = role.style_d(true);
                c = c.push(
                    container(label(
                        fit(&line.text, width, st.size),
                        st,
                        role.semibold(),
                        lyric_ink(role),
                    ))
                    .height(Length::Fixed(f32::from(st.line) + LINE_GAP))
                    .width(Length::Fill),
                );
            }
            let list = mouse_area(Offset::new(c, app.ui.lyric_scroll.value)).on_scroll(|delta| {
                let px = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => -y * 48.0,
                    mouse::ScrollDelta::Pixels { y, .. } => -y,
                };
                Message::Ui(Event::LyricsScrolled(px))
            });
            let mut layers: Vec<Element<'_, Message>> = vec![list.into()];
            if !app.ui.lyrics_following() {
                layers.push(
                    container(pill_button(
                        "Back to current line",
                        Message::Ui(Event::LyricsFollow),
                    ))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_y(Alignment::End)
                    .center_x(Length::Fill)
                    .padding(gap::SM)
                    .into(),
                );
            }
            stack(layers).into()
        }
        _ => lyrics_status(app, width),
    };
    column![
        container(body)
            .width(Length::Fill)
            .height(Length::Fixed(viewport_h + 6.0))
            .clip(true),
        footer(app, width),
    ]
    .width(Length::Fixed(width))
    .into()
}

/// Details of the playing song, the honest quality badge and the signal path.
fn info_tab(app: &App, width: f32) -> Element<'_, Message> {
    let Some(track) = &app.ui.snapshot.current else {
        return Space::new().into();
    };
    let dim = col(color::ON_SURFACE_VARIANT);
    let ink = col(color::ON_SURFACE);
    let pair = |name: &'static str, value: String| -> Element<'_, Message> {
        column![
            label(name, type_scale::LABEL_MD, false, dim),
            paragraph(value, type_scale::BODY_MD, ink, width),
        ]
        .spacing(gap::XS)
        .into()
    };
    let mut c = column![
        pair("Title", track.title.clone()),
        pair("Artist", format::artists(&track.artists)),
    ]
    .spacing(gap::MD)
    .width(Length::Fixed(width));
    if let Some(album) = &track.album {
        c = c.push(pair("Album", album.clone()));
    }
    c = c.push(pair(
        "Source",
        match track.origin {
            nemo_core::audio::Origin::Local => "Your music folder".to_owned(),
            nemo_core::audio::Origin::YoutubeMusic => "YouTube Music (lossy)".to_owned(),
        },
    ));
    c = c.push(
        column![
            label("Quality", type_scale::LABEL_MD, false, dim),
            badge(app, width),
        ]
        .spacing(gap::XS),
    );
    if let Some(path) = app.ui.snapshot.signal_path.as_ref() {
        for line in signal::lines(path) {
            c = c.push(pair(line.label, line.value));
        }
    }
    let padded = container(c).padding(Padding {
        bottom: super::kit::PAGE_BOTTOM,
        ..Padding::ZERO
    });
    scroll(padded).height(Length::Fill).into()
}
