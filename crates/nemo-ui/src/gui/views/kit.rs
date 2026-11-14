//! Pieces shared by the screens: page frame, cards and grids, track rows, buttons, chips, empty
//! states. Every pressable piece is wrapped in [`fx`](super::super::fx) so it has an animated
//! state layer; cards also lift and enter with a stagger.

use iced::font::Weight;
use iced::widget::text::Wrapping;
use iced::widget::{Space, column, container, image};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use super::super::app::{App, ArtShape, Message};
use super::super::assets::Icon;
use super::super::style::{col, gap, icon, label, label_w, scroll, vertical_gradient};
use crate::signal;
use crate::theme::{color, shape, type_scale};
use nemo_core::track::Artwork;

/// Bottom padding of a scrolling page. The panes already end above the capsule, so this is only
/// breathing room under the last row.
pub const PAGE_BOTTOM: f32 = 16.0;
/// Track row height.
pub const ROW_H: f32 = 56.0;
/// Height of a Quick picks row.
pub const QUICK_H: f32 = 64.0;

/// The colour of the pane behind a page; entrances fade to it.
#[must_use]
pub fn pane_bg() -> Color {
    col(color::surface_container().with_alpha(crate::scheme::PANE_ALPHA))
}

/// A scrolling page: a column `content_w` px wide, centred, with a thin scroll bar and bottom
/// padding for the capsule.
pub fn page<'a>(content_w: u32, body: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let width = content_w as f32;
    let framed = container(column![body.into()].width(Length::Fixed(width)))
        .width(Length::Fill)
        .padding(Padding {
            top: gap::SM,
            bottom: PAGE_BOTTOM,
            ..Padding::ZERO
        })
        .center_x(Length::Fill);
    scroll(framed)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// `title-lg` semibold section title.
pub fn section_title<'a>(text: impl Into<String>) -> Element<'a, Message> {
    label(
        text.into(),
        type_scale::TITLE_LG,
        true,
        col(color::on_surface()),
    )
    .into()
}

/// `headline-md` page title.
pub fn page_title<'a>(text: impl Into<String>) -> Element<'a, Message> {
    label(
        text.into(),
        type_scale::HEADLINE_MD,
        true,
        col(color::on_surface()),
    )
    .into()
}

/// Small caps line above a title.
pub fn eyebrow<'a>(text: impl Into<String>) -> Element<'a, Message> {
    label_w(
        text.into().to_uppercase(),
        type_scale::LABEL_SM,
        Weight::Medium,
        col(color::on_surface_variant()),
    )
    .into()
}

/// Wrapped paragraph of `width` px.
pub fn paragraph<'a>(
    text: impl Into<String>,
    style: crate::theme::TextStyle,
    ink: Color,
    width: f32,
) -> Element<'a, Message> {
    label(text.into(), style, false, ink)
        .wrapping(Wrapping::Word)
        .width(Length::Fixed(width))
        .into()
}

/// Vertical space.
pub fn vspace<'a>(h: f32) -> Element<'a, Message> {
    Space::new().height(h).into()
}

/// Neutral placeholder with a note glyph (`design.md` section 9).
pub fn art_placeholder<'a>(size: f32, radius: f32, loading: bool) -> Element<'a, Message> {
    let _ = loading;
    container(icon(
        Icon::Note,
        (size / 3.0).clamp(16.0, 56.0),
        col(color::on_surface_variant().with_alpha(0.7)),
    ))
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .center_x(Length::Fixed(size))
    .center_y(Length::Fixed(size))
    .style(move |_| container::Style {
        background: Some(vertical_gradient(
            col(color::surface_container_highest()),
            col(color::surface_container_high()),
        )),
        border: Border {
            radius: radius.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .into()
}

/// A picture in a `size` px slot: the cached image or the placeholder.
pub fn art<'a>(
    app: &App,
    artwork: &Artwork,
    size: u32,
    art_shape: ArtShape,
) -> Element<'a, Message> {
    // Slot sizes are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let px = size as f32;
    if let Some(handle) = app.art_handle(artwork, size, art_shape) {
        return image(handle)
            .width(Length::Fixed(px))
            .height(Length::Fixed(px))
            .content_fit(iced::ContentFit::Fill)
            .into();
    }
    art_placeholder(px, art_shape.radius(px), true)
}

mod action;
mod card;
mod chip;
mod track;

pub use action::{filled_button, text_button, tonal_button};
pub use card::{Card, card_grid, card_view, play_badge, shelf_header};
pub use chip::{CHIP_GAP, CHIP_SIDE, chip, chip_geometry, chip_padded, chip_width, sliding_chips};
pub use track::{RowCtx, eq_bars, quick_pick, track_list, track_row};

/// A calm status block: glyph, title, detail and an optional action. Never red.
pub fn status(
    glyph: Icon,
    title: impl Into<String>,
    detail: impl Into<String>,
    action: Option<(&str, Message)>,
) -> Element<'_, Message> {
    let dim = col(color::on_surface_variant());
    let mut c = column![
        icon(glyph, 32.0, dim),
        vspace(gap::MD),
        label(
            title.into(),
            type_scale::TITLE_MD,
            true,
            col(color::on_surface())
        ),
        vspace(gap::XS),
        paragraph(detail, type_scale::BODY_MD, dim, 420.0),
    ]
    .align_x(Alignment::Center);
    if let Some((text, message)) = action {
        c = c
            .push(vspace(gap::LG))
            .push(tonal_button(text, None, Some(message)));
    }
    container(c)
        .width(Length::Fill)
        .padding(Padding::from([gap::XXXL, gap::XL]))
        .center_x(Length::Fill)
        .into()
}

/// The accent-free badge text of the current signal path.
#[must_use]
pub fn badge_text(app: &App) -> String {
    signal::badge_label(app.ui.snapshot.signal_path.as_ref())
}

/// A rounded container with a flat background.
pub fn tile<'a>(
    content: impl Into<Element<'a, Message>>,
    bg: crate::theme::Rgba,
) -> Element<'a, Message> {
    container(content)
        .style(move |_| container::Style {
            background: Some(Background::Color(col(bg))),
            border: Border {
                radius: shape::lg().into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}
