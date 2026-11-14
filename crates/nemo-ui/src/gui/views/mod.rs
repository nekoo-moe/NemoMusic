//! Views. [`root`] picks the three-pane shell or full-window Now Playing and stacks the overlays;
//! the content router in [`shell`] matches on [`Route`], so a new screen adds one arm.

pub mod account;
pub mod backdrop;
pub mod bar;
pub mod detail;
pub mod explore;
pub mod home;
pub mod kit;
pub mod library;
pub mod lyrics;
pub mod now_playing;
pub mod opt_in;
pub mod overlays;
pub mod pane;
pub mod panels;
pub mod queue;
pub mod search;
pub mod settings;
pub mod settings_lyrics;
pub mod shell;

use iced::widget::{container, stack};
use iced::{Alignment, Element, Length, Padding};

use super::app::{App, Message};
use super::style::{col, gap, label};
use crate::layout;
use crate::route::Route;
use crate::theme::{color, shape, type_scale};

/// The whole window.
pub fn root(app: &App) -> Element<'_, Message> {
    if app.mini {
        return panels::mini_player(app);
    }
    let (w, h) = app.window_px();
    let now_playing = matches!(app.ui.route(), Route::NowPlaying);
    let page = if now_playing {
        now_playing::view(app, w, h)
    } else {
        shell::view(app, w, h)
    };
    let mut layers: Vec<Element<'_, Message>> = vec![
        background(app),
        container(page)
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
    ];

    // Space below the notices, so they sit above the capsule and not under it.
    let bar_space = if now_playing || app.ui.snapshot.current.is_none() {
        gap::XL
    } else {
        // Capsule 72 + margin 16 + a little air.
        // Layout numbers are small, exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let space = (layout::CAPSULE_HEIGHT + layout::CAPSULE_MARGIN + layout::PANE_GAP) as f32;
        space
    };

    if let Some(toast) = overlays::toast(app) {
        layers.push(
            container(toast)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(Alignment::End)
                .center_x(Length::Fill)
                .padding(Padding {
                    bottom: bar_space + gap::XS,
                    ..Padding::ZERO
                })
                .into(),
        );
    }
    if app.ui.signal_open {
        let (top, bottom) = if now_playing {
            (56.0, 0.0)
        } else {
            (0.0, bar_space + gap::XS)
        };
        layers.push(
            container(overlays::signal_popover(app))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::End)
                .align_y(if now_playing {
                    Alignment::Start
                } else {
                    Alignment::End
                })
                .padding(Padding {
                    top,
                    right: gap::XL,
                    bottom,
                    left: 0.0,
                })
                .into(),
        );
    }
    if app.readout {
        let text = app
            .frame_stats
            .readout()
            .map_or_else(|| "measuring\u{2026}".to_owned(), |r| r.text());
        let plate = container(label(
            text,
            type_scale::LABEL_SM,
            false,
            col(color::on_surface()),
        ))
        .padding(Padding::from([gap::XS, gap::SM]))
        .style(|_| container::Style {
            background: Some(col(color::surface_container_high()).into()),
            border: iced::Border {
                radius: shape::sm().into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });
        layers.push(
            container(plate)
                .padding(gap::SM)
                .align_x(Alignment::Start)
                .align_y(Alignment::Start)
                .into(),
        );
    }
    if let Some(track) = &app.ui.menu {
        layers.push(overlays::track_menu(app, track));
    }
    if app.tools_open {
        layers.push(
            container(panels::tools_panel(app))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::End)
                .align_y(Alignment::End)
                .padding(Padding {
                    right: gap::XL,
                    bottom: bar_space + gap::XS,
                    ..Padding::ZERO
                })
                .into(),
        );
    }
    if app.create_open {
        layers.push(panels::create_sheet(app));
    }
    if app.shortcuts_open {
        layers.push(panels::shortcuts_sheet(app));
    }
    if app.account.open {
        layers.push(account::sheet(app));
    }
    if app.ui.tos_sheet_visible() {
        layers.push(overlays::tos_sheet(app));
    }
    stack(layers).into()
}

/// The coloured field behind the panes: a soft diagonal gradient of three dark tonal colours
/// from the song or the page, with a vignette toward the bottom for depth. The colours
/// cross-fade (`scheme::BgFade`), so the whole window changes colour with the song.
fn background(app: &App) -> Element<'_, Message> {
    let [c0, c1, c2] = app.bg_fade.current();
    let base = color::surface_container_lowest();
    let diag = iced::gradient::Linear::new(iced::Radians(2.35))
        .add_stop(0.0, col(c0))
        .add_stop(0.55, col(c1))
        .add_stop(1.0, col(c2));
    let vignette = iced::gradient::Linear::new(iced::Radians(super::style::DOWN))
        .add_stop(0.0, iced::Color::TRANSPARENT)
        .add_stop(1.0, col(base.with_alpha(0.45)));
    iced::widget::stack![
        container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(iced::Background::Gradient(diag.into())),
                ..container::Style::default()
            }),
        container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(iced::Background::Gradient(vignette.into())),
                ..container::Style::default()
            }),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
