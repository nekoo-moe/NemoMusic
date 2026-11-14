//! Large cards and the card grid, with the hover play badge.

use iced::widget::{button, column, container, row, stack};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};
use nemo_core::track::Artwork;

use super::{art, pane_bg, section_title, vspace};
use crate::gui::app::{App, ArtShape, Message};
use crate::gui::assets::Icon;
use crate::gui::fx::{Badge, BadgeAnchor, entrance, fx};
use crate::gui::style::{button_flat, col, fit, fit_lines, gap, label};
use crate::layout;
use crate::theme::{color, shape, type_scale};

/// What a card shows.
pub struct Card {
    /// Picture source.
    pub artwork: Artwork,
    /// Title.
    pub title: String,
    /// Subtitle.
    pub subtitle: String,
    /// Picture shape.
    pub shape: ArtShape,
    /// Pressed.
    pub on_press: Message,
    /// Sent by the play button that fades in on hover; none for cards that cannot play.
    pub play: Option<Message>,
}

/// The hover play button in the cover's colours.
pub fn play_badge<'a>(
    app: &App,
    size: f32,
    anchor: BadgeAnchor,
    message: Message,
) -> Badge<'a, Message> {
    let accent = app.accent();
    Badge {
        size,
        anchor,
        fill: col(accent.primary),
        ink: col(accent.on_primary),
        glyph: Icon::Play.handle(),
        on_press: Box::new(move || message.clone()),
    }
}

fn scrim_style(radius: f32) -> impl Fn(&iced::Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Gradient(
            iced::gradient::Linear::new(iced::Radians(crate::gui::style::DOWN))
                .add_stop(0.0, Color::from_rgba(0.0, 0.0, 0.0, 0.0))
                .add_stop(0.52, Color::from_rgba(0.0, 0.0, 0.0, 0.0))
                .add_stop(1.0, Color::from_rgba(0.0, 0.0, 0.0, 0.82))
                .into(),
        )),
        border: Border {
            radius: radius.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// A large card: the art fills it, the title and subtitle sit on a bottom scrim, and it lifts on
/// hover. Artists get a round picture with the text below. `stagger` is its place in the
/// entrance order.
pub fn card_view<'a>(app: &App, card: Card, size: u32, stagger: usize) -> Element<'a, Message> {
    // Card sizes are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let px = size as f32;
    let progress = app.anim.reveal(stagger);
    if card.shape == ArtShape::Circle {
        let text = column![
            label(
                fit(&card.title, px, type_scale::BODY_MD.size),
                type_scale::BODY_MD,
                true,
                col(color::on_surface())
            ),
            label(
                fit(&card.subtitle, px, type_scale::BODY_SM.size),
                type_scale::BODY_SM,
                false,
                col(color::on_surface_variant())
            ),
        ]
        .width(Length::Fixed(px))
        .align_x(Alignment::Center);
        let b = button(
            column![
                art(app, &card.artwork, size, ArtShape::Circle),
                vspace(gap::MD),
                text
            ]
            .width(Length::Fixed(px))
            .align_x(Alignment::Center),
        )
        .padding(0)
        .on_press(card.on_press);
        return entrance(
            fx(b.style(button_flat(Color::TRANSPARENT, shape::xl())))
                .radius(shape::xl())
                .lift(4.0),
            progress,
            pane_bg(),
        );
    }
    let ink = col(color::on_surface());
    let text = column![
        // Two lines for a long title, broken at a space; never a cut inside a word.
        label(
            fit_lines(&card.title, px - 28.0, type_scale::TITLE_SM.size, 2),
            type_scale::TITLE_SM,
            true,
            ink
        ),
        label(
            fit(&card.subtitle, px - 28.0, type_scale::BODY_SM.size),
            type_scale::BODY_SM,
            false,
            col(color::on_surface().with_alpha(0.78))
        ),
    ];
    let overlay = container(text)
        .width(Length::Fixed(px))
        .height(Length::Fixed(px))
        .align_y(Alignment::End)
        .padding(Padding {
            top: 0.0,
            right: gap::MD,
            bottom: gap::MD,
            left: gap::MD,
        })
        .style(scrim_style(crate::gui::app::TILE_RADIUS));
    let b = button(stack![
        art(app, &card.artwork, size, ArtShape::Tile),
        overlay
    ])
    .padding(0)
    .on_press(card.on_press);
    let radius = shape::scaled(crate::gui::app::TILE_RADIUS);
    let mut host = fx(b.style(button_flat(Color::TRANSPARENT, radius)))
        .radius(radius)
        .lift(4.0)
        .zoom(0.025);
    if let Some(play) = card.play {
        host = host.badge(play_badge(app, 44.0, BadgeAnchor::BottomRight(12.0), play));
    }
    entrance(host, progress, pane_bg())
}

/// Cards in rows of `grid.columns`, at most `max_rows` rows. `base` is the stagger index of the
/// first card.
pub fn card_grid<'a>(
    app: &App,
    grid: layout::Grid,
    cards: Vec<Card>,
    max_rows: usize,
    base: usize,
) -> Element<'a, Message> {
    // Grid numbers are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let gap_px = grid.gap as f32;
    let per_row = grid.columns as usize;
    let mut rows = column![].spacing(gap::XL);
    let mut it = cards
        .into_iter()
        .take(per_row * max_rows)
        .enumerate()
        .peekable();
    while it.peek().is_some() {
        let line: Vec<Element<'a, Message>> = it
            .by_ref()
            .take(per_row)
            .map(|(i, c)| card_view(app, c, grid.card, base + i))
            .collect();
        rows = rows.push(row(line).spacing(gap_px));
    }
    rows.into()
}

/// A shelf header: title on the left.
pub fn shelf_header<'a>(title: impl Into<String>) -> Element<'a, Message> {
    column![section_title(title), vspace(gap::MD)].into()
}
