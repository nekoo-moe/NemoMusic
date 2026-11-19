//! The live preview at the top of Appearance: a small mock of the window (sidebar, a pane with
//! a featured card and rows, the right pane, the floating bar) drawn from tokens and the
//! current settings, so every choice shows at once. With window glass on, a stand-in desktop
//! shows through the gaps at the chosen opacity.

use iced::widget::{Space, column, container, row, stack};
use iced::{Alignment, Background, Border, Element, Length, Padding};

use crate::gui::app::{App, Message};
use crate::gui::style::{DOWN, col, gap, label, shadow};
use crate::gui::views::bar::capsule_fill;
use crate::palette::{Palette, mix_rgb};
use crate::scheme::PANE_ALPHA;
use crate::settings::NowPlayingStyle;
use crate::theme::{Rgba, color, glass, shape, type_scale};
use crate::window_glass::WindowGlass;

/// Height of the preview.
const HEIGHT: f32 = 208.0;
/// Width of the mock sidebar and of the mock right pane.
const SIDE_W: f32 = 56.0;
const RIGHT_W: f32 = 112.0;
/// Height of the mock player bar.
const BAR_H: f32 = 32.0;

/// A filled box with a corner radius.
fn block<'a>(w: Length, h: Length, fill: Rgba, radius: f32) -> Element<'a, Message> {
    container(Space::new())
        .width(w)
        .height(h)
        .style(move |_| container::Style {
            background: Some(col(fill).into()),
            border: Border {
                radius: radius.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

/// A skeleton line of `w` px in the quiet ink.
fn line<'a>(w: f32, strong: bool) -> Element<'a, Message> {
    let ink = if strong {
        color::on_surface().with_alpha(0.70)
    } else {
        color::on_surface_variant().with_alpha(0.40)
    };
    block(Length::Fixed(w), Length::Fixed(6.0), ink, shape::FULL)
}

/// A pane: `fill` with the pane radius.
fn pane<'a>(
    content: impl Into<Element<'a, Message>>,
    w: Length,
    fill: Rgba,
) -> Element<'a, Message> {
    container(content)
        .width(w)
        .height(Length::Fill)
        .padding(gap::SM)
        .clip(true)
        .style(move |_| container::Style {
            background: Some(col(fill).into()),
            border: Border {
                radius: shape::lg().into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

/// The sidebar: a pill under the chosen entry and three quiet entries.
fn sidebar<'a>(accent: &Palette, fill: Rgba) -> Element<'a, Message> {
    let entry = |on: bool| {
        let c = if on {
            accent.secondary_container
        } else {
            color::on_surface().with_alpha(0.12)
        };
        block(Length::Fill, Length::Fixed(16.0), c, shape::FULL)
    };
    pane(
        column![entry(true), entry(false), entry(false), entry(false)].spacing(gap::SM),
        Length::Fixed(SIDE_W),
        fill,
    )
}

/// The centre pane: a featured card in the accent container and two rows.
fn centre<'a>(accent: &Palette, fill: Rgba) -> Element<'a, Message> {
    let featured = container(
        column![
            line(72.0, true),
            line(44.0, false),
            block(
                Length::Fixed(44.0),
                Length::Fixed(16.0),
                accent.primary,
                shape::FULL
            )
        ]
        .spacing(gap::SM),
    )
    .width(Length::Fill)
    .padding(gap::MD)
    .style({
        let c = accent.primary_container;
        move |_| container::Style {
            background: Some(col(c).into()),
            border: Border {
                radius: shape::md().into(),
                ..Border::default()
            },
            ..container::Style::default()
        }
    });
    let item = || {
        row![
            block(
                Length::Fixed(20.0),
                Length::Fixed(20.0),
                color::on_surface().with_alpha(0.16),
                shape::sm()
            ),
            line(88.0, true)
        ]
        .spacing(gap::SM)
        .align_y(Alignment::Center)
    };
    pane(
        column![featured, item(), item()].spacing(gap::SM),
        Length::Fill,
        fill,
    )
}

/// The right pane in the arrangement of the chosen Now Playing style.
fn right<'a>(app: &App, accent: &Palette, fill: Rgba) -> Element<'a, Message> {
    let cover = block(
        Length::Fill,
        Length::Fixed(40.0),
        accent.secondary_container,
        shape::md(),
    );
    let lyric = |on: bool| line(if on { 60.0 } else { 44.0 }, on);
    let body: Element<'a, Message> = match app.ui.settings.now_playing_style {
        NowPlayingStyle::Classic => column![cover, line(56.0, true), line(36.0, false)]
            .spacing(gap::SM)
            .into(),
        _ => column![cover, lyric(false), lyric(true), lyric(false)]
            .spacing(gap::SM)
            .into(),
    };
    pane(body, Length::Fixed(RIGHT_W), fill)
}

/// The floating bar: a cover, two lines, the play disc and a rail, on the same Material surface
/// as the real bar: a tonal fill, no outline, a soft shadow.
fn bar<'a>(app: &App, accent: &Palette) -> Element<'a, Message> {
    let fill = capsule_fill(app);
    let inner = row![
        block(
            Length::Fixed(20.0),
            Length::Fixed(20.0),
            accent.secondary_container,
            shape::sm()
        ),
        column![line(52.0, true), line(32.0, false)].spacing(gap::XS),
        Space::new().width(Length::Fill),
        block(
            Length::Fixed(20.0),
            Length::Fixed(20.0),
            accent.primary,
            shape::FULL
        ),
        block(
            Length::Fixed(72.0),
            Length::Fixed(4.0),
            color::on_surface().with_alpha(0.24),
            shape::FULL
        ),
    ]
    .spacing(gap::SM)
    .align_y(Alignment::Center);
    container(inner)
        .width(Length::Fill)
        .height(Length::Fixed(BAR_H))
        .padding(Padding::from([0.0, gap::MD]))
        .align_y(Alignment::Center)
        .style(move |_| container::Style {
            background: Some(col(fill).into()),
            border: Border {
                radius: shape::FULL.into(),
                ..Border::default()
            },
            shadow: shadow(
                glass::ELEVATED_OFFSET_Y,
                glass::ELEVATED_BLUR,
                glass::SHADOW,
            ),
            ..container::Style::default()
        })
        .into()
}

/// The window backdrop in the preview at `alpha`: the song's colours while they are on and a
/// song plays, else the flat colour of the theme.
fn backdrop(app: &App, alpha: f32) -> Background {
    let flat = app.bg_fade.current()[1];
    let (Some(cover), true) = (&app.cover, app.ui.settings.song_colour_theme) else {
        return col(flat.with_alpha(alpha)).into();
    };
    let ground = color::surface();
    let tone = |c: Rgba| col(mix_rgb(c, ground, 0.62).with_alpha(alpha));
    Background::Gradient(
        iced::gradient::Linear::new(iced::Radians(DOWN))
            .add_stop(0.0, tone(cover.strip[0]))
            .add_stop(0.5, tone(cover.strip[1]))
            .add_stop(1.0, tone(cover.strip[2]))
            .into(),
    )
}

/// The stand-in desktop behind a see-through window: a gradient and two discs.
fn desktop<'a>(accent: &Palette) -> Element<'a, Message> {
    let g = iced::gradient::Linear::new(iced::Radians(DOWN * 0.75))
        .add_stop(0.0, col(accent.primary))
        .add_stop(1.0, col(accent.tertiary));
    let disc = |size: f32, c: Rgba, x: f32, y: f32| {
        container(block(
            Length::Fixed(size),
            Length::Fixed(size),
            c,
            shape::FULL,
        ))
        .padding(Padding {
            left: x,
            top: y,
            ..Padding::ZERO
        })
    };
    stack![
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Gradient(g.into())),
                ..container::Style::default()
            }),
        disc(72.0, accent.primary, 24.0, 96.0),
        disc(48.0, accent.on_primary_container, 300.0, 8.0),
    ]
    .into()
}

/// The preview, `width` px wide.
pub(super) fn preview(app: &App, width: f32) -> Element<'_, Message> {
    let s = &app.ui.settings;
    let accent = app.accent();
    // The preview shows what a restart gives: the glass as if the window were see-through.
    let glass = WindowGlass {
        created_transparent: true,
        ..s.window_glass(true)
    };
    let clear = glass.is_clear();
    let pane_fill = color::surface_container().with_alpha(glass.pane_alpha(PANE_ALPHA));
    let alpha = glass.backdrop_alpha(false);
    let ground = container(Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(backdrop(app, alpha)),
            ..container::Style::default()
        });
    let mut window_cols = row![sidebar(&accent, pane_fill), centre(&accent, pane_fill)]
        .spacing(gap::SM)
        .height(Length::Fill);
    if width >= 520.0 {
        window_cols = window_cols.push(right(app, &accent, pane_fill));
    }
    let window = container(column![window_cols, bar(app, &accent)].spacing(gap::SM))
        .padding(gap::SM)
        .width(Length::Fill)
        .height(Length::Fill);
    let mut layers: Vec<Element<'_, Message>> = Vec::new();
    if clear {
        layers.push(desktop(&accent));
    }
    layers.push(ground.into());
    layers.push(window.into());
    let caption = if clear {
        "Preview. The desktop behind it is a stand-in."
    } else {
        "Preview of this window."
    };
    column![
        container(stack(layers))
            .width(Length::Fixed(width))
            .height(Length::Fixed(HEIGHT))
            .clip(true)
            .style(|_| container::Style {
                background: Some(col(color::surface()).into()),
                border: Border {
                    radius: shape::lg().into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
        label(
            caption,
            type_scale::LABEL_SM,
            false,
            col(color::on_surface_variant())
        ),
    ]
    .spacing(gap::XS)
    .into()
}
