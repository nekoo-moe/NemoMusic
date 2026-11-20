//! Controls and card shapes of the Appearance page: the 52 x 32 switch, the card, and the row
//! that puts a control on the card's right edge.

use iced::widget::{Space, button, column, container, row, slider};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::gui::app::{App, Message};
use crate::gui::style::{animated, col, gap, label, slider_style};
use crate::gui::views::kit::paragraph;
use crate::settings::SettingChange;
use crate::state::Event;
use crate::theme::{TextStyle, color, shape, type_scale};

/// Width of the switch track (`design.md` section 4).
const SWITCH_W: f32 = 52.0;
/// Height of the switch track and of every target on the page.
pub(super) const TARGET: f32 = 32.0;
/// Opacity of a control that cannot be used now (`design.md` section 5).
const DISABLED_ALPHA: f32 = 0.38;
/// Padding inside a card.
pub(super) const CARD_PAD: f32 = gap::LG;
/// Least width of the text column beside a control; below it the control drops under the text.
const MIN_TEXT_W: f32 = 200.0;
/// Width of a switch with its `On` or `Off` text.
pub(super) const SWITCH_BOX_W: f32 = 116.0;
/// Width of a switch with the word `Overridden`.
const SWITCH_BOX_WIDE: f32 = 156.0;

/// What a switch shows beside its track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SwitchMode {
    /// Usable: `On` or `Off`.
    Normal,
    /// Greyed out, presses do nothing.
    Disabled,
    /// Usable, but another setting overrides it: shows `Overridden`.
    Overridden,
}

impl SwitchMode {
    /// Width of the switch with its text.
    pub(super) const fn box_w(self) -> f32 {
        match self {
            Self::Overridden => SWITCH_BOX_WIDE,
            _ => SWITCH_BOX_W,
        }
    }
}

/// The track and thumb of a switch, as a button so it gets the hover layer, the focus ring and
/// a 32 px target.
fn track<'a>(on: bool, dim: f32, press: Option<Message>) -> Element<'a, Message> {
    let ink = color::on_surface();
    let (fill, thumb, size, pad) = if on {
        (
            ink.with_alpha(dim),
            color::surface().with_alpha(dim),
            24.0,
            4.0,
        )
    } else {
        (
            ink.with_alpha(0.24 * dim),
            color::on_surface_variant().with_alpha(dim),
            16.0,
            8.0,
        )
    };
    let round = move |c: crate::theme::Rgba| {
        move |_: &iced::Theme| container::Style {
            background: Some(col(c).into()),
            border: iced::Border {
                radius: shape::FULL.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        }
    };
    let dot = container(Space::new())
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(round(thumb));
    let body = container(dot)
        .width(Length::Fixed(SWITCH_W))
        .height(Length::Fixed(TARGET))
        .padding(pad)
        .align_x(if on { Alignment::End } else { Alignment::Start })
        .align_y(Alignment::Center)
        .style(round(fill));
    let b = button(body).padding(0).on_press_maybe(press);
    animated(b, Color::TRANSPARENT, shape::FULL)
}

/// A switch with its state as text (`On`, `Off` or `Overridden`): state is never colour alone.
pub(super) fn switch<'a>(
    lang: crate::settings::Language,
    on: bool,
    mode: SwitchMode,
    change: fn(bool) -> SettingChange,
) -> Element<'a, Message> {
    let dim = if mode == SwitchMode::Disabled {
        DISABLED_ALPHA
    } else {
        1.0
    };
    let text = match (mode, on, lang) {
        (SwitchMode::Overridden, _, crate::settings::Language::Vietnamese) => "Bị ghi đè",
        (SwitchMode::Overridden, _, crate::settings::Language::English) => "Overridden",
        (_, true, crate::settings::Language::Vietnamese) => "Bật",
        (_, true, crate::settings::Language::English) => "On",
        (_, false, crate::settings::Language::Vietnamese) => "Tắt",
        (_, false, crate::settings::Language::English) => "Off",
    };
    let press = (mode != SwitchMode::Disabled).then(|| Message::Ui(Event::Setting(change(!on))));
    row![
        container(label(
            text,
            type_scale::BODY_MD,
            false,
            col(color::on_surface_variant().with_alpha(dim)),
        ))
        .width(Length::Fill)
        .align_x(Alignment::End),
        Space::new().width(gap::MD),
        track(on, dim, press),
    ]
    .width(Length::Fixed(mode.box_w()))
    .align_y(Alignment::Center)
    .into()
}

