//! The right Now Playing pane: the cover fills the width, then title, artist, actions, the live
//! lyric line and the tabs Lyrics, Queue and Info. The background takes the cover's colour.

use iced::font::Weight;
use iced::widget::{Space, button, column, container, row};
use iced::{Alignment, Color, Element, Length, Padding};
use nemo_core::catalog::ShelfItem;
use nemo_core::lyrics::Granularity;
use nemo_core::track::Track;

use super::super::app::{App, ArtShape, ListSource, Message, NpTab};
use super::super::assets::Icon;
use super::super::fx::{Fx, fx};
use super::super::style::{
    button_flat, col, fit, gap, icon, icon_button, label, label_w, scroll, tip,
};
use super::bar::{badge, cover_art, like_button};
use super::kit::{Card, card_view, paragraph, quick_pick, vspace};
use super::lyrics;
use super::queue;
use super::shell::pane_surface;
use crate::format;
use crate::layout;
use crate::route::Route;
use crate::settings::NowPlayingStyle;
use crate::signal;
use crate::state::{Event, LyricsState};
use crate::theme::{color, shape, type_scale};

/// The pane, `full_w` px wide when open; `slot` is its current width while it animates (the
/// content keeps `full_w` and is clipped).
pub fn view(app: &App, full_w: f32, slot: Length) -> Element<'_, Message> {
    let bg = super::backdrop::pane_background(app);
    let body: Element<'_, Message> = if app.ui.snapshot.current.is_some() {
        playing_in_style(app, full_w)
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

/// The playing pane, drawn by the builder of the chosen style. The empty pane is the same in
/// every style: it shows no Now Playing facts.
fn playing_in_style(app: &App, width: f32) -> Element<'_, Message> {
    match app.ui.settings.now_playing_style {
        NowPlayingStyle::Classic => super::np_classic::pane(app, width),
        // The Apple Music style draws today's look until its builder lands (task 6).
        NowPlayingStyle::M3Expressive | NowPlayingStyle::AppleMusic => playing(app, width),
    }
}

/// Nothing is playing: a short line about it, then what is useful. The recent songs come first;
/// without any, one song from the Home shelves is offered as a card. Nothing here is invented.
fn empty(app: &App, width: f32) -> Element<'_, Message> {
    let accent = app.accent();
    let pad = gap::LG;
    let inner = width - 2.0 * pad;
    let dim = col(color::on_surface_variant());
    let mark = container(icon(Icon::Note, 28.0, col(accent.on_primary_container)))
        .width(Length::Fixed(56.0))
        .height(Length::Fixed(56.0))
        .center_x(Length::Fixed(56.0))
        .center_y(Length::Fixed(56.0))
        .style(move |_| container::Style {
            background: Some(col(accent.primary_container).into()),
            border: iced::Border {
                radius: shape::lg().into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });
    let header = column![
        mark,
        vspace(gap::MD),
        label(
            "Nothing playing",
            type_scale::TITLE_MD,
            true,
            col(color::on_surface())
        ),
        vspace(gap::XS),
        label(
            "Pick a song and its cover, lyrics and queue show up here.",
            type_scale::BODY_MD,
            false,
            dim
        )
        .wrapping(iced::widget::text::Wrapping::Word)
        .width(Length::Fixed(inner))
        .align_x(iced::alignment::Horizontal::Center),
    ]
    .width(Length::Fixed(inner))
    .align_x(Alignment::Center);
    let mut c = column![header, vspace(gap::XL)].width(Length::Fixed(inner));

    let recent = &app.ui.recent;
    if !recent.is_empty() {
        c = c
            .push(label("Play it again", type_scale::LABEL_MD, true, dim))
            .push(vspace(gap::SM));
        for (i, track) in recent.iter().enumerate().take(4) {
            c = c.push(quick_pick(app, track, ListSource::Recent, i, inner, i));
        }
        c = c.push(vspace(gap::LG));
    }
    // A suggestion fills the space when there is little history.
    if recent.len() < 3
        && let Some((source, index, track)) = suggestion(app)
    {
        {
            c = c
                .push(label("Start with", type_scale::LABEL_MD, true, dim))
                .push(vspace(gap::SM))
                .push(card_view(
                    app,
                    Card {
                        artwork: track.artwork.clone(),
                        title: track.title.clone(),
                        subtitle: format::artists(&track.artists),
                        shape: ArtShape::Tile,
                        on_press: Message::PlayAt {
                            source: source.clone(),
                            index,
                        },
                        play: Some(Message::PlayAt { source, index }),
                    },
                    // Card size is the pane width less padding, a small exact number.
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    {
                        inner.max(160.0) as u32
                    },
                    0,
                ));
        }
    }
    container(scroll(container(c).padding(Padding {
        top: pad,
        right: pad,
        bottom: pad,
        left: pad,
    })))
    .width(Length::Fixed(width))
    .height(Length::Fill)
    .into()
}

