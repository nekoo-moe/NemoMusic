//! Home: the time-of-day greeting and the hero banner card.

use iced::widget::{Space, column, row};
use iced::{Alignment, Color, Element, Length};
use nemo_core::catalog::ShelfItem;
use nemo_core::track::Artwork;

use crate::format;
use crate::gui::app::{App, ArtShape, ListSource, Message};
use crate::gui::assets::Icon;
use crate::gui::style::{col, gap, label};
use crate::gui::views::kit::vspace;
use crate::theme::{color, type_scale};

/// What the hero banner shows.
struct Hero {
    tag: String,
    title: String,
    subtitle: String,
    artwork: Artwork,
    play: Message,
}

fn hero_info(app: &App) -> Option<Hero> {
    use crate::daily::DailyKind;
    if let Some((i, sec)) = app
        .daily_sections
        .iter()
        .enumerate()
        .find(|(_, s)| matches!(s.kind, DailyKind::DailyMix) && !s.tracks.is_empty())
    {
        return Some(Hero {
            tag: sec.tag.to_owned(),
            title: sec.title.clone(),
            subtitle: format!(
                "{} \u{b7} {} and more",
                sec.tracks[0].title,
                format::artists(&sec.tracks[0].artists)
            ),
            artwork: sec.tracks[0].artwork.clone(),
            play: Message::PlayAt {
                source: ListSource::Daily(i),
                index: 0,
            },
        });
    }
    // Signed out, or no history yet: a plain suggestion, said so.
    for (si, shelf) in app.home.shelves.iter().enumerate() {
        if let Some((n, t)) = shelf
            .items
            .iter()
            .filter_map(|i| match i {
                ShelfItem::Song(t) => Some(t),
                _ => None,
            })
            .enumerate()
            .next()
        {
            return Some(Hero {
                tag: "A suggestion to start with".to_owned(),
                title: t.title.clone(),
                subtitle: format::artists(&t.artists),
                artwork: t.artwork.clone(),
                play: Message::PlayAt {
                    source: ListSource::Home(si),
                    index: n,
                },
            });
        }
    }
    None
}

/// The time-of-day greeting and the hero banner card.
pub(super) fn greeting(app: &App, content_w: f32) -> Element<'_, Message> {
    use crate::gui::style::{button_flat, vertical_gradient};
    let mut c = column![
        label(
            app.day.greeting(),
            type_scale::HEADLINE_MD,
            true,
            col(color::on_surface())
        ),
        vspace(gap::MD),
    ];
    if let Some(h) = hero_info(app) {
        let accent = app.accent();
        let art_px = 176_u32;
        let text_w = (content_w - 176.0 - 2.0 * gap::XL - gap::XL).max(200.0);
        let info = column![
            label(
                crate::gui::style::fit(&h.tag, text_w, type_scale::LABEL_MD.size).to_uppercase(),
                type_scale::LABEL_SM,
                false,
                col(accent.on_primary_container.with_alpha(0.8))
            ),
            vspace(gap::XS),
            label(
                crate::gui::style::fit(&h.title, text_w, type_scale::HEADLINE_SM.size),
                type_scale::HEADLINE_SM,
                true,
                col(accent.on_primary_container)
            ),
            vspace(gap::XS),
            label(
                crate::gui::style::fit(&h.subtitle, text_w, type_scale::BODY_MD.size),
                type_scale::BODY_MD,
                false,
                col(accent.on_primary_container.with_alpha(0.85))
            ),
            vspace(gap::LG),
            crate::gui::views::kit::filled_button("Play", Icon::Play, accent, h.play.clone()),
        ]
        .width(Length::Fixed(text_w));
        let body = row![
            info,
            Space::new().width(Length::Fill),
            crate::gui::views::kit::art(app, &h.artwork, art_px, ArtShape::Tile)
        ]
        .align_y(Alignment::Center)
        .padding(gap::XL);
        let top = accent.primary_container;
        let bottom = accent
            .primary_container
            .with_alpha(0.55)
            .over(color::surface_container());
        let radius = crate::theme::shape::xl();
        let b = iced::widget::button(iced::widget::container(body).width(Length::Fill).style(
            move |_| iced::widget::container::Style {
                background: Some(vertical_gradient(col(top), col(bottom))),
                border: iced::Border {
                    radius: radius.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::container::Style::default()
            },
        ))
        .padding(0)
        .width(Length::Fill)
        .on_press(h.play);
        c = c
            .push(
                crate::gui::fx::fx(b.style(button_flat(Color::TRANSPARENT, radius)))
                    .radius(radius)
                    .lift(2.0),
            )
            .push(vspace(gap::XL));
    }
    c.into()
}
