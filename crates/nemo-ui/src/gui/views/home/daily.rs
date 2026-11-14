//! Home: one daily section, with the mood tile and its search text.

use iced::widget::{Space, column, row};
use iced::{Alignment, Element, Length, Padding};
use nemo_core::track::Artwork;

use crate::format;
use crate::gui::app::{App, ArtShape, DetailKey, DetailKind, ListSource, Message};
use crate::gui::assets::Icon;
use crate::gui::style::{col, gap, label};
use crate::gui::views::kit::{Card, card_grid, section_title, vspace};
use crate::layout;
use crate::route::Route;
use crate::state::Event;
use crate::theme::{color, type_scale};

/// One daily section: header with its honest source line, then cards or the mood tile.
pub(super) fn daily_section<'a>(
    app: &'a App,
    index: usize,
    sec: &'a crate::daily::DailySection,
    grid: layout::Grid,
    content_w: f32,
) -> Element<'a, Message> {
    use crate::daily::DailyKind;
    let dim = col(color::on_surface_variant());
    let mut c = column![
        section_title(sec.title.clone()),
        label(sec.tag, type_scale::LABEL_MD, false, dim),
        vspace(gap::MD),
    ];
    match &sec.kind {
        DailyKind::MoodOfDay(name) => {
            let mood = crate::explore::MOODS
                .iter()
                .find(|m| m.label.eq_ignore_ascii_case(name))
                .copied()
                .unwrap_or(crate::explore::MOODS[0]);
            let (fill, ink) = crate::gui::views::explore::tile_color(&mood);
            let radius = crate::theme::shape::lg();
            let b = iced::widget::button(
                iced::widget::container(
                    row![
                        label(name.clone(), type_scale::TITLE_MD, true, col(ink)),
                        Space::new().width(Length::Fill),
                        label("Search this mood", type_scale::BODY_SM, false, col(ink)),
                        Space::new().width(gap::SM),
                        crate::gui::style::icon(Icon::ArrowForward, 18.0, col(ink)),
                    ]
                    .align_y(Alignment::Center),
                )
                .padding(Padding::from([0.0, gap::XL]))
                .width(Length::Fixed(content_w.min(520.0)))
                .height(Length::Fixed(72.0))
                .align_y(Alignment::Center),
            )
            .padding(0)
            .on_press(Message::SearchFor(mood_query(name)));
            c = c.push(
                crate::gui::fx::fx(b.style(move |_, _| iced::widget::button::Style {
                    background: Some(col(fill).into()),
                    text_color: col(ink),
                    border: iced::Border {
                        radius: radius.into(),
                        ..iced::Border::default()
                    },
                    shadow: iced::Shadow::default(),
                    snap: false,
                }))
                .layer(col(ink))
                .radius(radius)
                .lift(2.0),
            );
        }
        _ if !sec.albums.is_empty() => {
            let cards = sec
                .albums
                .iter()
                .map(|a| Card {
                    artwork: Artwork::Remote(a.thumbnails.clone()),
                    title: a.title.clone(),
                    subtitle: format::artists(&a.artists),
                    shape: ArtShape::Tile,
                    on_press: Message::Ui(Event::Go(Route::Album(a.id.clone()))),
                    play: Some(Message::PlayCollection(DetailKey {
                        kind: DetailKind::Album,
                        id: a.id.clone(),
                    })),
                })
                .collect();
            c = c.push(card_grid(app, grid, cards, 1, 3 + index));
        }
        _ => {
            let cards = sec
                .tracks
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let play = Message::PlayAt {
                        source: ListSource::Daily(index),
                        index: i,
                    };
                    Card {
                        artwork: t.artwork.clone(),
                        title: t.title.clone(),
                        subtitle: format::artists(&t.artists),
                        shape: ArtShape::Tile,
                        on_press: play.clone(),
                        play: Some(play),
                    }
                })
                .collect();
            c = c.push(card_grid(app, grid, cards, 1, 3 + index));
        }
    }
    c.push(vspace(gap::XXL)).into()
}

/// The search text of a mood label: the curated query when it is one of ours.
fn mood_query(name: &str) -> String {
    crate::explore::MOODS
        .iter()
        .find(|m| m.label.eq_ignore_ascii_case(name))
        .map_or_else(|| format!("{name} music"), |m| m.query.to_owned())
}