/// A song worth offering when nothing was played yet: the first song of the Home shelves, or
/// the first local track.
fn suggestion(app: &App) -> Option<(ListSource, usize, Track)> {
    for (n, shelf) in app.home.shelves.iter().enumerate() {
        let songs: Vec<&Track> = shelf
            .items
            .iter()
            .filter_map(|i| match i {
                ShelfItem::Song(t) => Some(t),
                _ => None,
            })
            .collect();
        if let Some(first) = songs.first() {
            return Some((ListSource::Home(n), 0, (*first).clone()));
        }
    }
    app.local_tracks
        .first()
        .map(|t| (ListSource::Library, 0, t.clone()))
}

fn playing(app: &App, width: f32) -> Element<'_, Message> {
    let Some(track) = &app.ui.snapshot.current else {
        return Space::new().into();
    };
    let accent = app.accent();
    let pad = gap::MD;
    let inner = width - 2.0 * pad;
    let ink = col(color::on_surface());
    let dim = col(color::on_surface_variant());
    let (w, h) = app.window_px();
    // The art may be narrower than the pane on a short window; it stays centred.
    // Pixel sizes are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let art = (layout::pane_art(w, h) as f32).min(inner);

    let actions = row![
        like_button(app, track, 22.0, 40.0),
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

    let tab_h = layout::pane_lyrics_height(w, h);
    let tab_body: Element<'_, Message> = match app.np_tab {
        NpTab::Lyrics => lyrics_tab(app, inner, tab_h),
        NpTab::Queue => queue::tab(app, width),
        NpTab::Info => info_tab(app, inner),
    };
    // The lyrics tab shows the lines themselves; the other tabs keep one live line.
    let live: Element<'_, Message> = if app.np_tab == NpTab::Lyrics {
        Space::new().height(gap::SM).into()
    } else {
        container(live_line(app, inner))
            .padding(Padding {
                top: gap::XS,
                bottom: gap::SM,
                ..Padding::ZERO
            })
            .into()
    };

    column![
        container(breathing(app, cover_art(app, art, 18.0)))
            .width(Length::Fixed(width))
            .center_x(Length::Fixed(width))
            .padding(Padding {
                top: pad,
                ..Padding::ZERO
            }),
        container(titles).padding(Padding {
            top: gap::MD,
            right: pad,
            bottom: 0.0,
            left: pad,
        }),
        container(live).padding(Padding::from([0.0, pad])),
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
        col(color::on_surface_variant())
    };
    // The line wraps instead of being cut (wave-lyrics criterion 6); the slot grows with it.
    let rows = super::super::textfit::rows(&text, width, f32::from(st.size));
    let lyric = label_w(text, st, Weight::Semibold, ink)
        .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
        .width(Length::Fixed(width));
    let slot = container(Fx::new(lyric).passive().entrance(
        app.anim.line(),
        8.0,
        col(color::surface_container().with_alpha(0.0)),
    ));
    // Row counts are tiny, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let h = rows as f32 * f32::from(st.line);
    slot.height(Length::Fixed(h)).into()
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
            col(color::on_surface_variant())
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
            background: Some(col(color::surface_container_lowest().with_alpha(0.55)).into()),
            border: iced::Border {
                radius: shape::FULL.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

/// Lyrics in the dense sizes with the spring glide and the provider footer.
fn lyrics_tab(app: &App, width: f32, viewport_h: f32) -> Element<'_, Message> {
    column![
        container(lyrics::body(app, width, true))
            .width(Length::Fill)
            .height(Length::Fixed(
                viewport_h + 6.0 - lyrics::footer_extra(app, width)
            ))
            .clip(true),
        lyrics::footer(app, width),
    ]
    .width(Length::Fixed(width))
    .into()
}

/// Details of the playing song, the honest quality badge and the signal path.
fn info_tab(app: &App, width: f32) -> Element<'_, Message> {
    let Some(track) = &app.ui.snapshot.current else {
        return Space::new().into();
    };
    let dim = col(color::on_surface_variant());
    let ink = col(color::on_surface());
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
            nemo_core::audio::Origin::SoundCloud => "SoundCloud (lossy)".to_owned(),
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

/// The cover breathes with play and pause: 94% while paused, 100% while playing, riding the
/// play button''s spring (so it overshoots a hair, like the button).
pub fn breathing<'a>(app: &App, cover: Element<'a, Message>) -> Element<'a, Message> {
    Fx::new(cover)
        .passive()
        .scale(0.94 + 0.06 * app.anim.play.value)
        .into()
}
