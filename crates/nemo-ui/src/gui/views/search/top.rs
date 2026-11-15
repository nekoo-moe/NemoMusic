//! The Top result card: YouTube Music's best match for the query, above the sections.
//!
//! Cover (`art.card`, a circle for an artist), title, a `Type · artist` line and the actions:
//! Play and Shuffle for an album or a playlist, Play for a song or a video. An artist opens its
//! page. No quality label: the card shows no stream yet.

use iced::widget::{Space, button, column, container, mouse_area, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, mouse};
use nemo_core::catalog::TopResult;
use nemo_core::track::{Artwork, Track};

use super::super::super::app::{App, ArtShape, DetailKey, DetailKind, Message};
use super::super::super::assets::Icon;
use super::super::kit::{art, eyebrow, filled_button, tonal_button, vspace};
use crate::format;
use crate::gui::style::{animated, col, fit, gap, label};
use crate::layout::CARD_MIN;
use crate::route::Route;
use crate::state::Event;
use crate::theme::{color, shape, type_scale};

/// The cover slot in px (`art.card`).
// 160 is exact in f32.
#[allow(clippy::cast_precision_loss)]
const ART_PX: f32 = CARD_MIN as f32;

/// What the card shows, whatever the kind of result.
struct Shown {
    artwork: Artwork,
    shape: ArtShape,
    title: String,
    line: String,
    open: Option<Message>,
    play: Option<Message>,
    shuffle: Option<Message>,
}

fn line(kind: &str, parts: impl IntoIterator<Item = String>) -> String {
    std::iter::once(kind.to_owned())
        .chain(parts.into_iter().filter(|p| !p.is_empty()))
        .collect::<Vec<_>>()
        .join(" \u{b7} ")
}

fn play_one(track: &Track) -> Message {
    Message::Ui(Event::PlayTracks {
        tracks: vec![track.clone()],
        start: 0,
    })
}

fn shown(top: &TopResult) -> Shown {
    match top {
        TopResult::Song(t) => Shown {
            artwork: t.artwork.clone(),
            shape: ArtShape::Card,
            title: t.title.clone(),
            line: line("Song", [format::artists(&t.artists)]),
            open: None,
            play: Some(play_one(t)),
            shuffle: None,
        },
        TopResult::Video(v) => Shown {
            artwork: v.track.artwork.clone(),
            shape: ArtShape::Card,
            title: v.track.title.clone(),
            line: line(
                "Video",
                [
                    format::artists(&v.track.artists),
                    v.views.clone().unwrap_or_default(),
                ],
            ),
            open: None,
            play: Some(play_one(&v.track)),
            shuffle: None,
        },
        TopResult::Album(a) => {
            let key = DetailKey {
                kind: DetailKind::Album,
                id: a.id.clone(),
            };
            Shown {
                artwork: Artwork::Remote(a.thumbnails.clone()),
                shape: ArtShape::Card,
                title: a.title.clone(),
                line: line(
                    "Album",
                    [
                        format::artists(&a.artists),
                        a.year.map(|y| y.to_string()).unwrap_or_default(),
                    ],
                ),
                open: Some(Message::Ui(Event::Go(Route::Album(a.id.clone())))),
                play: Some(Message::PlayCollection(key.clone())),
                shuffle: Some(Message::ShuffleCollection(key)),
            }
        }
        TopResult::Playlist(p) => {
            let key = DetailKey {
                kind: DetailKind::Playlist,
                id: p.id.clone(),
            };
            Shown {
                artwork: Artwork::Remote(p.thumbnails.clone()),
                shape: ArtShape::Card,
                title: p.title.clone(),
                line: line("Playlist", p.author.clone()),
                open: Some(Message::Ui(Event::Go(Route::Playlist(p.id.clone())))),
                play: Some(Message::PlayCollection(key.clone())),
                shuffle: Some(Message::ShuffleCollection(key)),
            }
        }
        TopResult::Artist(a) => Shown {
            artwork: Artwork::Remote(a.thumbnails.clone()),
            shape: ArtShape::Circle,
            title: a.name.clone(),
            line: "Artist".to_owned(),
            open: Some(Message::Ui(Event::Go(Route::Artist(a.id.clone())))),
            play: None,
            shuffle: None,
        },
    }
}

/// The card, `width` px wide, under a "Top result" eyebrow.
pub(super) fn top_card<'a>(app: &'a App, top: &TopResult, width: f32) -> Element<'a, Message> {
    let s = shown(top);
    let text_w = (width - ART_PX - gap::XL - 2.0 * gap::LG).max(ART_PX);
    let mut title: Element<'a, Message> = label(
        fit(&s.title, text_w, type_scale::TITLE_MD.size),
        type_scale::TITLE_MD,
        true,
        col(color::on_surface()),
    )
    .into();
    if let Some(open) = s.open.clone() {
        // The title opens the page too, like the cover.
        title = mouse_area(title)
            .on_press(open)
            .interaction(mouse::Interaction::Pointer)
            .into();
    }
    let mut info = column![
        title,
        vspace(gap::XS),
        label(
            fit(&s.line, text_w, type_scale::BODY_MD.size),
            type_scale::BODY_MD,
            false,
            col(color::on_surface_variant()),
        ),
    ];
    let mut actions = row![].align_y(Alignment::Center);
    if let Some(play) = s.play {
        actions = actions.push(filled_button("Play", Icon::Play, app.accent(), play));
    }
    if let Some(shuffle) = s.shuffle {
        actions = actions.push(Space::new().width(gap::MD)).push(tonal_button(
            "Shuffle",
            Some(Icon::Shuffle),
            Some(shuffle),
        ));
    }
    info = info.push(vspace(gap::LG)).push(actions);
    let cover = art(app, &s.artwork, CARD_MIN, s.shape);
    let cover: Element<'a, Message> = match s.open {
        Some(open) => animated(
            button(cover).padding(0).on_press(open),
            Color::TRANSPARENT,
            s.shape.radius(ART_PX),
        ),
        None => cover,
    };
    let body = row![
        cover,
        Space::new().width(gap::XL),
        info.width(Length::Fixed(text_w))
    ]
    .align_y(Alignment::Center);
    column![
        eyebrow("Top result"),
        vspace(gap::SM),
        container(body)
            .width(Length::Fixed(width))
            .padding(Padding::from(gap::LG))
            .style(|_| container::Style {
                background: Some(Background::Color(col(color::surface_container_high()))),
                border: Border {
                    radius: shape::md().into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
        vspace(gap::XXL),
    ]
    .into()
}
