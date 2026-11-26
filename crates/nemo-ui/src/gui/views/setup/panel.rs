//! The stage around the step: the brand panel (from 1100 px wide) and the top row with dots
//! (below that). Shared by every step. The stepper itself is in [`super::stepper`].

use iced::widget::{Space, column, container, image, row, text};
use iced::{Alignment, Color, Element, Length, Padding, Pixels};

use super::stepper::{dots, stepper};
use crate::gui::app::{App, Message};
use crate::gui::assets::{app_logo, display_font};
use crate::gui::style::{col, gap, label};
use crate::setup::SetupStep;
use crate::setup::motion::{TOTAL_MS, settle};
use crate::theme::{TextStyle, color, type_scale};

/// Side of the mark in the brand panel and in the top row.
const MARK_PANEL: f32 = 48.0;
const MARK_TOP: f32 = 32.0;
/// Height of the top row.
const TOP_H: f32 = 48.0;
/// Width of the name's slot in the top row, and of the empty slot that balances it.
const SIDE_W: f32 = 200.0;

/// A run of text in `font.display` at 600.
pub(super) fn display<'a>(
    content: impl Into<String>,
    style: TextStyle,
    ink: Color,
) -> iced::widget::Text<'a> {
    text(content.into())
        .size(Pixels(f32::from(style.size)))
        .line_height(text::LineHeight::Absolute(Pixels(f32::from(style.line))))
        .font(display_font(true))
        .color(ink)
        .wrapping(text::Wrapping::Word)
}

/// The brand mark (`assets/logo/nemo-logo-256.png`). `scale` is 1 at rest; the Done step settles
/// it from a little smaller. The slot keeps its size, so nothing around it moves.
pub(super) fn mark<'a>(side: f32, scale: f32) -> Element<'a, Message> {
    let inner = side * scale;
    container(
        image(app_logo())
            .width(Length::Fixed(inner))
            .height(Length::Fixed(inner)),
    )
    .width(Length::Fixed(side))
    .height(Length::Fixed(side))
    .center_x(Length::Fixed(side))
    .center_y(Length::Fixed(side))
    .into()
}

/// Scale of the mark: the Done step settles it once, every other moment it is at rest.
fn mark_scale(app: &App, current: SetupStep) -> f32 {
    let running = app.setup_ms() < TOTAL_MS;
    if running && current == SetupStep::Done && app.setup_from() != Some(SetupStep::Done) {
        settle(app.setup_ms())
    } else {
        1.0
    }
}

/// The left panel: the mark and name, where the person is (`Step 2 of 6` and the step's name in
/// `font.display`), and the vertical stepper.
pub(super) fn brand_panel(app: &App, current: SetupStep, width: f32) -> Element<'_, Message> {
    let dim = col(color::on_surface_variant());
    let brand = row![
        mark(MARK_PANEL, mark_scale(app, current)),
        Space::new().width(gap::MD),
        display("NemoMusic", type_scale::TITLE_MD, col(color::on_surface())),
    ]
    .align_y(Alignment::Center);
    let lang = app.ui.settings.language;
    let place = crate::i18n::step_place(current, lang);
    let body = column![
        brand,
        Space::new().height(gap::XL),
        label(place, type_scale::LABEL_SM, false, dim),
        display(
            current.title_localized(lang),
            type_scale::HEADLINE_LG,
            col(color::on_surface())
        )
        .width(Length::Fill),
        Space::new().height(gap::XL),
        stepper(app, current),
    ]
    .width(Length::Fixed(width - 2.0 * gap::SM));
    container(body)
        .width(Length::Fixed(width))
        .height(Length::Fill)
        .padding(Padding::from([gap::SM, gap::SM]))
        .into()
}

/// Below 1100 px: the small mark and name on the left, one dot per step in the middle.
pub(super) fn top_row(app: &App, current: SetupStep) -> Element<'_, Message> {
    row![
        container(
            row![
                mark(MARK_TOP, mark_scale(app, current)),
                Space::new().width(gap::SM),
                display("NemoMusic", type_scale::TITLE_SM, col(color::on_surface())),
            ]
            .align_y(Alignment::Center)
        )
        .width(Length::Fixed(SIDE_W)),
        Space::new().width(Length::Fill),
        dots(app, current),
        Space::new().width(Length::Fill),
        Space::new().width(Length::Fixed(SIDE_W)),
    ]
    .align_y(Alignment::Center)
    .padding(Padding::from([0.0, gap::SM]))
    .height(Length::Fixed(TOP_H))
    .into()
}