/// A slider for the window opacity with its value as text.
pub(super) fn opacity_slider<'a>(app: &App, percent: u8, dim: f32) -> Element<'a, Message> {
    let accent = app.accent();
    let bar = slider(40..=95_u8, percent, |v| {
        Message::Ui(Event::Setting(SettingChange::WindowGlassOpacity(v)))
    })
    .step(5_u8)
    .height(TARGET)
    .width(Length::Fixed(160.0))
    .style(slider_style(
        col(accent.primary.with_alpha(dim)),
        col(color::on_surface().with_alpha(0.24 * dim)),
        col(accent.primary.with_alpha(dim)),
        10.0,
    ));
    row![
        bar,
        container(label(
            format!("{percent}%"),
            type_scale::BODY_MD,
            false,
            col(color::on_surface_variant()),
        ))
        .width(Length::Fixed(44.0))
        .align_x(Alignment::End),
    ]
    .align_y(Alignment::Center)
    .into()
}

/// A title and a helper under it, `width` px wide.
pub(super) fn head<'a>(
    title: &'a str,
    style: TextStyle,
    helper: &str,
    width: f32,
) -> Element<'a, Message> {
    let mut c = column![label(title, style, true, col(color::on_surface()))]
        .spacing(gap::XS)
        .width(Length::Fixed(width));
    if !helper.is_empty() {
        c = c.push(paragraph(
            helper.to_owned(),
            type_scale::BODY_SM,
            col(color::on_surface_variant()),
            width,
        ));
    }
    c.into()
}

/// `text(width)` on the left and `control` on the right edge of `inner` when both fit; else the
/// control under the text, still on the right edge.
pub(super) fn aside<'a>(
    inner: f32,
    text: impl FnOnce(f32) -> Element<'a, Message>,
    control: Element<'a, Message>,
    control_w: f32,
) -> Element<'a, Message> {
    let side = inner - control_w - gap::LG;
    if side >= MIN_TEXT_W {
        row![
            text(side),
            Space::new().width(Length::Fill),
            container(control).align_y(Alignment::Center)
        ]
        .align_y(Alignment::Center)
        .width(Length::Fixed(inner))
        .into()
    } else {
        column![
            text(inner),
            container(control)
                .width(Length::Fixed(inner))
                .align_x(Alignment::End)
        ]
        .spacing(gap::SM)
        .into()
    }
}

/// A card on `surface-container-high` with `rows` inside, `width` px wide.
pub(super) fn card(width: f32, rows: Vec<Element<'_, Message>>) -> Element<'_, Message> {
    container(column(rows).spacing(gap::MD))
        .width(Length::Fixed(width))
        .padding(Padding::from(CARD_PAD))
        .style(|_| container::Style {
            background: Some(col(color::surface_container_high()).into()),
            border: iced::Border {
                radius: shape::lg().into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

/// A card with one control: title and helper on the left, the control on the right edge.
pub(super) fn control_card<'a>(
    width: f32,
    title: &'a str,
    helper: &'a str,
    control: Element<'a, Message>,
    control_w: f32,
) -> Element<'a, Message> {
    let inner = width - 2.0 * CARD_PAD;
    card(
        width,
        vec![aside(
            inner,
            move |w| head(title, type_scale::TITLE_SM, helper, w),
            control,
            control_w,
        )],
    )
}

/// A row of a multi-row card: label and helper on the left, a control on the right edge.
pub(super) fn switch_row<'a>(
    inner: f32,
    title: &'a str,
    helper: String,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    aside(
        inner,
        move |w| head(title, type_scale::BODY_MD, &helper, w),
        control,
        SWITCH_BOX_W,
    )
}

/// Estimated width of a segmented control: 7.5 px a letter, 32 px of padding and 22 px for the
/// check mark of the chosen pill, 8 px between pills.
#[must_use]
pub(super) fn segmented_width(labels: &[&str]) -> f32 {
    let pills: f32 = labels
        .iter()
        // Labels are a few letters.
        .map(|l| {
            #[allow(clippy::cast_precision_loss)]
            let letters = l.chars().count() as f32;
            letters * 7.5 + 32.0 + 22.0
        })
        .sum();
    #[allow(clippy::cast_precision_loss)]
    let gaps = labels.len().saturating_sub(1) as f32 * gap::SM;
    pills + gaps
}
